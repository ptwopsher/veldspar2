use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use glam::{IVec3, Vec2, Vec3};
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};
use veldspar_shared::block::{
    is_bed_block, is_button, is_fire_block, is_flammable, is_lava_block, is_lever,
    is_sign, is_trapdoor_block, is_water_block, is_wheat_block, register_default_blocks,
    wheat_block_at_stage, wheat_growth_stage, BlockId, BlockRegistry,
};
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::{
    chunk_to_world, local_to_index, world_to_chunk, ChunkPos, LocalPos, CHUNK_SIZE,
};
use veldspar_shared::fluid::{simulate_lava_near, simulate_water_near};
use veldspar_shared::inventory::{
    armor_defense_points, armor_slot_for_item, max_stack_for_item, tool_max_durability,
    tool_properties, tool_speed_multiplier, ArmorSlot, Inventory, ItemId, ItemStack, ToolKind,
    ToolTier, FIRST_NON_BLOCK_ITEM_ID,
};
use veldspar_shared::mob::{mob_color, mob_properties, MobAiState, MobData, MobType};
use veldspar_shared::physics::{raycast_blocks, Face, Ray};
use veldspar_shared::protocol::{self, C2S, PlayerInputFlags, S2C};
use veldspar_shared::recipe;
use veldspar_shared::worldgen::WorldGenerator;

use crate::camera::Camera;
use crate::input::InputState;
use crate::mesh_worker::{MeshRequest, MeshWorker};
use crate::net::ClientNet;
use crate::persistence::{scan_worlds, ClientPersistence, SavedPlayMode, WorldMeta, WorldSummary};
use crate::renderer::item_drop_renderer::ItemDropRenderData;
use crate::renderer::mesh::{ChunkMeshes, ChunkVertex};
use crate::renderer::player_renderer::{MobRenderInfo, RemotePlayer};
use crate::renderer::{RenderFrameStats, Renderer};
use crate::ui::debug_overlay::DebugInfo;
use crate::ui::inventory;
use crate::ui::main_menu::{
    PauseMenuHitTarget, SettingsHitTarget, SettingsMenuView, SettingsSliderKind,
    WorldCreateInputField, WorldListEntryView, WorldSelectHitTarget, WorldSelectView,
};
use tracing::{debug, error, info, warn};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const GRAVITY: f32 = -32.0;
const JUMP_VELOCITY: f32 = 9.0;
const WALK_SPEED: f32 = 4.3;
const SPRINT_SPEED: f32 = 8.0;
const CROUCH_SPEED: f32 = 1.5;
const FLY_SPEED: f32 = 24.0;
const LADDER_CLIMB_SPEED: f32 = 4.0;
const DOOR_THICKNESS: f32 = 0.1875;
const EYE_HEIGHT: f32 = 1.62;
const PLAYER_HALF_W: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;
const MULTIPLAYER_MAX_IN_FLIGHT_CHUNKS: usize = 48;
const MULTIPLAYER_CHUNK_BATCH_SIZE: usize = 8;
const SINGLEPLAYER_CHUNK_REQUEST_BATCH_SIZE: usize = 128;
const MIN_CHUNKS_PER_FRAME: usize = 1;
const MAX_CHUNKS_PER_FRAME: usize = 4;
const LAVA_SIMULATION_INTERVAL_FRAMES: u8 = 3;
const FOG_START: f32 = 304.0;
const FOG_END: f32 = 368.0;
const WEATHER_MIN_DURATION_SECS: f32 = 300.0;
const WEATHER_MAX_DURATION_SECS: f32 = 600.0;
const WEATHER_SPAWN_RADIUS: f32 = 16.0;
const WEATHER_OCCLUSION_CHECK_HEIGHT: i32 = 20;
const WEATHER_SNOW_TEMP_THRESHOLD: f64 = -0.2;
const WEATHER_RAIN_MIN_SPAWN_PER_FRAME: usize = 50;
const WEATHER_RAIN_MAX_SPAWN_PER_FRAME: usize = 100;
const WEATHER_SNOW_MIN_SPAWN_PER_FRAME: usize = 20;
const WEATHER_SNOW_MAX_SPAWN_PER_FRAME: usize = 40;
const WEATHER_RAIN_MIN_HEIGHT: f32 = 2.5;
const WEATHER_RAIN_MAX_HEIGHT: f32 = 6.0;
const WEATHER_SNOW_MIN_HEIGHT: f32 = 3.0;
const WEATHER_SNOW_MAX_HEIGHT: f32 = 8.0;
const ITEM_DROP_LIFETIME: f32 = 300.0;
const ITEM_DROP_GRAVITY: f32 = -18.0;
const ITEM_DROP_MAGNET_RADIUS: f32 = 2.5;
const ITEM_DROP_PICKUP_RADIUS: f32 = 0.6;
const ITEM_DROP_MAGNET_ACCEL: f32 = 50.0;
const ITEM_DROP_PICKUP_DELAY_SECS: f32 = 0.2;
const ITEM_DROP_HALF_SIZE: f32 = 0.125;
const LEAF_DECAY_MIN_DELAY_SECS: f32 = 1.0;
const LEAF_DECAY_MAX_DELAY_SECS: f32 = 5.0;
const LEAF_DECAY_SAPLING_DROP_CHANCE: f32 = 0.10;
const LEAF_SUPPORT_RADIUS: i32 = 4;
const SAPLING_GROWTH_MIN_SECS: f32 = 60.0;
const SAPLING_GROWTH_MAX_SECS: f32 = 120.0;
const SUGAR_CANE_GROWTH_MIN_SECS: f32 = 60.0;
const SUGAR_CANE_GROWTH_MAX_SECS: f32 = 120.0;
const SUGAR_CANE_MAX_HEIGHT: i32 = 3;
const TNT_EXPLOSION_RADIUS: f32 = 4.0;
const REMOTE_WALK_CYCLE_SPEED: f32 = 2.0;
const REMOTE_INTERPOLATION_DURATION: f32 = 0.2;
const ATTACK_ANIMATION_DECAY: f32 = 4.0;
const SINGLE_CHEST_SLOT_COUNT: usize = 27;
const DOUBLE_CHEST_SLOT_COUNT: usize = 54;
const MOB_MAX_COUNT: usize = 20;
const MOB_DESPAWN_DISTANCE: f32 = 64.0;
const MOB_SPAWN_DISTANCE_MIN: f32 = 16.0;
const MOB_SPAWN_DISTANCE_MAX: f32 = 32.0;
const MOB_SPAWN_SCAN_MAX_Y: i32 = 128;
const MOB_GRAVITY: f32 = 20.0;
const SAVE_INTERVAL_SECS: f32 = 60.0;
const WORLDS_DIR: &str = "worlds";
const DEFAULT_WORLD_SEED: u64 = 0xC0FFEE;
const MAX_CONSOLE_MESSAGES: usize = 8;
const MAX_CHAT_MESSAGES: usize = 100;
const CHAT_VISIBLE_MESSAGES: usize = 6;
const CHAT_FADE_SECS: f32 = 10.0;
const FRAME_TIME_HISTORY_LEN: usize = 256;
const PERF_LOG_INTERVAL_SECS: f32 = 1.0;
const UPLOAD_BUDGET_HIGH_BYTES: u64 = 16 * 1024 * 1024;
const UPLOAD_BUDGET_MEDIUM_BYTES: u64 = 8 * 1024 * 1024;
const UPLOAD_BUDGET_LOW_BYTES: u64 = 4 * 1024 * 1024;
const UPLOAD_BUDGET_MIN_BYTES: u64 = 2 * 1024 * 1024;
const MAX_HEALTH: f32 = 20.0;
const MAX_AIR_SUPPLY: f32 = 300.0; // 15 seconds at 20 tps
const FALL_DAMAGE_SAFE_DISTANCE: f32 = 3.0;
const DROWN_DAMAGE_PER_SEC: f32 = 2.0;
const LAVA_CONTACT_DAMAGE_PER_SEC: f32 = 4.0;
const VOID_DAMAGE_PER_SEC: f32 = 4.0;
const VOID_Y: f32 = -64.0;
const HEALTH_REGEN_DELAY: f32 = 4.0; // seconds after last damage before regen starts
const HEALTH_REGEN_RATE: f32 = 1.0; // HP per second
const DAMAGE_FLASH_DURATION: f32 = 0.3;
const MAX_HUNGER: f32 = 20.0;
const HUNGER_PASSIVE_DRAIN_RATE: f32 = 0.01;
const HUNGER_SPRINT_DRAIN_RATE: f32 = 0.045;
const HUNGER_REGEN_THRESHOLD: f32 = 17.0;
const STARVATION_DAMAGE_PER_SEC: f32 = 1.0;
const DAMAGE_REDUCTION_PER_ARMOR_POINT: f32 = 0.04;
const MAX_ARMOR_DAMAGE_REDUCTION: f32 = 0.8;
const CHAT_INPUT_MAX_LEN: usize = 256;
const MAX_WORLD_NAME_INPUT_LEN: usize = 32;
const MAX_WORLD_SEED_INPUT_LEN: usize = 32;
const DEFAULT_NEW_WORLD_NAME: &str = "New World";
const SETTINGS_PATH: &str = "settings.toml";
const MIN_RENDER_DISTANCE: i32 = 4;
const MAX_RENDER_DISTANCE: i32 = 24;
const MIN_STREAM_SURFACE_BELOW: i32 = 0;
const MAX_STREAM_SURFACE_BELOW: i32 = 3;
const MIN_STREAM_FLIGHT_BELOW: i32 = 2;
const MAX_STREAM_FLIGHT_BELOW: i32 = 24;
const MIN_STREAM_ABOVE: i32 = 1;
const MAX_STREAM_ABOVE: i32 = 8;
const MIN_LOD1_DISTANCE: i32 = 4;
const MAX_LOD1_DISTANCE: i32 = 14;
const MIN_MOUSE_SENSITIVITY: f32 = 0.5;
const MAX_MOUSE_SENSITIVITY: f32 = 5.0;
const MIN_FOV: f32 = 60.0;
const MAX_FOV: f32 = 120.0;
const MIN_GUI_SCALE: f32 = 1.0;
const MAX_GUI_SCALE: f32 = 3.0;
const INVENTORY_SLOT_SIZE_MIN_PX: f32 = 46.0;
const INVENTORY_SLOT_SIZE_MAX_PX: f32 = 62.0;
const INVENTORY_SLOT_GAP_MIN_PX: f32 = 4.0;
const INVENTORY_ARMOR_COLUMN_OFFSET_PX: f32 = 50.0;
const CREATIVE_HOTBAR_BLOCKS: [BlockId; Inventory::HOTBAR_SIZE] = [
    BlockId(2),              // granite
    BlockId(3),              // loam
    BlockId(4),              // verdant_turf
    BlockId(10),             // rubblestone
    BlockId(13),             // kiln_brick
    BlockId::TORCH,          // torch
    BlockId::WOODEN_DOOR,    // wooden_door
    BlockId::LADDER,         // ladder
    BlockId::CHEST,          // chest
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InGameMenuState {
    None,
    Pause,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayMode {
    Survival,
    Creative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeatherState {
    Clear,
    Rain,
    Snow,
}

impl From<SavedPlayMode> for PlayMode {
    fn from(value: SavedPlayMode) -> Self {
        match value {
            SavedPlayMode::Survival => Self::Survival,
            SavedPlayMode::Creative => Self::Creative,
        }
    }
}

impl From<PlayMode> for SavedPlayMode {
    fn from(value: PlayMode) -> Self {
        match value {
            PlayMode::Survival => Self::Survival,
            PlayMode::Creative => Self::Creative,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsMenuItem {
    RenderDistance,
    SurfaceBelow,
    FlightBelow,
    StreamAbove,
    LodDistance,
    MouseSensitivity,
    Fov,
    GuiScale,
    ShowFps,
    Back,
}

impl SettingsMenuItem {
    fn from_index(index: u8) -> Self {
        match index {
            0 => Self::RenderDistance,
            1 => Self::SurfaceBelow,
            2 => Self::FlightBelow,
            3 => Self::StreamAbove,
            4 => Self::LodDistance,
            5 => Self::MouseSensitivity,
            6 => Self::Fov,
            7 => Self::GuiScale,
            8 => Self::ShowFps,
            _ => Self::Back,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClientSettings {
    #[serde(default = "default_render_distance")]
    render_distance: i32,
    #[serde(default = "default_stream_surface_below")]
    stream_surface_below: i32,
    #[serde(default = "default_stream_flight_below")]
    stream_flight_below: i32,
    #[serde(default = "default_stream_above")]
    stream_above: i32,
    #[serde(default = "default_lod1_distance")]
    lod1_distance: i32,
    #[serde(default = "default_mouse_sensitivity")]
    mouse_sensitivity: f32,
    #[serde(default = "default_fov")]
    fov: f32,
    #[serde(default = "default_gui_scale")]
    gui_scale: f32,
    #[serde(default)]
    show_fps: bool,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            render_distance: default_render_distance(),
            stream_surface_below: default_stream_surface_below(),
            stream_flight_below: default_stream_flight_below(),
            stream_above: default_stream_above(),
            lod1_distance: default_lod1_distance(),
            mouse_sensitivity: default_mouse_sensitivity(),
            fov: default_fov(),
            gui_scale: default_gui_scale(),
            show_fps: false,
        }
    }
}

impl ClientSettings {
    fn sanitize(mut self) -> Self {
        self.render_distance = self.render_distance.clamp(MIN_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
        self.stream_surface_below = self
            .stream_surface_below
            .clamp(MIN_STREAM_SURFACE_BELOW, MAX_STREAM_SURFACE_BELOW);
        self.stream_flight_below = self
            .stream_flight_below
            .clamp(MIN_STREAM_FLIGHT_BELOW, MAX_STREAM_FLIGHT_BELOW);
        if self.stream_flight_below < self.stream_surface_below + 1 {
            self.stream_flight_below = (self.stream_surface_below + 1)
                .clamp(MIN_STREAM_FLIGHT_BELOW, MAX_STREAM_FLIGHT_BELOW);
        }
        self.stream_above = self.stream_above.clamp(MIN_STREAM_ABOVE, MAX_STREAM_ABOVE);
        self.lod1_distance = self.lod1_distance.clamp(MIN_LOD1_DISTANCE, MAX_LOD1_DISTANCE);
        self.mouse_sensitivity = self
            .mouse_sensitivity
            .clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY);
        self.fov = self.fov.clamp(MIN_FOV, MAX_FOV);
        self.gui_scale = self.gui_scale.clamp(MIN_GUI_SCALE, MAX_GUI_SCALE);
        self
    }

    fn load(path: &Path) -> io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        let parsed = toml::from_str::<Self>(&contents).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to deserialize settings: {e}"),
            )
        })?;
        Ok(parsed.sanitize())
    }

    fn save(&self, path: &Path) -> io::Result<()> {
        let settings = self.clone().sanitize();
        let serialized = toml::to_string_pretty(&settings).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to serialize settings: {e}"),
            )
        })?;
        fs::write(path, serialized)
    }
}

fn default_render_distance() -> i32 {
    12
}

fn default_stream_surface_below() -> i32 {
    0
}

fn default_stream_flight_below() -> i32 {
    10
}

fn default_stream_above() -> i32 {
    2
}

fn default_lod1_distance() -> i32 {
    6
}

fn default_mouse_sensitivity() -> f32 {
    2.5
}

fn default_fov() -> f32 {
    70.0
}

fn default_gui_scale() -> f32 {
    1.0
}

fn load_or_create_settings(path: &Path) -> ClientSettings {
    match ClientSettings::load(path) {
        Ok(settings) => settings,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let settings = ClientSettings::default();
            if let Err(save_err) = settings.save(path) {
                warn!(
                    "Failed to create default settings at {}: {save_err}",
                    path.display()
                );
            }
            settings
        }
        Err(err) => {
            warn!("Failed to load settings from {}: {err}", path.display());
            let settings = ClientSettings::default();
            if let Err(save_err) = settings.save(path) {
                warn!(
                    "Failed to overwrite settings at {}: {save_err}",
                    path.display()
                );
            }
            settings
        }
    }
}

enum AppState {
    MainMenu,
    WorldSelect,
    InGame,
}

enum GameMode {
    Singleplayer,
    Multiplayer {
        net: ClientNet,
    },
}

#[derive(Debug, Clone)]
struct RemotePlayerSyncState {
    previous_position: Vec3,
    target_position: Vec3,
    display_position: Vec3,
    interpolation_time: f32,
    interpolation_duration: f32,
    yaw: f32,
    pitch: f32,
    flags: u8,
    attack_animation: f32,
    breaking_block: Option<IVec3>,
    break_progress: f32,
    animation_phase: f32,
}

#[derive(Debug, Clone)]
struct WorldMenuEntry {
    display_name: String,
    world_dir: PathBuf,
    world_seed: u64,
    size_label: String,
    last_opened_label: String,
}

#[derive(Debug, Clone)]
struct ChatMessage {
    text: String,
    received_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandFeedbackTarget {
    Console,
    Chat,
}

#[derive(Debug, Clone, Copy)]
struct ItemDrop {
    position: Vec3,
    velocity: Vec3,
    item: ItemStack,
    age: f32,
    pickup_delay: f32,
    lifetime: f32,
}

impl ItemDrop {
    fn new(position: Vec3, velocity: Vec3, item: ItemStack) -> Self {
        Self {
            position,
            velocity,
            item,
            age: 0.0,
            pickup_delay: ITEM_DROP_PICKUP_DELAY_SECS,
            lifetime: ITEM_DROP_LIFETIME,
        }
    }
}

#[derive(Debug, Clone)]
struct FurnaceState {
    input: Option<ItemStack>,
    fuel: Option<ItemStack>,
    output: Option<ItemStack>,
    smelt_progress: f32,
    fuel_remaining: f32,
    fuel_total: f32,
}

impl FurnaceState {
    fn new() -> Self {
        Self {
            input: None,
            fuel: None,
            output: None,
            smelt_progress: 0.0,
            fuel_remaining: 0.0,
            fuel_total: 0.0,
        }
    }
}

struct BlockScanState {
    chunk_list: Vec<ChunkPos>,
    current_index: usize,
    chunks_per_frame: usize,
    pending_leaves: Vec<IVec3>,
    pending_saplings: Vec<IVec3>,
    pending_sugar_cane: Vec<IVec3>,
    pending_wheat: Vec<IVec3>,
    pending_fire: Vec<IVec3>,
}

impl Default for BlockScanState {
    fn default() -> Self {
        Self {
            chunk_list: Vec::new(),
            current_index: 0,
            chunks_per_frame: 2,
            pending_leaves: Vec::new(),
            pending_saplings: Vec::new(),
            pending_sugar_cane: Vec::new(),
            pending_wheat: Vec::new(),
            pending_fire: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct PendingMeshUpload {
    chunk_pos: ChunkPos,
    meshes: ChunkMeshes,
    version: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct UploadFrameStats {
    uploaded_bytes: u64,
    uploaded_chunks: u32,
    buffer_reallocations: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct FrameTimeStats {
    avg_ms: f32,
    p95_ms: f32,
    p99_ms: f32,
    max_ms: f32,
}

#[derive(Debug, Clone, Copy)]
struct MeshUploadBudget {
    max_bytes: u64,
    max_chunks: usize,
}

struct ClientApp {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    camera: Camera,
    input: InputState,
    last_frame: Option<Instant>,
    cursor_grabbed: bool,
    settings_path: PathBuf,
    settings: ClientSettings,
    show_debug: bool,
    fps: f32,
    fps_frame_count: u32,
    fps_sample_start: Option<Instant>,
    render_time_seconds: f32,
    frame_time_samples_ms: VecDeque<f32>,
    perf_log_timer_seconds: f32,
    pending_mesh_uploads: VecDeque<PendingMeshUpload>,
    last_upload_stats: UploadFrameStats,
    last_render_stats: RenderFrameStats,
    chunks: HashMap<ChunkPos, ChunkData>,
    dirty_chunks: HashSet<ChunkPos>,
    registry: Option<Arc<BlockRegistry>>,
    selected_block: BlockId,
    inventory: Inventory,
    armor_slots: [Option<ItemStack>; 4],
    selected_hotbar_slot: usize,
    inventory_open: bool,
    crafting_ui_mode: inventory::CraftingUiMode,
    inventory_crafting_slots: [Option<ItemStack>; 4],
    table_crafting_slots: [Option<ItemStack>; 9],
    chest_inventories: HashMap<IVec3, Inventory>,
    open_chest: Option<IVec3>,
    double_chest_partner: Option<IVec3>,
    double_chest_slots: Option<[Option<ItemStack>; DOUBLE_CHEST_SLOT_COUNT]>,
    cursor_stack: Option<ItemStack>,
    creative_search: String,
    creative_scroll: usize,
    creative_catalog: Vec<ItemId>,
    creative_catalog_full: Vec<ItemId>,
    drag_distributing: bool,
    drag_visited_slots: Vec<usize>,
    drag_original_cursor_count: u8,
    last_inv_click_slot: Option<usize>,
    last_inv_click_time: Instant,
    inv_click_count: u8,
    player_pos: Vec3,
    velocity: Vec3,
    on_ground: bool,
    fly_mode: bool,
    flight_stream_floor_y: Option<i32>,
    play_mode: PlayMode,
    sprinting: bool,
    health: f32,
    hunger: f32,
    xp_total: u32,
    xp_level: u32,
    air_supply: f32,
    fall_start_y: Option<f32>,
    last_damage_time: f32,
    damage_flash_timer: f32,
    spawn_position: Vec3,
    block_physics_timer: f32,
    pending_fluid_positions: HashSet<ChunkPos>,
    lava_simulation_frame_accumulator: u8,
    last_player_chunk: Option<ChunkPos>,
    mesh_queue: VecDeque<ChunkPos>,
    mesh_queue_set: HashSet<ChunkPos>,
    mesh_jobs_in_flight: HashSet<ChunkPos>,
    mesh_worker: Option<MeshWorker>,
    mesh_versions: HashMap<ChunkPos, u64>,
    chunk_lods: HashMap<ChunkPos, u8>,
    targeted_block: Option<IVec3>,
    breaking_block: Option<IVec3>,
    break_progress: f32,
    chunk_request_tx: Option<std::sync::mpsc::Sender<ChunkPos>>,
    chunk_result_rx: Option<std::sync::mpsc::Receiver<(ChunkPos, ChunkData)>>,
    pending_chunks: HashSet<ChunkPos>,
    time_of_day: f32,
    time_frozen: bool,
    weather_state: WeatherState,
    weather_timer: f32,
    weather_rng_state: u64,
    gameplay_rng_state: u64,
    mobs: Vec<MobData>,
    mob_spawn_timer: f32,
    mob_spawn_cooldown: f32,
    block_scan_state: BlockScanState,
    leaf_decay_timers: HashMap<IVec3, f32>,
    sapling_growth_timers: HashMap<IVec3, f32>,
    sugar_cane_growth_timers: HashMap<IVec3, f32>,
    world_seed: u64,
    spawn_found: bool,
    chunks_ready: bool,
    app_state: AppState,
    in_game_menu: InGameMenuState,
    game_mode: Option<GameMode>,
    menu_selected: u8,
    pause_menu_selected: u8,
    settings_menu_selected: u8,
    active_settings_slider: Option<SettingsSliderKind>,
    world_entries: Vec<WorldMenuEntry>,
    world_selected: Option<usize>,
    world_create_form_open: bool,
    world_create_name_input: String,
    world_create_seed_input: String,
    world_create_active_field: WorldCreateInputField,
    world_create_play_mode: PlayMode,
    world_delete_confirmation_open: bool,
    server_ip: String,
    remote_players: Vec<RemotePlayer>,
    remote_player_states: HashMap<u64, RemotePlayerSyncState>,
    remote_break_overlays: Vec<(IVec3, f32)>,
    attack_animation: f32,
    was_left_click_down: bool,
    walk_particle_timer: f32,
    item_drops: Vec<ItemDrop>,
    active_world_dir: Option<PathBuf>,
    active_world_name: Option<String>,
    persistence: Option<ClientPersistence>,
    last_save_time: Option<Instant>,
    my_player_id: Option<u64>,
    cursor_position: Option<(f64, f64)>,
    console_open: bool,
    text_input: String,
    console_messages: VecDeque<String>,
    chat_open: bool,
    chat_input: String,
    chat_messages: VecDeque<ChatMessage>,
    tnt_explosion_queue: VecDeque<IVec3>,
    furnace_data: HashMap<IVec3, FurnaceState>,
    open_furnace: Option<IVec3>,
    bed_spawn_point: Option<Vec3>,
    button_timers: Vec<(IVec3, f32)>,
    active_pressure_plates: HashSet<(i32, i32, i32)>,
}

impl Default for ClientApp {
    fn default() -> Self {
        let settings_path = PathBuf::from(SETTINGS_PATH);
        let settings = load_or_create_settings(&settings_path);
        let show_debug = settings.show_fps;
        let mut camera = Camera::default();
        camera.fov = settings.fov.to_radians();
        let inventory = Inventory::new();
        let selected_hotbar_slot = 0;

        Self {
            window: None,
            renderer: None,
            camera,
            input: InputState::default(),
            last_frame: None,
            cursor_grabbed: false,
            settings_path,
            settings,
            show_debug,
            fps: 0.0,
            fps_frame_count: 0,
            fps_sample_start: None,
            render_time_seconds: 0.0,
            frame_time_samples_ms: VecDeque::with_capacity(FRAME_TIME_HISTORY_LEN),
            perf_log_timer_seconds: 0.0,
            pending_mesh_uploads: VecDeque::new(),
            last_upload_stats: UploadFrameStats::default(),
            last_render_stats: RenderFrameStats::default(),
            chunks: HashMap::new(),
            dirty_chunks: HashSet::new(),
            registry: None,
            selected_block: selected_block_from_inventory(&inventory, selected_hotbar_slot),
            inventory,
            armor_slots: [None; 4],
            selected_hotbar_slot,
            inventory_open: false,
            crafting_ui_mode: inventory::CraftingUiMode::Inventory2x2,
            inventory_crafting_slots: [None; 4],
            table_crafting_slots: [None; 9],
            chest_inventories: HashMap::new(),
            open_chest: None,
            double_chest_partner: None,
            double_chest_slots: None,
            cursor_stack: None,
            creative_search: String::new(),
            creative_scroll: 0,
            creative_catalog: Vec::new(),
            creative_catalog_full: Vec::new(),
            drag_distributing: false,
            drag_visited_slots: Vec::new(),
            drag_original_cursor_count: 0,
            last_inv_click_slot: None,
            last_inv_click_time: Instant::now(),
            inv_click_count: 0,
            player_pos: Vec3::new(0.0, 40.0, 0.0),
            velocity: Vec3::ZERO,
            on_ground: false,
            fly_mode: false,
            flight_stream_floor_y: None,
            play_mode: PlayMode::Survival,
            sprinting: false,
            health: MAX_HEALTH,
            hunger: MAX_HUNGER,
            xp_total: 0,
            xp_level: 0,
            air_supply: MAX_AIR_SUPPLY,
            fall_start_y: None,
            last_damage_time: -999.0,
            damage_flash_timer: 0.0,
            spawn_position: Vec3::new(0.0, 40.0, 0.0),
            block_physics_timer: 0.0,
            pending_fluid_positions: HashSet::new(),
            lava_simulation_frame_accumulator: 0,
            last_player_chunk: None,
            mesh_queue: VecDeque::new(),
            mesh_queue_set: HashSet::new(),
            mesh_jobs_in_flight: HashSet::new(),
            mesh_worker: None,
            mesh_versions: HashMap::new(),
            chunk_lods: HashMap::new(),
            targeted_block: None,
            breaking_block: None,
            break_progress: 0.0,
            chunk_request_tx: None,
            chunk_result_rx: None,
            pending_chunks: HashSet::new(),
            time_of_day: 0.5, // Start at noon
            time_frozen: false,
            weather_state: WeatherState::Clear,
            weather_timer: 420.0,
            weather_rng_state: (random_seed() ^ 0x7D4A_B3E1_91C2_5F0D).max(1),
            gameplay_rng_state: (random_seed() ^ 0x9C3A_52F1_D4BE_8A07).max(1),
            mobs: Vec::new(),
            mob_spawn_timer: 0.0,
            mob_spawn_cooldown: 5.0,
            block_scan_state: BlockScanState::default(),
            leaf_decay_timers: HashMap::new(),
            sapling_growth_timers: HashMap::new(),
            sugar_cane_growth_timers: HashMap::new(),
            world_seed: DEFAULT_WORLD_SEED,
            spawn_found: false,
            chunks_ready: false,
            app_state: AppState::MainMenu,
            in_game_menu: InGameMenuState::None,
            game_mode: None,
            menu_selected: 0,
            pause_menu_selected: 0,
            settings_menu_selected: 0,
            active_settings_slider: None,
            world_entries: Vec::new(),
            world_selected: None,
            world_create_form_open: false,
            world_create_name_input: String::new(),
            world_create_seed_input: String::new(),
            world_create_active_field: WorldCreateInputField::Name,
            world_create_play_mode: PlayMode::Survival,
            world_delete_confirmation_open: false,
            server_ip: "127.0.0.1:25565".to_string(),
            remote_players: Vec::new(),
            remote_player_states: HashMap::new(),
            remote_break_overlays: Vec::new(),
            attack_animation: 0.0,
            was_left_click_down: false,
            walk_particle_timer: 0.0,
            item_drops: Vec::new(),
            active_world_dir: None,
            active_world_name: None,
            persistence: None,
            last_save_time: None,
            my_player_id: None,
            cursor_position: None,
            console_open: false,
            text_input: String::new(),
            console_messages: VecDeque::new(),
            chat_open: false,
            chat_input: String::new(),
            chat_messages: VecDeque::new(),
            tnt_explosion_queue: VecDeque::new(),
            furnace_data: HashMap::new(),
            open_furnace: None,
            bed_spawn_point: None,
            button_timers: Vec::new(),
            active_pressure_plates: HashSet::new(),
        }
    }
}

impl ClientApp {
    fn set_cursor_grab(&mut self, enabled: bool) {
        let Some(window) = self.window.as_ref() else {
            self.cursor_grabbed = false;
            return;
        };

        let grabbed = if enabled {
            window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok()
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            false
        };

        if !enabled {
            self.input.left_click = false;
            self.breaking_block = None;
            self.break_progress = 0.0;
        }

        window.set_cursor_visible(!grabbed);
        self.cursor_grabbed = grabbed;
    }

    fn apply_settings(&mut self) {
        self.settings = std::mem::take(&mut self.settings).sanitize();
        self.camera.fov = self.settings.fov.to_radians();
        self.show_debug = self.settings.show_fps;
    }

    fn save_settings(&self) {
        if let Err(err) = self.settings.save(&self.settings_path) {
            warn!(
                "Failed to save settings to {}: {err}",
                self.settings_path.display()
            );
        }
    }

    fn set_render_distance(&mut self, render_distance: i32) {
        let new_value = render_distance.clamp(MIN_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
        if self.settings.render_distance == new_value {
            return;
        }
        self.settings.render_distance = new_value;
        self.last_player_chunk = None;
        self.save_settings();
    }

    fn set_stream_surface_below(&mut self, value: i32) {
        let new_value = value.clamp(MIN_STREAM_SURFACE_BELOW, MAX_STREAM_SURFACE_BELOW);
        if self.settings.stream_surface_below == new_value {
            return;
        }
        self.settings.stream_surface_below = new_value;
        if self.settings.stream_flight_below < new_value + 1 {
            self.settings.stream_flight_below = (new_value + 1)
                .clamp(MIN_STREAM_FLIGHT_BELOW, MAX_STREAM_FLIGHT_BELOW);
        }
        self.last_player_chunk = None;
        self.save_settings();
    }

    fn set_stream_flight_below(&mut self, value: i32) {
        let minimum = (self.settings.stream_surface_below + 1).max(MIN_STREAM_FLIGHT_BELOW);
        let new_value = value.clamp(minimum, MAX_STREAM_FLIGHT_BELOW);
        if self.settings.stream_flight_below == new_value {
            return;
        }
        self.settings.stream_flight_below = new_value;
        if self.fly_mode {
            self.flight_stream_floor_y = Some(self.player_chunk_pos().y - new_value);
        }
        self.last_player_chunk = None;
        self.save_settings();
    }

    fn set_stream_above(&mut self, value: i32) {
        let new_value = value.clamp(MIN_STREAM_ABOVE, MAX_STREAM_ABOVE);
        if self.settings.stream_above == new_value {
            return;
        }
        self.settings.stream_above = new_value;
        self.last_player_chunk = None;
        self.save_settings();
    }

    fn set_lod1_distance(&mut self, value: i32) {
        let new_value = value.clamp(MIN_LOD1_DISTANCE, MAX_LOD1_DISTANCE);
        if self.settings.lod1_distance == new_value {
            return;
        }
        self.settings.lod1_distance = new_value;
        let existing: Vec<ChunkPos> = self.chunks.keys().copied().collect();
        for pos in existing {
            if self.mesh_queue_set.insert(pos) {
                self.mesh_queue.push_back(pos);
            }
        }
        self.save_settings();
    }

    fn set_mouse_sensitivity(&mut self, sensitivity: f32) {
        let new_value = sensitivity.clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY);
        let rounded = (new_value * 100.0).round() / 100.0;
        if (self.settings.mouse_sensitivity - rounded).abs() < f32::EPSILON {
            return;
        }
        self.settings.mouse_sensitivity = rounded;
        self.save_settings();
    }

    fn set_fov(&mut self, fov: f32) {
        let new_value = fov.clamp(MIN_FOV, MAX_FOV).round();
        if (self.settings.fov - new_value).abs() < f32::EPSILON {
            return;
        }
        self.settings.fov = new_value;
        self.apply_settings();
        self.save_settings();
    }

    fn set_show_fps(&mut self, show: bool) {
        if self.settings.show_fps == show {
            return;
        }
        self.settings.show_fps = show;
        self.apply_settings();
        self.save_settings();
    }

    fn set_gui_scale(&mut self, gui_scale: f32) {
        let new_value = gui_scale.clamp(MIN_GUI_SCALE, MAX_GUI_SCALE);
        if (self.settings.gui_scale - new_value).abs() < f32::EPSILON {
            return;
        }
        self.settings.gui_scale = new_value;
        self.save_settings();
    }

    fn set_slider_from_fraction(&mut self, slider: SettingsSliderKind, value: f32) {
        let value = value.clamp(0.0, 1.0);
        match slider {
            SettingsSliderKind::RenderDistance => {
                let span = (MAX_RENDER_DISTANCE - MIN_RENDER_DISTANCE) as f32;
                let render_distance = MIN_RENDER_DISTANCE + (value * span).round() as i32;
                self.set_render_distance(render_distance);
            }
            SettingsSliderKind::SurfaceBelow => {
                let span = (MAX_STREAM_SURFACE_BELOW - MIN_STREAM_SURFACE_BELOW) as f32;
                let chunks = MIN_STREAM_SURFACE_BELOW + (value * span).round() as i32;
                self.set_stream_surface_below(chunks);
            }
            SettingsSliderKind::FlightBelow => {
                let span = (MAX_STREAM_FLIGHT_BELOW - MIN_STREAM_FLIGHT_BELOW) as f32;
                let chunks = MIN_STREAM_FLIGHT_BELOW + (value * span).round() as i32;
                self.set_stream_flight_below(chunks);
            }
            SettingsSliderKind::StreamAbove => {
                let span = (MAX_STREAM_ABOVE - MIN_STREAM_ABOVE) as f32;
                let chunks = MIN_STREAM_ABOVE + (value * span).round() as i32;
                self.set_stream_above(chunks);
            }
            SettingsSliderKind::LodDistance => {
                let span = (MAX_LOD1_DISTANCE - MIN_LOD1_DISTANCE) as f32;
                let chunks = MIN_LOD1_DISTANCE + (value * span).round() as i32;
                self.set_lod1_distance(chunks);
            }
            SettingsSliderKind::MouseSensitivity => {
                let span = MAX_MOUSE_SENSITIVITY - MIN_MOUSE_SENSITIVITY;
                self.set_mouse_sensitivity(MIN_MOUSE_SENSITIVITY + value * span);
            }
            SettingsSliderKind::Fov => {
                let span = MAX_FOV - MIN_FOV;
                self.set_fov(MIN_FOV + value * span);
            }
            SettingsSliderKind::GuiScale => {
                let span = MAX_GUI_SCALE - MIN_GUI_SCALE;
                self.set_gui_scale(MIN_GUI_SCALE + value * span);
            }
        }
    }

    fn clear_transient_input_state(&mut self) {
        self.input = InputState::default();
        self.breaking_block = None;
        self.break_progress = 0.0;
        self.was_left_click_down = false;
    }

    fn clear_gameplay_input_state(&mut self) {
        self.clear_transient_input_state();
        self.sprinting = false;
    }

    fn refresh_selected_block(&mut self) {
        self.selected_block =
            selected_block_from_inventory(&self.inventory, self.selected_hotbar_slot);
    }

    fn is_survival_mode(&self) -> bool {
        matches!(self.play_mode, PlayMode::Survival)
    }

    fn armor_slot_index(slot: ArmorSlot) -> usize {
        match slot {
            ArmorSlot::Helmet => 0,
            ArmorSlot::Chestplate => 1,
            ArmorSlot::Leggings => 2,
            ArmorSlot::Boots => 3,
        }
    }

    fn total_armor_defense_points(&self) -> u8 {
        self.armor_slots
            .iter()
            .flatten()
            .map(|stack| armor_defense_points(stack.item))
            .sum()
    }

    fn xp_for_level(level: u32) -> u32 {
        if level < 16 {
            level * level + 6 * level
        } else if level < 31 {
            (2.5 * level as f32 * level as f32 - 40.5 * level as f32 + 360.0) as u32
        } else {
            (4.5 * level as f32 * level as f32 - 162.5 * level as f32 + 2220.0) as u32
        }
    }

    fn recalculate_xp_level(&mut self) {
        self.xp_level = 0;
        while Self::xp_for_level(self.xp_level + 1) <= self.xp_total {
            self.xp_level += 1;
        }
    }

    fn add_xp(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.xp_total = self.xp_total.saturating_add(amount);
        self.recalculate_xp_level();
    }

    fn xp_progress(&self) -> f32 {
        let current_level_xp = Self::xp_for_level(self.xp_level);
        let next_level_xp = Self::xp_for_level(self.xp_level + 1);
        if next_level_xp <= current_level_xp {
            return 0.0;
        }
        (self.xp_total.saturating_sub(current_level_xp)) as f32
            / (next_level_xp - current_level_xp) as f32
    }

    fn item_label(&self, item: ItemId) -> String {
        if let Some(block) = item.as_block_id() {
            if let Some(registry) = self.registry.as_ref() {
                return registry.get_properties(block).name.clone();
            }
        }
        format!("item_{}", item.0)
    }

    fn is_creative_inventory_ui_active(&self) -> bool {
        self.inventory_open
            && self.open_chest.is_none()
            && self.open_furnace.is_none()
            && matches!(self.play_mode, PlayMode::Creative)
            && matches!(self.crafting_ui_mode, inventory::CraftingUiMode::Inventory2x2)
    }

    fn get_item_display_name(&self, item_id: ItemId) -> String {
        inventory::item_display_name(item_id, self.registry.as_deref())
    }

    fn rebuild_creative_catalog(&mut self) {
        if let Some(registry) = self.registry.as_deref() {
            self.creative_catalog_full = inventory::build_creative_catalog(registry);
            self.filter_creative_catalog();
        } else {
            self.creative_catalog_full.clear();
            self.creative_catalog.clear();
            self.creative_scroll = 0;
        }
    }

    fn filter_creative_catalog(&mut self) {
        let query = self.creative_search.to_lowercase();
        if query.is_empty() {
            self.creative_catalog = self.creative_catalog_full.clone();
        } else {
            let mut filtered = Vec::new();
            for item_id in self.creative_catalog_full.iter().copied() {
                if self
                    .get_item_display_name(item_id)
                    .to_lowercase()
                    .contains(&query)
                {
                    filtered.push(item_id);
                }
            }
            self.creative_catalog = filtered;
        }
        self.creative_scroll = 0;
    }

    fn append_creative_search_text(&mut self, text: &str) {
        let mut changed = false;
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            self.creative_search.push(ch);
            changed = true;
        }
        if changed {
            self.filter_creative_catalog();
        }
    }

    fn set_fly_mode(&mut self, enabled: bool) {
        self.fly_mode = enabled;
        if self.fly_mode {
            self.velocity = Vec3::ZERO;
            let current_chunk = self.player_chunk_pos();
            let initial_floor = current_chunk.y - self.settings.stream_flight_below;
            self.flight_stream_floor_y = Some(
                self.flight_stream_floor_y
                    .map_or(initial_floor, |existing| existing.min(initial_floor)),
            );
        } else {
            self.flight_stream_floor_y = None;
        }
        self.last_player_chunk = None;
    }

    fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
        match self.play_mode {
            PlayMode::Survival => {
                self.set_fly_mode(false);
            }
            PlayMode::Creative => {
                self.set_fly_mode(true);
                self.health = MAX_HEALTH;
                self.hunger = MAX_HUNGER;
                self.air_supply = MAX_AIR_SUPPLY;
                self.damage_flash_timer = 0.0;
                self.last_damage_time = -999.0;
                populate_creative_hotbar(&mut self.inventory);
            }
        }
        self.refresh_selected_block();
    }

    fn reset_weather_state(&mut self) {
        self.weather_state = WeatherState::Clear;
        self.weather_rng_state ^= self.world_seed ^ 0xA9F1_42C3_6ED8_119B;
        if self.weather_rng_state == 0 {
            self.weather_rng_state = 0x9E37_79B9_7F4A_7C15;
        }
        self.weather_timer =
            self.weather_rand_range(WEATHER_MIN_DURATION_SECS, WEATHER_MAX_DURATION_SECS);
    }

    fn weather_dim_amount(&self) -> f32 {
        match self.weather_state {
            WeatherState::Clear => 0.0,
            WeatherState::Rain | WeatherState::Snow => 1.0,
        }
    }

    fn next_weather_rand_u32(&mut self) -> u32 {
        let mut x = self.weather_rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.weather_rng_state = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D)) >> 32) as u32
    }

    fn weather_rand_f32(&mut self) -> f32 {
        self.next_weather_rand_u32() as f32 / u32::MAX as f32
    }

    fn weather_rand_range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.weather_rand_f32()
    }

    fn weather_random_count(&mut self, min: usize, max: usize) -> usize {
        if max <= min {
            return min;
        }
        min + (self.next_weather_rand_u32() as usize % (max - min + 1))
    }

    fn reset_growth_state(&mut self) {
        self.block_scan_state = BlockScanState::default();
        self.leaf_decay_timers.clear();
        self.sapling_growth_timers.clear();
        self.sugar_cane_growth_timers.clear();
        self.tnt_explosion_queue.clear();
        self.button_timers.clear();
        self.active_pressure_plates.clear();
        self.gameplay_rng_state = (random_seed() ^ self.world_seed ^ 0xC6E7_4B39_18F2_DA41).max(1);
    }

    fn next_gameplay_rand_u32(&mut self) -> u32 {
        let mut x = self.gameplay_rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.gameplay_rng_state = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D)) >> 32) as u32
    }

    fn gameplay_rand_f32(&mut self) -> f32 {
        self.next_gameplay_rand_u32() as f32 / u32::MAX as f32
    }

    fn gameplay_rand_range(&mut self, min: f32, max: f32) -> f32 {
        if max <= min {
            return min;
        }
        min + (max - min) * self.gameplay_rand_f32()
    }

    fn tick_weather(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        if dt == 0.0 {
            return;
        }

        self.weather_timer -= dt;
        if self.weather_timer <= 0.0 {
            let precipitation = self.weather_rand_f32() < 0.62;
            if precipitation {
                self.weather_state =
                    if self.is_snowy_precipitation_biome(self.player_pos.x, self.player_pos.z) {
                        WeatherState::Snow
                    } else {
                        WeatherState::Rain
                    };
            } else {
                self.weather_state = WeatherState::Clear;
            }
            self.weather_timer =
                self.weather_rand_range(WEATHER_MIN_DURATION_SECS, WEATHER_MAX_DURATION_SECS);
        }

        if matches!(self.weather_state, WeatherState::Rain | WeatherState::Snow) {
            self.weather_state = if self.is_snowy_precipitation_biome(self.player_pos.x, self.player_pos.z) {
                WeatherState::Snow
            } else {
                WeatherState::Rain
            };
        }
    }

    fn is_snowy_precipitation_biome(&self, world_x: f32, world_z: f32) -> bool {
        let biome_noise = Perlin::new(self.world_seed.wrapping_add(3) as u32);
        let wx = world_x as f64;
        let wz = world_z as f64;
        biome_noise.get([wx * 0.002, wz * 0.002]) < WEATHER_SNOW_TEMP_THRESHOLD
    }

    fn generate_weather_spawn_candidates(
        &mut self,
        candidate_count: usize,
        min_height: f32,
        max_height: f32,
    ) -> Vec<Vec3> {
        let mut candidates = Vec::with_capacity(candidate_count);
        let center = self.camera.position;
        for _ in 0..candidate_count {
            let angle = self.weather_rand_range(0.0, std::f32::consts::TAU);
            let distance = WEATHER_SPAWN_RADIUS * self.weather_rand_f32().sqrt();
            let x = center.x + distance * angle.cos();
            let z = center.z + distance * angle.sin();
            let y = center.y + self.weather_rand_range(min_height, max_height);
            candidates.push(Vec3::new(x, y, z));
        }
        candidates
    }

    fn can_spawn_weather_at(&self, spawn_pos: Vec3, registry: &BlockRegistry) -> bool {
        let block_pos = IVec3::new(
            spawn_pos.x.floor() as i32,
            spawn_pos.y.floor() as i32,
            spawn_pos.z.floor() as i32,
        );
        if is_block_solid(
            block_pos.x,
            block_pos.y,
            block_pos.z,
            &self.chunks,
            registry,
        ) {
            return false;
        }
        for y in (block_pos.y + 1)..=(block_pos.y + WEATHER_OCCLUSION_CHECK_HEIGHT) {
            if is_block_solid(block_pos.x, y, block_pos.z, &self.chunks, registry) {
                return false;
            }
        }
        true
    }

    fn find_adjacent_chest_partner(&self, chest_pos: IVec3) -> Option<IVec3> {
        const OFFSETS: [IVec3; 4] = [
            IVec3::new(1, 0, 0),
            IVec3::new(-1, 0, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(0, 0, -1),
        ];
        OFFSETS
            .into_iter()
            .map(|offset| chest_pos + offset)
            .find(|candidate| block_at(*candidate, &self.chunks) == Some(BlockId::CHEST))
    }

    fn persist_open_double_chest(&mut self) {
        let (Some(primary_pos), Some(partner_pos), Some(slots)) = (
            self.open_chest,
            self.double_chest_partner,
            self.double_chest_slots,
        ) else {
            return;
        };

        let primary_inventory = self
            .chest_inventories
            .entry(primary_pos)
            .or_insert_with(Inventory::new);
        primary_inventory.slots[..SINGLE_CHEST_SLOT_COUNT]
            .clone_from_slice(&slots[..SINGLE_CHEST_SLOT_COUNT]);

        let partner_inventory = self
            .chest_inventories
            .entry(partner_pos)
            .or_insert_with(Inventory::new);
        partner_inventory.slots[..SINGLE_CHEST_SLOT_COUNT]
            .clone_from_slice(&slots[SINGLE_CHEST_SLOT_COUNT..DOUBLE_CHEST_SLOT_COUNT]);
    }

    fn close_open_chest_ui(&mut self) {
        self.persist_open_double_chest();
        self.open_chest = None;
        self.double_chest_partner = None;
        self.double_chest_slots = None;
    }

    fn set_inventory_open(&mut self, open: bool) {
        if !matches!(self.app_state, AppState::InGame) {
            self.inventory_open = false;
            self.close_open_chest_ui();
            self.reset_inventory_drag_distribution();
            return;
        }
        if !open {
            self.close_open_chest_ui();
            self.reset_inventory_drag_distribution();
        }
        if self.inventory_open == open {
            return;
        }
        self.inventory_open = open;
        self.clear_gameplay_input_state();
        self.set_cursor_grab(!open);
    }

    fn reset_inventory_drag_distribution(&mut self) {
        self.drag_distributing = false;
        self.drag_visited_slots.clear();
        self.drag_original_cursor_count = 0;
    }

    fn try_begin_inventory_drag_distribution(&mut self, slot_index: usize) -> bool {
        if slot_index >= Inventory::TOTAL_SIZE {
            return false;
        }
        let Some(cursor) = self.cursor_stack else {
            return false;
        };
        if cursor.count == 0 || cursor.item.is_tool() {
            return false;
        }
        self.drag_distributing = true;
        self.drag_original_cursor_count = cursor.count;
        self.drag_visited_slots.clear();
        self.drag_visited_slots.push(slot_index);
        true
    }

    fn track_inventory_drag_slot(&mut self, slot_index: usize) {
        if !self.drag_distributing || slot_index >= Inventory::TOTAL_SIZE {
            return;
        }
        if !self.drag_visited_slots.contains(&slot_index) {
            self.drag_visited_slots.push(slot_index);
        }
    }

    fn finish_inventory_drag_distribution(&mut self) {
        if !self.drag_distributing {
            self.reset_inventory_drag_distribution();
            return;
        }
        let visited_slots = self.drag_visited_slots.clone();
        let Ok(visited_count) = u8::try_from(visited_slots.len()) else {
            self.reset_inventory_drag_distribution();
            return;
        };
        if visited_count == 0 {
            self.reset_inventory_drag_distribution();
            return;
        }
        if visited_count == 1 {
            self.handle_inventory_slot_click(visited_slots[0]);
            self.reset_inventory_drag_distribution();
            return;
        }
        let Some(cursor) = self.cursor_stack else {
            self.reset_inventory_drag_distribution();
            return;
        };
        if cursor.count == 0 || cursor.item.is_tool() {
            self.reset_inventory_drag_distribution();
            return;
        }

        let item = cursor.item;
        let stack_limit = max_stack_for_item(item);
        let distributable_total = self.drag_original_cursor_count.min(cursor.count);
        let mut remaining_to_place = distributable_total;
        let base_remaining = cursor.count.saturating_sub(distributable_total);
        let items_per_slot = distributable_total / visited_count;

        for slot_index in visited_slots {
            if slot_index >= Inventory::TOTAL_SIZE {
                continue;
            }
            let slot = &mut self.inventory.slots[slot_index];
            let moved = match slot.as_mut() {
                None => {
                    let moved = items_per_slot.min(stack_limit);
                    if moved > 0 {
                        *slot = Some(ItemStack::new(item, moved));
                    }
                    moved
                }
                Some(target) if target.item == item => {
                    let space = stack_limit.saturating_sub(target.count);
                    let moved = items_per_slot.min(space);
                    target.count += moved;
                    moved
                }
                _ => 0,
            };
            remaining_to_place = remaining_to_place.saturating_sub(moved);
        }

        let cursor_remaining = base_remaining.saturating_add(remaining_to_place);
        if cursor_remaining == 0 {
            self.cursor_stack = None;
        } else {
            self.cursor_stack = Some(ItemStack::new(item, cursor_remaining));
        }
        self.refresh_selected_block();
        self.reset_inventory_drag_distribution();
    }

    /// Triple-click: gather all matching items from inventory into the clicked slot.
    fn gather_matching_items_to_slot(&mut self, target_slot: usize) {
        if target_slot >= Inventory::TOTAL_SIZE {
            return;
        }
        let Some(target_stack) = self.inventory.slots[target_slot] else {
            return;
        };
        let item = target_stack.item;
        if item.is_tool() {
            return;
        }
        let stack_limit = max_stack_for_item(item);
        let mut total = target_stack.count;

        for i in 0..Inventory::TOTAL_SIZE {
            if i == target_slot || total >= stack_limit {
                continue;
            }
            if let Some(ref other) = self.inventory.slots[i] {
                if other.item == item && !other.item.is_tool() {
                    let take = other.count.min(stack_limit - total);
                    total += take;
                    if take == other.count {
                        self.inventory.slots[i] = None;
                    } else if let Some(ref mut s) = self.inventory.slots[i] {
                        s.count -= take;
                    }
                }
            }
        }
        if let Some(ref mut s) = self.inventory.slots[target_slot] {
            s.count = total;
        }
        self.refresh_selected_block();
    }

    fn open_inventory_ui(&mut self, mode: inventory::CraftingUiMode) {
        self.close_open_chest_ui();
        self.crafting_ui_mode = mode;
        self.set_inventory_open(true);
    }

    fn open_chest_ui(&mut self, chest_pos: IVec3) {
        self.close_open_chest_ui();
        self.chest_inventories
            .entry(chest_pos)
            .or_insert_with(Inventory::new);
        self.open_chest = Some(chest_pos);

        if let Some(partner_pos) = self.find_adjacent_chest_partner(chest_pos) {
            self.chest_inventories
                .entry(partner_pos)
                .or_insert_with(Inventory::new);
            self.double_chest_partner = Some(partner_pos);
            let mut slots = [None; DOUBLE_CHEST_SLOT_COUNT];
            if let Some(primary_inventory) = self.chest_inventories.get(&chest_pos) {
                slots[..SINGLE_CHEST_SLOT_COUNT]
                    .clone_from_slice(&primary_inventory.slots[..SINGLE_CHEST_SLOT_COUNT]);
            }
            if let Some(partner_inventory) = self.chest_inventories.get(&partner_pos) {
                slots[SINGLE_CHEST_SLOT_COUNT..DOUBLE_CHEST_SLOT_COUNT]
                    .clone_from_slice(&partner_inventory.slots[..SINGLE_CHEST_SLOT_COUNT]);
            }
            self.double_chest_slots = Some(slots);
        } else {
            self.double_chest_partner = None;
            self.double_chest_slots = None;
        }
        self.set_inventory_open(true);
    }

    fn toggle_inventory(&mut self) {
        if self.inventory_open {
            self.set_inventory_open(false);
        } else {
            self.open_inventory_ui(inventory::CraftingUiMode::Inventory2x2);
        }
    }

    fn toggle_world_create_play_mode(&mut self) {
        self.world_create_play_mode = match self.world_create_play_mode {
            PlayMode::Survival => PlayMode::Creative,
            PlayMode::Creative => PlayMode::Survival,
        };
    }

    fn inventory_armor_slot_hit_test(
        &self,
        cursor_x: f32,
        cursor_y: f32,
        width: u32,
        height: u32,
    ) -> Option<usize> {
        if self.open_chest.is_some() {
            return None;
        }

        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let slot_size = (screen_w.min(screen_h) * 0.07)
            .clamp(INVENTORY_SLOT_SIZE_MIN_PX, INVENTORY_SLOT_SIZE_MAX_PX);
        let slot_gap = (slot_size * 0.14).round().max(INVENTORY_SLOT_GAP_MIN_PX);

        let inv_grid_w = slot_size * 9.0 + slot_gap * 8.0;
        let inv_grid_h = slot_size * 4.0 + slot_gap * 3.0;
        let craft_side = match self.crafting_ui_mode {
            inventory::CraftingUiMode::Inventory2x2 => 2,
            inventory::CraftingUiMode::CraftingTable3x3 => 3,
        };
        let craft_grid_w = slot_size * craft_side as f32 + slot_gap * (craft_side as f32 - 1.0);
        let craft_grid_h = craft_grid_w;
        let craft_section_w = craft_grid_w + slot_gap * 2.0 + slot_size;
        let panel_w = inv_grid_w + craft_section_w + 72.0;
        let panel_h = (inv_grid_h + 94.0).max(craft_grid_h + 110.0);
        let panel_x = (screen_w - panel_w) * 0.5;
        let panel_y = (screen_h - panel_h) * 0.5;
        let inv_grid_x = panel_x + 24.0;
        let armor_x = inv_grid_x - slot_size - INVENTORY_ARMOR_COLUMN_OFFSET_PX;
        let armor_y = panel_y + 54.0;

        for idx in 0..4 {
            let y = armor_y + idx as f32 * (slot_size + slot_gap);
            let inside_x = cursor_x >= armor_x && cursor_x <= armor_x + slot_size;
            let inside_y = cursor_y >= y && cursor_y <= y + slot_size;
            if inside_x && inside_y {
                return Some(idx);
            }
        }
        None
    }

    fn handle_armor_slot_click(&mut self, slot_index: usize) {
        let Some(stack) = self.armor_slots.get_mut(slot_index).and_then(Option::take) else {
            return;
        };
        let remaining = self.inventory.add_item(stack.item, stack.count);
        if remaining > 0 {
            self.armor_slots[slot_index] = Some(ItemStack {
                count: remaining,
                ..stack
            });
        }
        self.refresh_selected_block();
    }

    fn try_swap_inventory_slot_with_armor(&mut self, slot_index: usize) -> bool {
        if self.cursor_stack.is_some() {
            return false;
        }
        let Some(item) = self.inventory.slots[slot_index].as_ref().map(|stack| stack.item) else {
            return false;
        };
        let Some(armor_slot) = armor_slot_for_item(item) else {
            return false;
        };
        let armor_index = Self::armor_slot_index(armor_slot);
        std::mem::swap(
            &mut self.armor_slots[armor_index],
            &mut self.inventory.slots[slot_index],
        );
        self.refresh_selected_block();
        true
    }

    fn handle_inventory_slot_click(&mut self, slot_index: usize) {
        if slot_index >= Inventory::TOTAL_SIZE {
            return;
        }

        if self.try_swap_inventory_slot_with_armor(slot_index) {
            return;
        }

        swap_or_merge_slot_with_cursor(&mut self.inventory.slots[slot_index], &mut self.cursor_stack);
        self.refresh_selected_block();
    }

    fn handle_inventory_slot_right_click(&mut self, slot_index: usize) {
        if slot_index >= Inventory::TOTAL_SIZE {
            return;
        }

        if self.try_swap_inventory_slot_with_armor(slot_index) {
            return;
        }

        right_click_slot(&mut self.inventory.slots[slot_index], &mut self.cursor_stack);
        self.refresh_selected_block();
    }

    fn handle_chest_slot_interaction(&mut self, slot_index: usize, right_click: bool) {
        let slot_limit = if self.double_chest_slots.is_some() {
            DOUBLE_CHEST_SLOT_COUNT
        } else {
            SINGLE_CHEST_SLOT_COUNT
        };
        if slot_index >= slot_limit || self.open_chest.is_none() {
            return;
        }

        if let Some(slots) = self.double_chest_slots.as_mut() {
            if right_click {
                right_click_slot(&mut slots[slot_index], &mut self.cursor_stack);
            } else {
                swap_or_merge_slot_with_cursor(&mut slots[slot_index], &mut self.cursor_stack);
            }
            return;
        }

        let chest_pos = self.open_chest.expect("checked above");
        let chest_inventory = self
            .chest_inventories
            .entry(chest_pos)
            .or_insert_with(Inventory::new);
        if right_click {
            right_click_slot(
                &mut chest_inventory.slots[slot_index],
                &mut self.cursor_stack,
            );
        } else {
            swap_or_merge_slot_with_cursor(
                &mut chest_inventory.slots[slot_index],
                &mut self.cursor_stack,
            );
        }
    }

    fn handle_chest_slot_click(&mut self, slot_index: usize) {
        self.handle_chest_slot_interaction(slot_index, false);
    }

    fn handle_chest_slot_right_click(&mut self, slot_index: usize) {
        self.handle_chest_slot_interaction(slot_index, true);
    }

    fn handle_crafting_input_slot_click(&mut self, input_idx: usize) {
        let cursor = &mut self.cursor_stack;
        match self.crafting_ui_mode {
            inventory::CraftingUiMode::Inventory2x2 => {
                if input_idx >= self.inventory_crafting_slots.len() {
                    return;
                }
                swap_or_merge_slot_with_cursor(&mut self.inventory_crafting_slots[input_idx], cursor);
            }
            inventory::CraftingUiMode::CraftingTable3x3 => {
                if input_idx >= self.table_crafting_slots.len() {
                    return;
                }
                swap_or_merge_slot_with_cursor(&mut self.table_crafting_slots[input_idx], cursor);
            }
        }
    }

    fn handle_crafting_input_slot_right_click(&mut self, input_idx: usize) {
        let cursor = &mut self.cursor_stack;
        match self.crafting_ui_mode {
            inventory::CraftingUiMode::Inventory2x2 => {
                if input_idx >= self.inventory_crafting_slots.len() {
                    return;
                }
                right_click_slot(&mut self.inventory_crafting_slots[input_idx], cursor);
            }
            inventory::CraftingUiMode::CraftingTable3x3 => {
                if input_idx >= self.table_crafting_slots.len() {
                    return;
                }
                right_click_slot(&mut self.table_crafting_slots[input_idx], cursor);
            }
        }
    }

    fn active_crafting_result(&self) -> Option<ItemStack> {
        match self.crafting_ui_mode {
            inventory::CraftingUiMode::Inventory2x2 => {
                recipe::match_inventory_crafting(&self.inventory_crafting_slots)
            }
            inventory::CraftingUiMode::CraftingTable3x3 => {
                recipe::match_crafting_table(&self.table_crafting_slots)
            }
        }
    }

    fn consume_one_from_active_crafting_inputs(&mut self) {
        let slots: &mut [Option<ItemStack>] = match self.crafting_ui_mode {
            inventory::CraftingUiMode::Inventory2x2 => &mut self.inventory_crafting_slots,
            inventory::CraftingUiMode::CraftingTable3x3 => &mut self.table_crafting_slots,
        };

        for slot in slots {
            let Some(stack) = slot.as_mut() else {
                continue;
            };
            if stack.count == 0 {
                *slot = None;
                continue;
            }
            stack.count -= 1;
            if stack.count == 0 {
                *slot = None;
            }
        }
    }

    fn handle_crafting_output_click(&mut self) {
        let Some(mut result) = self.active_crafting_result() else {
            return;
        };
        Self::initialize_tool_durability(&mut result);

        if let Some(cursor) = self.cursor_stack.as_ref() {
            if cursor.item != result.item || cursor.durability != result.durability {
                return;
            }
            let space = max_stack_for_item(cursor.item).saturating_sub(cursor.count);
            if result.count > space {
                return;
            }
        }

        self.consume_one_from_active_crafting_inputs();
        match self.cursor_stack.as_mut() {
            Some(cursor) => {
                cursor.count += result.count;
            }
            None => {
                self.cursor_stack = Some(result);
            }
        }
    }

    fn initialize_tool_durability(stack: &mut ItemStack) {
        if let Some((_, tier)) = tool_properties(stack.item) {
            if stack.durability.is_none() {
                stack.durability = Some(tool_max_durability(tier));
            }
        }
    }

    fn consume_held_tool_durability(&mut self, required_kind: Option<ToolKind>) {
        let slot_idx = self.selected_hotbar_slot;
        let mut broke = false;

        let Some(slot) = self.inventory.slots.get_mut(slot_idx) else {
            return;
        };
        let Some(mut stack) = slot.take() else {
            return;
        };

        if let Some((kind, tier)) = tool_properties(stack.item) {
            if required_kind.is_some_and(|required| required != kind) {
                *slot = Some(stack);
                return;
            }
            let remaining = stack.durability.get_or_insert(tool_max_durability(tier));
            if *remaining <= 1 {
                broke = true;
            } else {
                *remaining -= 1;
                *slot = Some(stack);
            }
        } else {
            *slot = Some(stack);
        }

        if broke {
            self.push_chat_message("System", "Tool broke!");
        }
    }

    fn consume_held_tool_durability_on_break(&mut self) {
        self.consume_held_tool_durability(None);
    }

    fn consume_held_tool_durability_on_use(&mut self, kind: ToolKind) {
        self.consume_held_tool_durability(Some(kind));
    }

    fn is_in_game_menu_open(&self) -> bool {
        !matches!(self.in_game_menu, InGameMenuState::None)
    }

    fn open_pause_menu(&mut self) {
        if !matches!(self.app_state, AppState::InGame) {
            return;
        }
        if self.inventory_open {
            self.set_inventory_open(false);
        }
        self.in_game_menu = InGameMenuState::Pause;
        self.pause_menu_selected = 0;
        self.settings_menu_selected = 0;
        self.active_settings_slider = None;
        if self.console_open {
            self.set_console_open(false);
        }
        if self.chat_open {
            self.set_chat_open(false);
        }
        self.clear_gameplay_input_state();
        self.set_cursor_grab(false);
    }

    fn resume_gameplay(&mut self) {
        self.in_game_menu = InGameMenuState::None;
        self.active_settings_slider = None;
        self.clear_gameplay_input_state();
        self.set_cursor_grab(!self.inventory_open);
    }

    fn open_settings_menu(&mut self) {
        self.in_game_menu = InGameMenuState::Settings;
        self.settings_menu_selected = 0;
        self.active_settings_slider = None;
        self.clear_gameplay_input_state();
        self.set_cursor_grab(false);
    }

    fn close_settings_menu(&mut self) {
        self.in_game_menu = InGameMenuState::Pause;
        self.pause_menu_selected = 1;
        self.active_settings_slider = None;
        self.clear_gameplay_input_state();
    }

    fn apply_pause_menu_hit(&mut self, hit: PauseMenuHitTarget) {
        match hit {
            PauseMenuHitTarget::Resume => self.resume_gameplay(),
            PauseMenuHitTarget::Settings => self.open_settings_menu(),
            PauseMenuHitTarget::SaveAndQuit => self.return_to_menu(),
        }
    }

    fn apply_settings_target(&mut self, target: SettingsHitTarget) {
        match target {
            SettingsHitTarget::Slider(slider, value) => self.set_slider_from_fraction(slider, value),
            SettingsHitTarget::ShowFpsToggle => self.set_show_fps(!self.settings.show_fps),
            SettingsHitTarget::Back => self.close_settings_menu(),
        }
    }

    fn handle_pause_menu_keyboard(&mut self, code: KeyCode) {
        match code {
            KeyCode::Escape => self.resume_gameplay(),
            KeyCode::ArrowUp => {
                self.pause_menu_selected = self.pause_menu_selected.saturating_sub(1);
            }
            KeyCode::ArrowDown => {
                self.pause_menu_selected = (self.pause_menu_selected + 1).min(2);
            }
            KeyCode::Enter | KeyCode::Space => match self.pause_menu_selected {
                0 => self.resume_gameplay(),
                1 => self.open_settings_menu(),
                2 => self.return_to_menu(),
                _ => {}
            },
            _ => {}
        }
    }

    fn adjust_selected_setting(&mut self, direction: f32) {
        match SettingsMenuItem::from_index(self.settings_menu_selected) {
            SettingsMenuItem::RenderDistance => {
                self.set_render_distance(self.settings.render_distance + direction as i32);
            }
            SettingsMenuItem::SurfaceBelow => {
                self.set_stream_surface_below(self.settings.stream_surface_below + direction as i32);
            }
            SettingsMenuItem::FlightBelow => {
                self.set_stream_flight_below(self.settings.stream_flight_below + direction as i32);
            }
            SettingsMenuItem::StreamAbove => {
                self.set_stream_above(self.settings.stream_above + direction as i32);
            }
            SettingsMenuItem::LodDistance => {
                self.set_lod1_distance(self.settings.lod1_distance + direction as i32);
            }
            SettingsMenuItem::MouseSensitivity => {
                self.set_mouse_sensitivity(self.settings.mouse_sensitivity + direction * 0.1);
            }
            SettingsMenuItem::Fov => {
                self.set_fov(self.settings.fov + direction);
            }
            SettingsMenuItem::GuiScale => {
                self.set_gui_scale(self.settings.gui_scale + direction * 0.5);
            }
            SettingsMenuItem::ShowFps => self.set_show_fps(!self.settings.show_fps),
            SettingsMenuItem::Back => {}
        }
    }

    fn handle_settings_menu_keyboard(&mut self, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_settings_menu(),
            KeyCode::ArrowUp => {
                self.settings_menu_selected = self.settings_menu_selected.saturating_sub(1);
            }
            KeyCode::ArrowDown => {
                self.settings_menu_selected = (self.settings_menu_selected + 1).min(9);
            }
            KeyCode::ArrowLeft => self.adjust_selected_setting(-1.0),
            KeyCode::ArrowRight => self.adjust_selected_setting(1.0),
            KeyCode::Enter | KeyCode::Space => match SettingsMenuItem::from_index(self.settings_menu_selected) {
                SettingsMenuItem::ShowFps => self.set_show_fps(!self.settings.show_fps),
                SettingsMenuItem::Back => self.close_settings_menu(),
                _ => {}
            },
            _ => {}
        }
    }

    fn push_console_message(&mut self, message: impl Into<String>) {
        let message = message.into();
        info!("[console] {message}");
        self.console_messages.push_back(message);
        while self.console_messages.len() > MAX_CONSOLE_MESSAGES {
            self.console_messages.pop_front();
        }
    }

    fn set_console_open(&mut self, open: bool) {
        if open {
            self.chat_open = false;
            self.chat_input.clear();
            if self.inventory_open {
                self.set_inventory_open(false);
            }
        }
        self.console_open = open;
        self.clear_transient_input_state();
        if !open {
            self.text_input.clear();
        }
    }

    fn toggle_console(&mut self) {
        self.set_console_open(!self.console_open);
        if self.console_open {
            self.push_console_message("Console opened");
        }
    }

    fn append_console_text(&mut self, text: &str) {
        for ch in text.chars() {
            if ch.is_ascii_graphic() || ch == ' ' {
                self.text_input.push(ch);
            }
        }
    }

    fn set_chat_open(&mut self, open: bool) {
        if open {
            self.console_open = false;
            self.text_input.clear();
            if self.inventory_open {
                self.set_inventory_open(false);
            }
        }
        self.chat_open = open;
        self.clear_transient_input_state();
        if !open {
            self.chat_input.clear();
        }
    }

    fn append_chat_text(&mut self, text: &str) {
        for ch in text.chars() {
            if !(ch.is_ascii_graphic() || ch == ' ') {
                continue;
            }
            if self.chat_input.len() >= CHAT_INPUT_MAX_LEN {
                break;
            }
            self.chat_input.push(ch);
        }
    }

    fn push_chat_message(&mut self, sender_name: &str, message: &str) {
        let sender = sender_name.trim();
        let body = message.trim();
        if body.is_empty() {
            return;
        }
        let text = if sender.is_empty() {
            body.to_string()
        } else {
            format!("<{sender}> {body}")
        };
        self.chat_messages.push_back(ChatMessage {
            text,
            received_at: Instant::now(),
        });
        while self.chat_messages.len() > MAX_CHAT_MESSAGES {
            self.chat_messages.pop_front();
        }
    }

    fn submit_chat_input(&mut self) {
        let message = std::mem::take(&mut self.chat_input);
        let message = message.trim();
        if message.is_empty() {
            self.set_chat_open(false);
            return;
        }

        if matches!(self.game_mode, Some(GameMode::Singleplayer)) && message.starts_with('/') {
            self.handle_singleplayer_command(message);
        } else if let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode {
            net.send_reliable(&C2S::Chat {
                message: message.to_string(),
            });
        } else {
            self.push_chat_message("System", "Chat is only available in multiplayer.");
        }

        self.set_chat_open(false);
    }

    fn handle_singleplayer_command(&mut self, command: &str) {
        self.run_local_command(command, CommandFeedbackTarget::Chat);
    }

    fn push_command_feedback(&mut self, target: CommandFeedbackTarget, message: impl Into<String>) {
        let message = message.into();
        match target {
            CommandFeedbackTarget::Console => self.push_console_message(message),
            CommandFeedbackTarget::Chat => self.push_chat_message("System", &message),
        }
    }

    fn push_command_help(&mut self, target: CommandFeedbackTarget, page: usize) {
        match page {
            1 => {
                self.push_command_feedback(target, "Commands 1/5 (core)");
                self.push_command_feedback(target, "/help [page], /commands");
                self.push_command_feedback(target, "/tp <x> <y> <z>  (supports ~relative)");
                self.push_command_feedback(target, "/pos, /whereami, /chunk_info, /seed");
                self.push_command_feedback(target, "/gamemode creative|survival, /gmc, /gms");
                self.push_command_feedback(target, "/fly [on|off|toggle]");
                self.push_command_feedback(target, "/setspawn, /spawn, /home");
            }
            2 => {
                self.push_command_feedback(target, "Commands 2/5 (time + weather)");
                self.push_command_feedback(target, "/time query");
                self.push_command_feedback(target, "/time set <0..1|day|sunrise|noon|sunset|night>");
                self.push_command_feedback(target, "/time add <delta>");
                self.push_command_feedback(target, "/time freeze [on|off|toggle]");
                self.push_command_feedback(target, "/day, /sunrise, /noon, /sunset, /night");
                self.push_command_feedback(target, "/weather clear|rain|snow|cycle|query");
            }
            3 => {
                self.push_command_feedback(target, "Commands 3/5 (player + inventory)");
                self.push_command_feedback(target, "/heal [amount], /damage <amount>");
                self.push_command_feedback(target, "/feed [amount], /air [amount|full]");
                self.push_command_feedback(target, "/xp add <n>, /xp set <n>, /xp query");
                self.push_command_feedback(target, "/give <item_name> [count]");
                self.push_command_feedback(target, "/clearinventory, /clearhotbar");
                self.push_command_feedback(target, "/cleararmor, /clearcursor, /clearall, /kill");
            }
            4 => {
                self.push_command_feedback(target, "Commands 4/5 (settings)");
                self.push_command_feedback(target, "/fov <60..120>");
                self.push_command_feedback(target, "/gui <1.0..3.0> (/guiscale)");
                self.push_command_feedback(target, "/renderdistance <4..24> (/rd)");
                self.push_command_feedback(target, "/lod <4..14> (/lod1)");
                self.push_command_feedback(target, "/mouse <0.5..5.0> (/sensitivity)");
                self.push_command_feedback(target, "/showfps [on|off|toggle|query], /fps ...");
            }
            5 => {
                self.push_command_feedback(target, "Commands 5/5 (session)");
                self.push_command_feedback(target, "/save, /savefull");
                self.push_command_feedback(target, "/connect <ip:port>, /disconnect");
                self.push_command_feedback(target, "Tip: this build now has 50+ command features.");
            }
            _ => {
                self.push_command_feedback(target, "Usage: /help [1..5]");
            }
        }
    }

    fn weather_name(state: WeatherState) -> &'static str {
        match state {
            WeatherState::Clear => "clear",
            WeatherState::Rain => "rain",
            WeatherState::Snow => "snow",
        }
    }

    fn parse_toggle_argument(value: Option<&str>, current: bool) -> Option<bool> {
        match value {
            None => Some(!current),
            Some(raw) => match raw.to_ascii_lowercase().as_str() {
                "on" | "true" | "1" | "yes" => Some(true),
                "off" | "false" | "0" | "no" => Some(false),
                "toggle" => Some(!current),
                _ => None,
            },
        }
    }

    fn parse_tp_coordinate(base: f32, raw: &str) -> Option<f32> {
        if let Some(rest) = raw.strip_prefix('~') {
            if rest.is_empty() {
                Some(base)
            } else {
                rest.parse::<f32>().ok().map(|delta| base + delta)
            }
        } else {
            raw.parse::<f32>().ok()
        }
    }

    fn parse_time_token(raw: &str) -> Option<f32> {
        match raw.to_ascii_lowercase().as_str() {
            "day" => Some(0.25),
            "sunrise" => Some(0.20),
            "noon" => Some(0.50),
            "sunset" => Some(0.75),
            "night" => Some(0.0),
            _ => raw.parse::<f32>().ok().filter(|value| (0.0..=1.0).contains(value)),
        }
    }

    fn set_weather_state(&mut self, state: WeatherState) {
        self.weather_state = state;
        self.weather_timer = self.weather_rand_range(WEATHER_MIN_DURATION_SECS, WEATHER_MAX_DURATION_SECS);
    }

    fn set_xp_total(&mut self, amount: u32) {
        self.xp_total = amount;
        self.recalculate_xp_level();
    }

    fn teleport_player(&mut self, x: f32, y: f32, z: f32) {
        self.player_pos = Vec3::new(x, y, z);
        self.velocity = Vec3::ZERO;
        self.camera.position = self.player_pos + Vec3::new(0.0, EYE_HEIGHT, 0.0);
        if self.fly_mode {
            self.flight_stream_floor_y = Some(self.player_chunk_pos().y - self.settings.stream_flight_below);
        }
        self.last_player_chunk = None;
        self.chunks_ready = false;
        self.spawn_found = true;
    }

    fn run_local_command(&mut self, raw: &str, target: CommandFeedbackTarget) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let cmdline = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let mut parts = cmdline.split_whitespace();
        let Some(command) = parts.next().map(|part| part.to_ascii_lowercase()) else {
            return;
        };
        let args: Vec<&str> = parts.collect();

        match command.as_str() {
            "help" | "commands" => {
                if args.len() > 1 {
                    self.push_command_feedback(target, "Usage: /help [page]");
                    return;
                }
                let page = args
                    .first()
                    .and_then(|raw| raw.parse::<usize>().ok())
                    .unwrap_or(1);
                self.push_command_help(target, page);
            }
            "tp" => {
                if args.len() != 3 {
                    self.push_command_feedback(target, "Usage: /tp <x> <y> <z>");
                    return;
                }
                let x = Self::parse_tp_coordinate(self.player_pos.x, args[0]);
                let y = Self::parse_tp_coordinate(self.player_pos.y, args[1]);
                let z = Self::parse_tp_coordinate(self.player_pos.z, args[2]);
                match (x, y, z) {
                    (Some(x), Some(y), Some(z)) => {
                        self.teleport_player(x, y, z);
                        self.push_command_feedback(target, format!("Teleported to {x:.1} {y:.1} {z:.1}"));
                    }
                    _ => self.push_command_feedback(target, "Usage: /tp <x> <y> <z>"),
                }
            }
            "pos" | "where" | "whereami" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /pos");
                    return;
                }
                let chunk = self.player_chunk_pos();
                self.push_command_feedback(
                    target,
                    format!(
                        "Position {:.2} {:.2} {:.2} | Chunk {} {} {}",
                        self.player_pos.x, self.player_pos.y, self.player_pos.z, chunk.x, chunk.y, chunk.z
                    ),
                );
            }
            "chunk_info" | "chunks" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /chunk_info");
                    return;
                }
                self.push_command_feedback(
                    target,
                    format!(
                        "Chunks loaded={} pending={} mesh_queue={} dirty={}",
                        self.chunks.len(),
                        self.pending_chunks.len(),
                        self.mesh_queue.len(),
                        self.dirty_chunks.len(),
                    ),
                );
            }
            "seed" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /seed");
                    return;
                }
                self.push_command_feedback(target, format!("World seed: {}", self.world_seed));
            }
            "gamemode" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /gamemode creative|survival");
                    return;
                }
                match args[0].to_ascii_lowercase().as_str() {
                    "creative" | "c" => {
                        self.set_play_mode(PlayMode::Creative);
                        self.push_command_feedback(target, "Gamemode set to creative");
                    }
                    "survival" | "s" => {
                        self.set_play_mode(PlayMode::Survival);
                        self.push_command_feedback(target, "Gamemode set to survival");
                    }
                    _ => self.push_command_feedback(target, "Usage: /gamemode creative|survival"),
                }
            }
            "gmc" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /gmc");
                    return;
                }
                self.set_play_mode(PlayMode::Creative);
                self.push_command_feedback(target, "Gamemode set to creative");
            }
            "gms" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /gms");
                    return;
                }
                self.set_play_mode(PlayMode::Survival);
                self.push_command_feedback(target, "Gamemode set to survival");
            }
            "fly" => {
                if args.len() > 1 {
                    self.push_command_feedback(target, "Usage: /fly [on|off|toggle]");
                    return;
                }
                let Some(new_state) = Self::parse_toggle_argument(args.first().copied(), self.fly_mode) else {
                    self.push_command_feedback(target, "Usage: /fly [on|off|toggle]");
                    return;
                };
                if new_state && self.is_survival_mode() {
                    self.push_command_feedback(target, "Fly mode is only available in creative.");
                    return;
                }
                self.set_fly_mode(new_state);
                self.push_command_feedback(
                    target,
                    format!("Fly mode {}", if self.fly_mode { "enabled" } else { "disabled" }),
                );
            }
            "time" => {
                if args.is_empty() {
                    self.push_command_feedback(
                        target,
                        "Usage: /time <set|add|freeze|query> ...",
                    );
                    return;
                }
                match args[0].to_ascii_lowercase().as_str() {
                    "set" => {
                        if args.len() != 2 {
                            self.push_command_feedback(
                                target,
                                "Usage: /time set <0..1|day|sunrise|noon|sunset|night>",
                            );
                            return;
                        }
                        if let Some(new_time) = Self::parse_time_token(args[1]) {
                            self.time_of_day = new_time.rem_euclid(1.0);
                            self.push_command_feedback(target, format!("Time set to {:.3}", self.time_of_day));
                        } else {
                            self.push_command_feedback(
                                target,
                                "Usage: /time set <0..1|day|sunrise|noon|sunset|night>",
                            );
                        }
                    }
                    "add" => {
                        if args.len() != 2 {
                            self.push_command_feedback(target, "Usage: /time add <delta>");
                            return;
                        }
                        match args[1].parse::<f32>() {
                            Ok(delta) => {
                                self.time_of_day = (self.time_of_day + delta).rem_euclid(1.0);
                                self.push_command_feedback(target, format!("Time is now {:.3}", self.time_of_day));
                            }
                            Err(_) => self.push_command_feedback(target, "Usage: /time add <delta>"),
                        }
                    }
                    "freeze" => {
                        if args.len() > 2 {
                            self.push_command_feedback(target, "Usage: /time freeze [on|off|toggle]");
                            return;
                        }
                        let Some(new_state) = Self::parse_toggle_argument(args.get(1).copied(), self.time_frozen) else {
                            self.push_command_feedback(target, "Usage: /time freeze [on|off|toggle]");
                            return;
                        };
                        self.time_frozen = new_state;
                        self.push_command_feedback(
                            target,
                            format!("Time freeze {}", if self.time_frozen { "enabled" } else { "disabled" }),
                        );
                    }
                    "query" => {
                        if args.len() != 1 {
                            self.push_command_feedback(target, "Usage: /time query");
                            return;
                        }
                        self.push_command_feedback(
                            target,
                            format!("Time {:.3} (frozen: {})", self.time_of_day, if self.time_frozen { "yes" } else { "no" }),
                        );
                    }
                    _ => self.push_command_feedback(
                        target,
                        "Usage: /time <set|add|freeze|query> ...",
                    ),
                }
            }
            "day" | "sunrise" | "noon" | "sunset" | "night" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, format!("Usage: /{command}"));
                    return;
                }
                if let Some(new_time) = Self::parse_time_token(command.as_str()) {
                    self.time_of_day = new_time;
                    self.push_command_feedback(target, format!("Time set to {command} ({new_time:.3})"));
                }
            }
            "weather" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /weather clear|rain|snow|cycle|query");
                    return;
                }
                match args[0].to_ascii_lowercase().as_str() {
                    "clear" => {
                        self.set_weather_state(WeatherState::Clear);
                        self.push_command_feedback(target, "Weather set to clear");
                    }
                    "rain" => {
                        self.set_weather_state(WeatherState::Rain);
                        self.push_command_feedback(target, "Weather set to rain");
                    }
                    "snow" => {
                        self.set_weather_state(WeatherState::Snow);
                        self.push_command_feedback(target, "Weather set to snow");
                    }
                    "cycle" => {
                        let next = match self.next_weather_rand_u32() % 3 {
                            0 => WeatherState::Clear,
                            1 => WeatherState::Rain,
                            _ => WeatherState::Snow,
                        };
                        self.set_weather_state(next);
                        self.push_command_feedback(
                            target,
                            format!("Weather cycled to {}", Self::weather_name(self.weather_state)),
                        );
                    }
                    "query" => {
                        self.push_command_feedback(
                            target,
                            format!(
                                "Weather {} (next change in {:.1}s)",
                                Self::weather_name(self.weather_state),
                                self.weather_timer.max(0.0)
                            ),
                        );
                    }
                    _ => self.push_command_feedback(target, "Usage: /weather clear|rain|snow|cycle|query"),
                }
            }
            "heal" => {
                if args.len() > 1 {
                    self.push_command_feedback(target, "Usage: /heal [amount]");
                    return;
                }
                if args.is_empty() {
                    self.health = MAX_HEALTH;
                } else {
                    match args[0].parse::<f32>() {
                        Ok(amount) if amount > 0.0 => {
                            self.health = (self.health + amount).clamp(0.0, MAX_HEALTH);
                        }
                        _ => {
                            self.push_command_feedback(target, "Usage: /heal [amount]");
                            return;
                        }
                    }
                }
                self.push_command_feedback(target, format!("Health: {:.1}/{}", self.health, MAX_HEALTH));
            }
            "damage" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /damage <amount>");
                    return;
                }
                let Ok(amount) = args[0].parse::<f32>() else {
                    self.push_command_feedback(target, "Usage: /damage <amount>");
                    return;
                };
                if amount <= 0.0 {
                    self.push_command_feedback(target, "Usage: /damage <amount>");
                    return;
                }
                if self.is_survival_mode() {
                    self.apply_damage(amount);
                } else {
                    self.health = (self.health - amount).max(0.0);
                    if self.health <= 0.0 {
                        self.respawn();
                    }
                }
                self.push_command_feedback(target, format!("Health: {:.1}/{}", self.health, MAX_HEALTH));
            }
            "feed" => {
                if args.len() > 1 {
                    self.push_command_feedback(target, "Usage: /feed [amount]");
                    return;
                }
                if args.is_empty() {
                    self.hunger = MAX_HUNGER;
                } else {
                    match args[0].parse::<f32>() {
                        Ok(amount) if amount > 0.0 => {
                            self.hunger = (self.hunger + amount).clamp(0.0, MAX_HUNGER);
                        }
                        _ => {
                            self.push_command_feedback(target, "Usage: /feed [amount]");
                            return;
                        }
                    }
                }
                self.push_command_feedback(target, format!("Hunger: {:.1}/{}", self.hunger, MAX_HUNGER));
            }
            "air" => {
                if args.len() > 1 {
                    self.push_command_feedback(target, "Usage: /air [amount|full]");
                    return;
                }
                if let Some(raw) = args.first() {
                    let lower = raw.to_ascii_lowercase();
                    if matches!(lower.as_str(), "full" | "max") {
                        self.air_supply = MAX_AIR_SUPPLY;
                    } else {
                        match raw.parse::<f32>() {
                            Ok(value) if value >= 0.0 => {
                                self.air_supply = value.clamp(0.0, MAX_AIR_SUPPLY);
                            }
                            _ => {
                                self.push_command_feedback(target, "Usage: /air [amount|full]");
                                return;
                            }
                        }
                    }
                }
                self.push_command_feedback(target, format!("Air: {:.1}/{}", self.air_supply, MAX_AIR_SUPPLY));
            }
            "xp" => {
                if args.is_empty() {
                    self.push_command_feedback(target, "Usage: /xp <add|set|query> ...");
                    return;
                }
                match args[0].to_ascii_lowercase().as_str() {
                    "add" => {
                        if args.len() != 2 {
                            self.push_command_feedback(target, "Usage: /xp add <amount>");
                            return;
                        }
                        match args[1].parse::<u32>() {
                            Ok(amount) => {
                                self.add_xp(amount);
                                self.push_command_feedback(
                                    target,
                                    format!("XP total={} level={}", self.xp_total, self.xp_level),
                                );
                            }
                            Err(_) => self.push_command_feedback(target, "Usage: /xp add <amount>"),
                        }
                    }
                    "set" => {
                        if args.len() != 2 {
                            self.push_command_feedback(target, "Usage: /xp set <amount>");
                            return;
                        }
                        match args[1].parse::<u32>() {
                            Ok(amount) => {
                                self.set_xp_total(amount);
                                self.push_command_feedback(
                                    target,
                                    format!("XP total={} level={}", self.xp_total, self.xp_level),
                                );
                            }
                            Err(_) => self.push_command_feedback(target, "Usage: /xp set <amount>"),
                        }
                    }
                    "query" => {
                        if args.len() != 1 {
                            self.push_command_feedback(target, "Usage: /xp query");
                            return;
                        }
                        self.push_command_feedback(
                            target,
                            format!(
                                "XP total={} level={} progress={:.1}%",
                                self.xp_total,
                                self.xp_level,
                                self.xp_progress().clamp(0.0, 1.0) * 100.0
                            ),
                        );
                    }
                    _ => self.push_command_feedback(target, "Usage: /xp <add|set|query> ..."),
                }
            }
            "setspawn" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /setspawn");
                    return;
                }
                self.spawn_position = self.player_pos;
                self.bed_spawn_point = Some(self.player_pos);
                self.push_command_feedback(
                    target,
                    format!(
                        "Spawn set to {:.1} {:.1} {:.1}",
                        self.spawn_position.x, self.spawn_position.y, self.spawn_position.z
                    ),
                );
            }
            "spawn" | "home" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, format!("Usage: /{command}"));
                    return;
                }
                let spawn = self.spawn_position;
                self.teleport_player(spawn.x, spawn.y, spawn.z);
                self.push_command_feedback(
                    target,
                    format!("Teleported to spawn {:.1} {:.1} {:.1}", spawn.x, spawn.y, spawn.z),
                );
            }
            "kill" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /kill");
                    return;
                }
                self.respawn();
                self.push_command_feedback(target, "You died");
            }
            "clearinventory" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /clearinventory");
                    return;
                }
                self.inventory.slots.fill(None);
                self.reset_inventory_drag_distribution();
                self.refresh_selected_block();
                self.push_command_feedback(target, "Inventory cleared");
            }
            "clearhotbar" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /clearhotbar");
                    return;
                }
                for slot in 0..Inventory::HOTBAR_SIZE {
                    self.inventory.slots[slot] = None;
                }
                self.reset_inventory_drag_distribution();
                self.refresh_selected_block();
                self.push_command_feedback(target, "Hotbar cleared");
            }
            "cleararmor" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /cleararmor");
                    return;
                }
                self.armor_slots = [None; 4];
                self.push_command_feedback(target, "Armor cleared");
            }
            "clearcursor" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /clearcursor");
                    return;
                }
                self.cursor_stack = None;
                self.reset_inventory_drag_distribution();
                self.push_command_feedback(target, "Cursor stack cleared");
            }
            "clearall" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /clearall");
                    return;
                }
                self.inventory.slots.fill(None);
                self.armor_slots = [None; 4];
                self.cursor_stack = None;
                self.inventory_crafting_slots = [None; 4];
                self.table_crafting_slots = [None; 9];
                self.reset_inventory_drag_distribution();
                self.refresh_selected_block();
                self.push_command_feedback(target, "Inventory, armor, cursor and crafting slots cleared");
            }
            "give" => {
                let Some(item_name) = args.first().copied() else {
                    self.push_command_feedback(target, "Usage: /give <item_name> [count]");
                    return;
                };
                if args.len() > 2 {
                    self.push_command_feedback(target, "Usage: /give <item_name> [count]");
                    return;
                }
                let count = match args.get(1) {
                    None => 1u8,
                    Some(raw) => match raw.parse::<u16>() {
                        Ok(parsed) if (1..=u8::MAX as u16).contains(&parsed) => parsed as u8,
                        _ => {
                            self.push_command_feedback(target, "Usage: /give <item_name> [count]");
                            return;
                        }
                    },
                };

                let Some(item_id) = self.singleplayer_item_id_from_name(item_name) else {
                    self.push_command_feedback(target, format!("Unknown item: {item_name}"));
                    return;
                };

                let remaining = self.inventory.add_item(item_id, count);
                self.refresh_selected_block();
                let added = count.saturating_sub(remaining);
                if added == 0 {
                    self.push_command_feedback(target, "Inventory is full.");
                } else if remaining > 0 {
                    self.push_command_feedback(
                        target,
                        format!("Added {added} x {item_name} ({remaining} could not be added)."),
                    );
                } else {
                    self.push_command_feedback(target, format!("Added {added} x {item_name}."));
                }
            }
            "fov" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /fov <60..120>");
                    return;
                }
                match args[0].parse::<f32>() {
                    Ok(value) => {
                        self.set_fov(value);
                        self.push_command_feedback(target, format!("FOV set to {:.1}", self.settings.fov));
                    }
                    Err(_) => self.push_command_feedback(target, "Usage: /fov <60..120>"),
                }
            }
            "gui" | "guiscale" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /gui <1.0..3.0>");
                    return;
                }
                match args[0].parse::<f32>() {
                    Ok(value) => {
                        self.set_gui_scale(value);
                        self.push_command_feedback(
                            target,
                            format!("GUI scale set to {:.1}x", self.settings.gui_scale),
                        );
                    }
                    Err(_) => self.push_command_feedback(target, "Usage: /gui <1.0..3.0>"),
                }
            }
            "renderdistance" | "rd" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /renderdistance <4..24>");
                    return;
                }
                match args[0].parse::<i32>() {
                    Ok(value) => {
                        self.set_render_distance(value);
                        self.push_command_feedback(
                            target,
                            format!("Render distance set to {}", self.settings.render_distance),
                        );
                    }
                    Err(_) => self.push_command_feedback(target, "Usage: /renderdistance <4..24>"),
                }
            }
            "lod" | "lod1" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /lod <4..14>");
                    return;
                }
                match args[0].parse::<i32>() {
                    Ok(value) => {
                        self.set_lod1_distance(value);
                        self.push_command_feedback(
                            target,
                            format!("LOD1 distance set to {}", self.settings.lod1_distance),
                        );
                    }
                    Err(_) => self.push_command_feedback(target, "Usage: /lod <4..14>"),
                }
            }
            "mouse" | "sensitivity" => {
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /mouse <0.5..5.0>");
                    return;
                }
                match args[0].parse::<f32>() {
                    Ok(value) => {
                        self.set_mouse_sensitivity(value);
                        self.push_command_feedback(
                            target,
                            format!("Mouse sensitivity set to {:.2}", self.settings.mouse_sensitivity),
                        );
                    }
                    Err(_) => self.push_command_feedback(target, "Usage: /mouse <0.5..5.0>"),
                }
            }
            "showfps" | "fps" => {
                if args.first().map(|arg| arg.eq_ignore_ascii_case("query")) == Some(true) {
                    if args.len() != 1 {
                        self.push_command_feedback(target, "Usage: /showfps [on|off|toggle|query]");
                        return;
                    }
                    self.push_command_feedback(
                        target,
                        format!(
                            "show_fps={} current_fps={:.1}",
                            if self.settings.show_fps { "on" } else { "off" },
                            self.fps
                        ),
                    );
                    return;
                }
                if args.len() > 1 {
                    self.push_command_feedback(target, "Usage: /showfps [on|off|toggle|query]");
                    return;
                }
                let Some(new_state) =
                    Self::parse_toggle_argument(args.first().copied(), self.settings.show_fps)
                else {
                    self.push_command_feedback(target, "Usage: /showfps [on|off|toggle|query]");
                    return;
                };
                self.set_show_fps(new_state);
                self.push_command_feedback(
                    target,
                    format!("Show FPS {}", if self.settings.show_fps { "enabled" } else { "disabled" }),
                );
            }
            "save" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /save");
                    return;
                }
                if matches!(self.game_mode, Some(GameMode::Singleplayer)) {
                    self.save_world();
                    self.last_save_time = Some(Instant::now());
                    self.push_command_feedback(target, "World saved");
                } else {
                    self.push_command_feedback(target, "Save is only available in singleplayer");
                }
            }
            "savefull" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /savefull");
                    return;
                }
                if matches!(self.game_mode, Some(GameMode::Singleplayer)) {
                    self.save_world_full();
                    self.last_save_time = Some(Instant::now());
                    self.push_command_feedback(target, "World fully saved");
                } else {
                    self.push_command_feedback(target, "Save is only available in singleplayer");
                }
            }
            "connect" => {
                let Some(addr) = args.first().copied() else {
                    self.push_command_feedback(target, "Usage: /connect <ip:port>");
                    return;
                };
                if args.len() != 1 {
                    self.push_command_feedback(target, "Usage: /connect <ip:port>");
                    return;
                }
                match addr.parse::<SocketAddr>() {
                    Ok(parsed_addr) => {
                        self.server_ip = parsed_addr.to_string();
                        self.push_command_feedback(target, format!("Connecting to {}", self.server_ip));
                        self.return_to_menu();
                        self.start_multiplayer();
                        if matches!(self.game_mode, Some(GameMode::Multiplayer { .. })) {
                            self.push_command_feedback(target, "Connection attempt started");
                        } else {
                            self.push_command_feedback(target, "Connection attempt failed");
                        }
                    }
                    Err(e) => self.push_command_feedback(target, format!("Invalid address: {e}")),
                }
            }
            "disconnect" => {
                if !args.is_empty() {
                    self.push_command_feedback(target, "Usage: /disconnect");
                    return;
                }
                if matches!(self.game_mode, Some(GameMode::Multiplayer { .. })) {
                    self.return_to_menu();
                    self.push_command_feedback(target, "Disconnected from server");
                } else {
                    self.push_command_feedback(target, "Not connected to multiplayer");
                }
            }
            _ => {
                self.push_command_feedback(target, format!("Unknown command: {command}"));
            }
        }
    }

    fn singleplayer_item_id_from_name(&self, item_name: &str) -> Option<ItemId> {
        let normalized = item_name
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_");

        let explicit_item = match normalized.as_str() {
            "stick" => Some(ItemId::STICK),
            "wooden_pickaxe" => Some(ItemId::WOODEN_PICKAXE),
            "wooden_sword" => Some(ItemId::WOODEN_SWORD),
            "stone_pickaxe" => Some(ItemId::STONE_PICKAXE),
            "stone_sword" => Some(ItemId::STONE_SWORD),
            "stone_shovel" => Some(ItemId::STONE_SHOVEL),
            "stone_axe" => Some(ItemId::STONE_AXE),
            "iron_pickaxe" => Some(ItemId::IRON_PICKAXE),
            "iron_sword" => Some(ItemId::IRON_SWORD),
            "iron_shovel" => Some(ItemId::IRON_SHOVEL),
            "iron_axe" => Some(ItemId::IRON_AXE),
            "diamond_pickaxe" => Some(ItemId::DIAMOND_PICKAXE),
            "diamond_sword" => Some(ItemId::DIAMOND_SWORD),
            "diamond_shovel" => Some(ItemId::DIAMOND_SHOVEL),
            "diamond_axe" => Some(ItemId::DIAMOND_AXE),
            "wooden_shovel" => Some(ItemId::WOODEN_SHOVEL),
            "wooden_axe" => Some(ItemId::WOODEN_AXE),
            "wooden_hoe" => Some(ItemId::WOODEN_HOE),
            "stone_hoe" => Some(ItemId::STONE_HOE),
            "iron_hoe" => Some(ItemId::IRON_HOE),
            "diamond_hoe" => Some(ItemId::DIAMOND_HOE),
            "iron_ingot" => Some(ItemId::IRON_INGOT),
            "gold_ingot" => Some(ItemId::GOLD_INGOT),
            "diamond_gem" | "diamond" => Some(ItemId::DIAMOND_GEM),
            "coal" => Some(ItemId::COAL),
            "wheat_item" | "wheat" => Some(ItemId::WHEAT_ITEM),
            "wheat_seeds" | "seeds" => Some(ItemId::WHEAT_SEEDS),
            "bread" => Some(ItemId::BREAD),
            "copper_ingot" => Some(ItemId::COPPER_INGOT),
            "iron_helmet" => Some(ItemId::IRON_HELMET),
            "iron_chestplate" => Some(ItemId::IRON_CHESTPLATE),
            "iron_leggings" => Some(ItemId::IRON_LEGGINGS),
            "iron_boots" => Some(ItemId::IRON_BOOTS),
            "diamond_helmet" => Some(ItemId::DIAMOND_HELMET),
            "diamond_chestplate" => Some(ItemId::DIAMOND_CHESTPLATE),
            "diamond_leggings" => Some(ItemId::DIAMOND_LEGGINGS),
            "diamond_boots" => Some(ItemId::DIAMOND_BOOTS),
            "bone_meal" | "bonemeal" => Some(ItemId::BONE_MEAL),
            _ => None,
        };
        if let Some(item_id) = explicit_item {
            return Some(item_id);
        }

        self.registry
            .as_ref()
            .and_then(|registry| registry.get_by_name(&normalized))
            .map(ItemId::from)
    }

    fn build_chat_overlay_lines(&self, now: Instant) -> Vec<(String, f32)> {
        let start = self
            .chat_messages
            .len()
            .saturating_sub(CHAT_VISIBLE_MESSAGES);
        let mut lines = Vec::new();
        for message in self.chat_messages.iter().skip(start) {
            let alpha = if self.chat_open {
                1.0
            } else {
                let age = now
                    .saturating_duration_since(message.received_at)
                    .as_secs_f32();
                if age >= CHAT_FADE_SECS {
                    continue;
                }
                1.0 - (age / CHAT_FADE_SECS)
            };
            if alpha > 0.0 {
                lines.push((message.text.clone(), alpha.clamp(0.0, 1.0)));
            }
        }
        lines
    }

    fn mode_and_connection_status(&self) -> (String, String) {
        match &self.game_mode {
            Some(GameMode::Singleplayer) => ("singleplayer".to_string(), "local".to_string()),
            Some(GameMode::Multiplayer { net }) => {
                let status = if net.is_connected() {
                    "connected"
                } else {
                    "disconnected"
                };
                ("multiplayer".to_string(), status.to_string())
            }
            None => ("none".to_string(), "n/a".to_string()),
        }
    }

    fn execute_console_command(&mut self, raw: &str) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }

        self.push_console_message(format!("> {trimmed}"));
        self.run_local_command(trimmed, CommandFeedbackTarget::Console);
    }

    fn build_overlay_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if self.show_debug {
            let (mode, connection_status) = self.mode_and_connection_status();
            let frame_time_stats = self.frame_time_stats();
            let debug_info = DebugInfo::from_camera(
                &self.camera,
                self.fps,
                self.selected_block,
                self.fly_mode,
                self.chunks.len(),
                self.pending_chunks.len(),
                self.mesh_queue.len(),
                mode,
                connection_status,
                self.last_render_stats,
                self.last_upload_stats.uploaded_bytes,
                self.last_upload_stats.uploaded_chunks,
                self.last_upload_stats.buffer_reallocations,
                frame_time_stats.avg_ms,
                frame_time_stats.p95_ms,
                frame_time_stats.p99_ms,
                frame_time_stats.max_ms,
            );
            lines.extend(debug_info.overlay_lines());
        }

        if self.is_survival_mode() {
            let progress = self.xp_progress().clamp(0.0, 1.0);
            let filled = (progress * 20.0).round() as usize;
            let filled = filled.min(20);
            let bar = format!("{}{}", "=".repeat(filled), ".".repeat(20 - filled));
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!(
                "XP Lv{} [{}] {:>3}%",
                self.xp_level,
                bar,
                (progress * 100.0).round() as u32
            ));
        }

        if self.inventory_open && self.open_chest.is_none() {
            let labels = ["H", "C", "L", "B"];
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push("Armor".to_string());
            for (idx, label) in labels.iter().enumerate() {
                let slot_text = self.armor_slots[idx]
                    .map(|stack| self.item_label(stack.item))
                    .unwrap_or_else(|| "-".to_string());
                lines.push(format!("{label}: {slot_text}"));
            }
        }

        if self.console_open || !self.console_messages.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(if self.console_open {
                "Console [Open]".to_string()
            } else {
                "Console".to_string()
            });

            let start = self.console_messages.len().saturating_sub(5);
            for msg in self.console_messages.iter().skip(start) {
                lines.push(msg.clone());
            }

            if self.console_open {
                lines.push(format!("> {}", self.text_input));
                lines.push("Enter=run  Esc/F1/`=close".to_string());
            }
        }

        lines
    }

    fn record_frame_time_sample(&mut self, frame_ms: f32) {
        if self.frame_time_samples_ms.len() == FRAME_TIME_HISTORY_LEN {
            self.frame_time_samples_ms.pop_front();
        }
        self.frame_time_samples_ms.push_back(frame_ms.max(0.0));
    }

    fn frame_time_stats(&self) -> FrameTimeStats {
        if self.frame_time_samples_ms.is_empty() {
            return FrameTimeStats::default();
        }

        let mut sorted: Vec<f32> = self.frame_time_samples_ms.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = sorted.len();
        let avg_ms = sorted.iter().sum::<f32>() / len as f32;
        let p95_idx = percentile_index(len, 0.95);
        let p99_idx = percentile_index(len, 0.99);
        let max_ms = *sorted.last().unwrap_or(&0.0);

        FrameTimeStats {
            avg_ms,
            p95_ms: sorted[p95_idx],
            p99_ms: sorted[p99_idx],
            max_ms,
        }
    }

    fn log_performance_metrics_if_due(&mut self) {
        if self.perf_log_timer_seconds < PERF_LOG_INTERVAL_SECS {
            return;
        }
        self.perf_log_timer_seconds -= PERF_LOG_INTERVAL_SECS;

        let frame_time_stats = self.frame_time_stats();
        info!(
            "render stats | frame_ms avg={:.2} p95={:.2} p99={:.2} max={:.2} | draws opaque={} water={} | chunks={} indices={} vertices={} | uploads bytes={} chunks={} reallocs={} pending_uploads={}",
            frame_time_stats.avg_ms,
            frame_time_stats.p95_ms,
            frame_time_stats.p99_ms,
            frame_time_stats.max_ms,
            self.last_render_stats.opaque_draw_calls,
            self.last_render_stats.water_draw_calls,
            self.last_render_stats.rendered_chunks,
            self.last_render_stats.rendered_indices,
            self.last_render_stats.rendered_vertices,
            self.last_upload_stats.uploaded_bytes,
            self.last_upload_stats.uploaded_chunks,
            self.last_upload_stats.buffer_reallocations,
            self.pending_mesh_uploads.len(),
        );
    }

    fn world_summary_to_entry(summary: WorldSummary) -> WorldMenuEntry {
        let display_name = if summary.display_name.trim().is_empty() {
            summary.folder_name.clone()
        } else {
            summary.display_name
        };
        WorldMenuEntry {
            display_name,
            world_dir: summary.world_dir,
            world_seed: summary.world_seed,
            size_label: format_size_bytes(summary.regions_size_bytes),
            last_opened_label: format_system_time(summary.last_opened),
        }
    }

    fn refresh_world_entries(&mut self) {
        let previously_selected_path = self
            .world_selected
            .and_then(|idx| self.world_entries.get(idx))
            .map(|world| world.world_dir.clone());

        match scan_worlds(Path::new(WORLDS_DIR)) {
            Ok(worlds) => {
                self.world_entries = worlds
                    .into_iter()
                    .map(Self::world_summary_to_entry)
                    .collect();
                self.world_selected = if self.world_entries.is_empty() {
                    None
                } else if let Some(selected_path) = previously_selected_path {
                    self.world_entries
                        .iter()
                        .position(|world| world.world_dir == selected_path)
                        .or(Some(0))
                } else {
                    Some(0)
                };
            }
            Err(e) => {
                warn!("Failed to scan worlds in {WORLDS_DIR}: {e}");
                self.world_entries.clear();
                self.world_selected = None;
            }
        }
    }

    fn enter_world_select(&mut self) {
        self.refresh_world_entries();
        self.world_create_form_open = false;
        self.world_delete_confirmation_open = false;
        self.world_create_active_field = WorldCreateInputField::Name;
        self.app_state = AppState::WorldSelect;
    }

    fn open_create_world_form(&mut self) {
        self.world_create_form_open = true;
        self.world_delete_confirmation_open = false;
        self.world_create_active_field = WorldCreateInputField::Name;
        self.world_create_play_mode = PlayMode::Survival;
        if self.world_create_name_input.trim().is_empty() {
            self.world_create_name_input = DEFAULT_NEW_WORLD_NAME.to_string();
        }
    }

    fn close_create_world_form(&mut self) {
        self.world_create_form_open = false;
        self.world_create_name_input.clear();
        self.world_create_seed_input.clear();
        self.world_create_active_field = WorldCreateInputField::Name;
        self.world_create_play_mode = PlayMode::Survival;
    }

    fn append_world_form_text(&mut self, text: &str) {
        match self.world_create_active_field {
            WorldCreateInputField::Name => {
                for ch in text.chars() {
                    if !ch.is_ascii() {
                        continue;
                    }
                    if !matches!(ch, ' ' | '_' | '-') && !ch.is_ascii_alphanumeric() {
                        continue;
                    }
                    if self.world_create_name_input.len() >= MAX_WORLD_NAME_INPUT_LEN {
                        break;
                    }
                    self.world_create_name_input.push(ch);
                }
            }
            WorldCreateInputField::Seed => {
                for ch in text.chars() {
                    if !ch.is_ascii() {
                        continue;
                    }
                    if !matches!(ch, '-' | '_') && !ch.is_ascii_alphanumeric() {
                        continue;
                    }
                    if self.world_create_seed_input.len() >= MAX_WORLD_SEED_INPUT_LEN {
                        break;
                    }
                    self.world_create_seed_input.push(ch);
                }
            }
        }
    }

    fn select_previous_world(&mut self) {
        let Some(selected) = self.world_selected else {
            return;
        };
        self.world_selected = Some(selected.saturating_sub(1));
    }

    fn select_next_world(&mut self) {
        let Some(selected) = self.world_selected else {
            return;
        };
        if selected + 1 < self.world_entries.len() {
            self.world_selected = Some(selected + 1);
        }
    }

    fn play_selected_world(&mut self) {
        let Some(selected_world) = self
            .world_selected
            .and_then(|idx| self.world_entries.get(idx))
            .cloned()
        else {
            return;
        };
        self.start_singleplayer(
            selected_world.world_dir,
            Some(selected_world.display_name),
        );
    }

    fn create_world_from_form(&mut self) {
        let world_name = self.world_create_name_input.trim();
        let world_name = if world_name.is_empty() {
            DEFAULT_NEW_WORLD_NAME.to_string()
        } else {
            world_name.to_string()
        };
        let world_seed = parse_seed_input(&self.world_create_seed_input);

        let folder_base = sanitize_world_folder_name(&world_name);
        let folder_name = unique_world_folder_name(Path::new(WORLDS_DIR), &folder_base);
        let world_dir = Path::new(WORLDS_DIR).join(folder_name);

        if let Err(e) = ClientPersistence::open(&world_dir) {
            warn!("Failed to create world directory {}: {e}", world_dir.display());
            return;
        }

        let meta = WorldMeta {
            world_name: Some(world_name.clone()),
            world_seed,
            player_position: [0.0, 40.0, 0.0],
            player_yaw: 0.0,
            player_pitch: 0.0,
            time_of_day: 0.5,
            play_mode: self.world_create_play_mode.into(),
        };
        if let Err(e) = meta.save(&world_dir) {
            warn!("Failed to create world metadata for {}: {e}", world_dir.display());
            return;
        }

        self.close_create_world_form();
        self.refresh_world_entries();
        if let Some(selected) = self
            .world_entries
            .iter()
            .position(|world| world.world_dir == world_dir)
        {
            self.world_selected = Some(selected);
        }
    }

    fn delete_selected_world(&mut self) {
        let Some(selected_world) = self
            .world_selected
            .and_then(|idx| self.world_entries.get(idx))
            .cloned()
        else {
            return;
        };

        match fs::remove_dir_all(&selected_world.world_dir) {
            Ok(()) => {
                self.world_delete_confirmation_open = false;
                self.refresh_world_entries();
            }
            Err(e) => {
                warn!(
                    "Failed to delete world directory {}: {e}",
                    selected_world.world_dir.display()
                );
            }
        }
    }

    fn handle_world_select_hit(&mut self, hit: WorldSelectHitTarget) {
        match hit {
            WorldSelectHitTarget::WorldEntry(index) => {
                if index < self.world_entries.len() {
                    self.world_selected = Some(index);
                }
            }
            WorldSelectHitTarget::CreateNewWorld => self.open_create_world_form(),
            WorldSelectHitTarget::PlaySelected => self.play_selected_world(),
            WorldSelectHitTarget::DeleteSelected => {
                if self.world_selected.is_some() {
                    self.world_delete_confirmation_open = true;
                }
            }
            WorldSelectHitTarget::Back => {
                self.world_delete_confirmation_open = false;
                self.close_create_world_form();
                self.app_state = AppState::MainMenu;
            }
            WorldSelectHitTarget::CreateNameField => {
                self.world_create_active_field = WorldCreateInputField::Name;
            }
            WorldSelectHitTarget::CreateSeedField => {
                self.world_create_active_field = WorldCreateInputField::Seed;
            }
            WorldSelectHitTarget::CreateConfirm => self.create_world_from_form(),
            WorldSelectHitTarget::CreateCancel => self.close_create_world_form(),
            WorldSelectHitTarget::CreatePlayModeToggle => self.toggle_world_create_play_mode(),
            WorldSelectHitTarget::DeleteConfirm => self.delete_selected_world(),
            WorldSelectHitTarget::DeleteCancel => {
                self.world_delete_confirmation_open = false;
            }
        }
    }

    fn start_chunk_thread(&mut self, world_dir: PathBuf) {
        let (request_tx, request_rx) = std::sync::mpsc::channel::<ChunkPos>();
        let (result_tx, result_rx) = std::sync::mpsc::channel::<(ChunkPos, ChunkData)>();

        // Clone what we need for the thread
        let registry = self.registry.as_ref().unwrap().clone();

        let world_seed = self.world_seed;
        std::thread::Builder::new()
            .name("chunk-gen".to_string())
            .spawn(move || {
                let generator = WorldGenerator::new(world_seed);
                let mut persistence = match ClientPersistence::open(&world_dir) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        warn!(
                            "Chunk thread failed to open persistence at {}: {e}",
                            world_dir.display()
                        );
                        None
                    }
                };
                while let Ok(pos) = request_rx.recv() {
                    let chunk = if let Some(persistence) = persistence.as_mut() {
                        match persistence.load_chunk(pos) {
                            Ok(Some(saved_chunk)) => saved_chunk,
                            Ok(None) => generator.generate_chunk(pos, &registry),
                            Err(e) => {
                                warn!("Failed to load chunk {pos:?} from persistence: {e}");
                                generator.generate_chunk(pos, &registry)
                            }
                        }
                    } else {
                        generator.generate_chunk(pos, &registry)
                    };
                    if result_tx.send((pos, chunk)).is_err() {
                        break; // Main thread dropped receiver
                    }
                }
            })
            .expect("failed to spawn chunk generation thread");

        self.chunk_request_tx = Some(request_tx);
        self.chunk_result_rx = Some(result_rx);
    }

    fn upload_startup_chunks(&mut self, world_dir: PathBuf) {
        let registry = Arc::new(register_default_blocks());
        self.registry = Some(registry);
        self.rebuild_creative_catalog();
        self.mesh_worker = Some(MeshWorker::new());
        self.start_chunk_thread(world_dir);
        self.stream_chunks();
    }

    fn start_singleplayer(&mut self, world_dir: PathBuf, world_name: Option<String>) {
        let mut startup_play_mode = self.world_create_play_mode;
        self.render_time_seconds = 0.0;
        self.frame_time_samples_ms.clear();
        self.perf_log_timer_seconds = 0.0;
        self.pending_mesh_uploads.clear();
        self.mesh_jobs_in_flight.clear();
        self.last_upload_stats = UploadFrameStats::default();
        self.last_render_stats = RenderFrameStats::default();
        self.world_seed = DEFAULT_WORLD_SEED;
        self.player_pos = Vec3::new(0.0, 40.0, 0.0);
        self.velocity = Vec3::ZERO;
        self.on_ground = false;
        self.camera.yaw = 0.0;
        self.camera.pitch = 0.0;
        self.time_of_day = 0.5;
        self.health = MAX_HEALTH;
        self.hunger = MAX_HUNGER;
        self.air_supply = MAX_AIR_SUPPLY;
        self.last_damage_time = -999.0;
        self.damage_flash_timer = 0.0;
        self.inventory = Inventory::new();
        self.armor_slots = [None; 4];
        self.selected_hotbar_slot = 0;
        self.creative_search.clear();
        self.creative_scroll = 0;
        self.creative_catalog.clear();
        self.creative_catalog_full.clear();
        self.xp_total = 0;
        self.xp_level = 0;
        self.cursor_stack = None;
        self.inventory_open = false;
        self.reset_inventory_drag_distribution();
        self.close_open_chest_ui();
        self.chest_inventories.clear();
        self.attack_animation = 0.0;
        self.was_left_click_down = false;
        self.walk_particle_timer = 0.0;
        self.item_drops.clear();
        self.mobs.clear();
        self.mob_spawn_timer = 0.0;
        self.mob_spawn_cooldown = 5.0;
        self.remote_players.clear();
        self.remote_player_states.clear();
        self.remote_break_overlays.clear();
        self.button_timers.clear();
        self.active_pressure_plates.clear();
        self.active_world_dir = Some(world_dir.clone());
        self.active_world_name = world_name;
        if self.active_world_name.is_none() {
            self.active_world_name = world_dir
                .file_name()
                .map(|name| name.to_string_lossy().to_string());
        }

        // Try to load existing world
        let loaded_world_meta = match WorldMeta::load(&world_dir) {
            Ok(Some(meta)) => {
                if meta
                    .world_name
                    .as_ref()
                    .is_some_and(|name| !name.trim().is_empty())
                {
                    self.active_world_name = meta.world_name.clone();
                }
                self.world_seed = meta.world_seed;
                self.player_pos = Vec3::from_array(meta.player_position);
                self.camera.yaw = meta.player_yaw;
                self.camera.pitch = meta.player_pitch;
                self.time_of_day = meta.time_of_day;
                startup_play_mode = PlayMode::from(meta.play_mode);
                info!("Loaded world from {}", world_dir.display());
                true
            }
            Ok(None) => false,
            Err(e) => {
                warn!("Failed to load world metadata from {}: {e}", world_dir.display());
                false
            }
        };
        self.camera.position = self.player_pos + Vec3::new(0.0, EYE_HEIGHT, 0.0);
        self.reset_weather_state();
        self.reset_growth_state();

        // Set up persistence
        match ClientPersistence::open(&world_dir) {
            Ok(persistence) => {
                match persistence.load_chest_inventories() {
                    Ok(chests) => {
                        self.chest_inventories = chests;
                    }
                    Err(e) => {
                        warn!("Failed to load chest inventories: {e}");
                        self.close_open_chest_ui();
                        self.chest_inventories.clear();
                    }
                }
                match persistence.load_inventory() {
                    Ok(inv) => {
                        self.inventory = inv;
                    }
                    Err(e) => {
                        warn!("Failed to load player inventory: {e}");
                    }
                }
                self.persistence = Some(persistence);
            }
            Err(e) => warn!("Failed to open persistence: {e}"),
        }

        self.upload_startup_chunks(world_dir);
        self.set_cursor_grab(true);
        self.app_state = AppState::InGame;
        self.in_game_menu = InGameMenuState::None;
        self.game_mode = Some(GameMode::Singleplayer);
        self.pause_menu_selected = 0;
        self.settings_menu_selected = 0;
        self.active_settings_slider = None;
        self.spawn_found = loaded_world_meta;
        self.chunks_ready = false;
        self.sprinting = false;
        self.set_play_mode(startup_play_mode);
        self.last_save_time = Some(Instant::now());
        self.world_create_form_open = false;
        self.world_delete_confirmation_open = false;
        self.inventory_open = false;
        self.close_open_chest_ui();
        self.cursor_stack = None;
        self.reset_inventory_drag_distribution();
        self.set_console_open(false);
        self.set_chat_open(false);
        self.chat_messages.clear();
    }

    fn start_multiplayer(&mut self) {
        let addr: SocketAddr = match self.server_ip.parse() {
            Ok(a) => a,
            Err(e) => {
                warn!("Invalid server address '{}': {e}", self.server_ip);
                return;
            }
        };

        info!("Starting multiplayer connection to {addr}");
        let mut net = ClientNet::new(addr);
        net.connect("Player");
        info!("Handshake sent to {addr}");

        let registry = Arc::new(register_default_blocks());
        self.registry = Some(registry);
        self.rebuild_creative_catalog();
        self.mesh_worker = Some(MeshWorker::new());
        self.render_time_seconds = 0.0;
        self.frame_time_samples_ms.clear();
        self.perf_log_timer_seconds = 0.0;
        self.pending_mesh_uploads.clear();
        self.last_upload_stats = UploadFrameStats::default();
        self.last_render_stats = RenderFrameStats::default();
        self.world_seed = DEFAULT_WORLD_SEED;
        self.active_world_dir = None;
        self.active_world_name = None;
        self.chunks.clear();
        self.dirty_chunks.clear();
        self.pending_fluid_positions.clear();
        self.lava_simulation_frame_accumulator = 0;
        self.pending_chunks.clear();
        self.mesh_queue.clear();
        self.mesh_queue_set.clear();
        self.mesh_jobs_in_flight.clear();
        self.mesh_versions.clear();
        self.chunk_lods.clear();
        self.last_player_chunk = None;
        self.player_pos = Vec3::new(0.0, 40.0, 0.0);
        self.velocity = Vec3::ZERO;
        self.on_ground = false;
        self.health = MAX_HEALTH;
        self.hunger = MAX_HUNGER;
        self.xp_total = 0;
        self.xp_level = 0;
        self.air_supply = MAX_AIR_SUPPLY;
        self.last_damage_time = -999.0;
        self.damage_flash_timer = 0.0;
        self.armor_slots = [None; 4];
        self.creative_search.clear();
        self.creative_scroll = 0;
        self.filter_creative_catalog();
        self.inventory_open = false;
        self.close_open_chest_ui();
        self.chest_inventories.clear();
        self.cursor_stack = None;
        self.reset_inventory_drag_distribution();
        self.mobs.clear();
        self.mob_spawn_timer = 0.0;
        self.mob_spawn_cooldown = 5.0;
        self.my_player_id = None;
        self.chunk_request_tx = None;
        self.chunk_result_rx = None;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.clear_chunk_meshes();
        }

        self.set_cursor_grab(true);
        self.app_state = AppState::InGame;
        self.in_game_menu = InGameMenuState::None;
        self.game_mode = Some(GameMode::Multiplayer { net });
        self.pause_menu_selected = 0;
        self.settings_menu_selected = 0;
        self.active_settings_slider = None;
        self.spawn_found = false;
        self.chunks_ready = false;
        self.sprinting = false;
        self.set_play_mode(PlayMode::Survival);
        self.attack_animation = 0.0;
        self.was_left_click_down = false;
        self.walk_particle_timer = 0.0;
        self.item_drops.clear();
        self.mobs.clear();
        self.remote_players.clear();
        self.remote_player_states.clear();
        self.remote_break_overlays.clear();
        self.button_timers.clear();
        self.active_pressure_plates.clear();
        self.reset_weather_state();
        self.reset_growth_state();
        self.set_console_open(false);
        self.set_chat_open(false);
        self.chat_messages.clear();
    }

    fn return_to_menu(&mut self) {
        // Save world fully if singleplayer (flush to disk)
        self.save_world_full();
        self.set_inventory_open(false);

        // Disconnect if multiplayer
        if let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode {
            net.disconnect();
        }

        // Reset game state
        self.chunks.clear();
        self.dirty_chunks.clear();
        self.pending_fluid_positions.clear();
        self.lava_simulation_frame_accumulator = 0;
        self.mesh_queue.clear();
        self.mesh_queue_set.clear();
        self.mesh_jobs_in_flight.clear();
        self.mesh_versions.clear();
        self.chunk_lods.clear();
        self.pending_chunks.clear();
        self.mobs.clear();
        self.mob_spawn_timer = 0.0;
        self.mob_spawn_cooldown = 5.0;
        self.remote_players.clear();
        self.remote_player_states.clear();
        self.remote_break_overlays.clear();
        self.pending_mesh_uploads.clear();
        self.last_upload_stats = UploadFrameStats::default();
        self.last_render_stats = RenderFrameStats::default();
        self.close_open_chest_ui();
        self.chest_inventories.clear();
        self.armor_slots = [None; 4];
        self.reset_inventory_drag_distribution();
        self.last_player_chunk = None;
        self.chunk_request_tx = None;
        self.chunk_result_rx = None;
        self.game_mode = None;
        self.active_world_dir = None;
        self.active_world_name = None;
        self.persistence = None;
        self.my_player_id = None;
        self.chunks_ready = false;
        self.attack_animation = 0.0;
        self.was_left_click_down = false;
        self.walk_particle_timer = 0.0;
        self.item_drops.clear();
        self.xp_total = 0;
        self.xp_level = 0;
        self.weather_state = WeatherState::Clear;
        self.weather_timer = WEATHER_MAX_DURATION_SECS;
        self.leaf_decay_timers.clear();
        self.sapling_growth_timers.clear();
        self.sugar_cane_growth_timers.clear();
        self.tnt_explosion_queue.clear();
        self.button_timers.clear();
        self.active_pressure_plates.clear();
        self.block_scan_state = BlockScanState::default();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.clear_chunk_meshes();
        }
        self.set_cursor_grab(false);
        self.app_state = AppState::MainMenu;
        self.in_game_menu = InGameMenuState::None;
        self.sprinting = false;
        self.pause_menu_selected = 0;
        self.settings_menu_selected = 0;
        self.active_settings_slider = None;
        self.world_create_form_open = false;
        self.world_delete_confirmation_open = false;
        self.set_console_open(false);
        self.set_chat_open(false);
        self.chat_messages.clear();
    }

    fn apply_damage(&mut self, amount: f32) {
        if !self.is_survival_mode() || self.fly_mode || amount <= 0.0 {
            return;
        }
        let defense = self.total_armor_defense_points();
        let reduction =
            (defense as f32 * DAMAGE_REDUCTION_PER_ARMOR_POINT).min(MAX_ARMOR_DAMAGE_REDUCTION);
        let actual_damage = amount * (1.0 - reduction);
        if actual_damage <= 0.0 {
            return;
        }
        self.health = (self.health - actual_damage).max(0.0);
        self.damage_flash_timer = DAMAGE_FLASH_DURATION;
        self.last_damage_time = 0.0; // reset regen timer
        if self.health <= 0.0 {
            self.respawn();
        }
    }

    fn find_surface_y(&self, x: i32, z: i32, registry: &BlockRegistry) -> Option<f32> {
        for y in (0..=MOB_SPAWN_SCAN_MAX_Y).rev() {
            if is_block_solid(x, y, z, &self.chunks, registry) {
                return Some(y as f32 + 1.0);
            }
        }
        None
    }

    fn try_spawn_mob(&mut self, registry: &BlockRegistry) {
        if self.mobs.len() >= MOB_MAX_COUNT {
            return;
        }

        let angle = self.gameplay_rand_range(0.0, std::f32::consts::TAU);
        let distance = self.gameplay_rand_range(MOB_SPAWN_DISTANCE_MIN, MOB_SPAWN_DISTANCE_MAX);
        let spawn_x = (self.player_pos.x + angle.cos() * distance).floor() as i32;
        let spawn_z = (self.player_pos.z + angle.sin() * distance).floor() as i32;
        let Some(surface_y) = self.find_surface_y(spawn_x, spawn_z, registry) else {
            return;
        };

        let feet_y = surface_y + 0.01;
        let feet_block_y = surface_y.floor() as i32;
        if is_block_solid(spawn_x, feet_block_y, spawn_z, &self.chunks, registry)
            || is_block_solid(spawn_x, feet_block_y + 1, spawn_z, &self.chunks, registry)
        {
            return;
        }

        let day_cycle = self.time_of_day.rem_euclid(1.0);
        let is_daylight = (0.25..=0.75).contains(&day_cycle);
        let mob_type = if is_daylight {
            match self.next_gameplay_rand_u32() % 3 {
                0 => MobType::Chicken,
                1 => MobType::Pig,
                _ => MobType::Cow,
            }
        } else {
            if self.gameplay_rand_f32() < 0.7 {
                MobType::Zombie
            } else {
                MobType::Skeleton
            }
        };

        let mut mob = MobData::new(mob_type, [spawn_x as f32 + 0.5, feet_y, spawn_z as f32 + 0.5]);
        mob.ai_timer = self.gameplay_rand_range(2.0, 5.0);
        self.mobs.push(mob);
    }

    fn mob_xp_reward(&mut self, mob_type: MobType) -> u32 {
        let (min_xp, max_xp) = match mob_type {
            MobType::Chicken | MobType::Pig | MobType::Cow => (1, 3),
            MobType::Zombie | MobType::Skeleton => (5, 10),
        };
        if max_xp <= min_xp {
            return min_xp;
        }
        min_xp + (self.next_gameplay_rand_u32() % (max_xp - min_xp + 1))
    }

    fn drop_mob_loot(&mut self, mob_type: MobType, position: Vec3) {
        let props = mob_properties(mob_type);
        let drop_origin = IVec3::new(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        );
        for &(item, min_count, max_count) in props.drops {
            let count = if max_count <= min_count {
                min_count
            } else {
                min_count
                    + (self.next_gameplay_rand_u32() % u32::from(max_count - min_count + 1)) as u8
            };
            if count == 0 {
                continue;
            }
            self.spawn_item_drop(drop_origin, ItemStack::new(item, count));
        }
    }

    fn update_mobs(&mut self, dt: f32, registry: &BlockRegistry) {
        if !matches!(self.game_mode, Some(GameMode::Singleplayer)) {
            self.mobs.clear();
            self.mob_spawn_timer = 0.0;
            return;
        }

        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }

        self.mob_spawn_timer += dt;
        while self.mob_spawn_timer >= self.mob_spawn_cooldown {
            self.mob_spawn_timer -= self.mob_spawn_cooldown;
            self.try_spawn_mob(registry);
        }

        if self.mobs.is_empty() {
            return;
        }

        let day_cycle = self.time_of_day.rem_euclid(1.0);
        let hostile_daylight_despawn = (0.3..=0.7).contains(&day_cycle);
        let player_pos = self.player_pos;
        let mut pending_player_damage = 0.0;
        let mut alive_mobs = Vec::with_capacity(self.mobs.len());

        while let Some(mut mob) = self.mobs.pop() {
            mob.ai_timer = (mob.ai_timer - dt).max(0.0);
            mob.attack_cooldown = (mob.attack_cooldown - dt).max(0.0);
            mob.hurt_timer = (mob.hurt_timer - dt).max(0.0);

            let props = mob_properties(mob.mob_type);
            if props.hostile && hostile_daylight_despawn {
                continue;
            }

            let mut position = Vec3::from_array(mob.position);
            if (position - player_pos).length_squared() > MOB_DESPAWN_DISTANCE * MOB_DESPAWN_DISTANCE {
                continue;
            }

            let mut velocity = Vec3::from_array(mob.velocity);
            velocity.x *= (1.0 - 4.0 * dt).clamp(0.0, 1.0);
            velocity.z *= (1.0 - 4.0 * dt).clamp(0.0, 1.0);

            let initial_surface_y = self
                .find_surface_y(position.x.floor() as i32, position.z.floor() as i32, registry)
                .unwrap_or(position.y);
            let mut ground_y = initial_surface_y + 0.01;
            if position.y > ground_y {
                velocity.y -= MOB_GRAVITY * dt;
            }

            position += velocity * dt;
            if position.y < ground_y {
                position.y = ground_y;
                velocity.y = 0.0;
            }

            let mut movement = Vec2::ZERO;
            let to_player = Vec2::new(player_pos.x - position.x, player_pos.z - position.z);
            let player_distance = to_player.length();

            if props.hostile {
                match mob.ai_state {
                    MobAiState::Idle => {
                        if player_distance <= props.detection_range {
                            mob.ai_state = MobAiState::Chasing;
                            mob.wander_target = None;
                        } else if mob.ai_timer <= 0.0 {
                            mob.ai_state = MobAiState::Wandering;
                            mob.ai_timer = 5.0;
                            mob.wander_target = None;
                        }
                    }
                    MobAiState::Wandering => {
                        if player_distance <= props.detection_range {
                            mob.ai_state = MobAiState::Chasing;
                            mob.wander_target = None;
                        } else {
                            if mob.wander_target.is_none() {
                                let angle = self.gameplay_rand_range(0.0, std::f32::consts::TAU);
                                let distance = self.gameplay_rand_range(3.0, 8.0);
                                mob.wander_target = Some([
                                    position.x + angle.cos() * distance,
                                    position.y,
                                    position.z + angle.sin() * distance,
                                ]);
                            }
                            if let Some(target) = mob.wander_target {
                                let to_target = Vec2::new(target[0] - position.x, target[2] - position.z);
                                let target_dist = to_target.length();
                                if target_dist > 0.0001 {
                                    movement = to_target / target_dist * props.speed;
                                    mob.yaw = movement.y.atan2(movement.x);
                                }
                                if target_dist <= 0.5 || mob.ai_timer <= 0.0 {
                                    mob.ai_state = MobAiState::Idle;
                                    mob.ai_timer = self.gameplay_rand_range(2.0, 5.0);
                                    mob.wander_target = None;
                                }
                            }
                        }
                    }
                    MobAiState::Chasing => {
                        if player_distance > props.detection_range * 1.5 {
                            mob.ai_state = MobAiState::Idle;
                            mob.ai_timer = self.gameplay_rand_range(2.0, 5.0);
                            mob.wander_target = None;
                        } else {
                            if player_distance > 0.0001 {
                                movement = to_player / player_distance * props.speed;
                                mob.yaw = movement.y.atan2(movement.x);
                            }
                            if player_distance <= props.attack_range && mob.attack_cooldown <= 0.0 {
                                pending_player_damage += props.attack_damage;
                                mob.attack_cooldown = 1.0;
                            }
                        }
                    }
                    MobAiState::Fleeing => {
                        mob.ai_state = MobAiState::Idle;
                    }
                }
            } else {
                match mob.ai_state {
                    MobAiState::Idle => {
                        if mob.ai_timer <= 0.0 {
                            mob.ai_state = MobAiState::Wandering;
                            mob.ai_timer = 5.0;
                            mob.wander_target = None;
                        }
                    }
                    MobAiState::Wandering => {
                        if mob.wander_target.is_none() {
                            let angle = self.gameplay_rand_range(0.0, std::f32::consts::TAU);
                            let distance = self.gameplay_rand_range(3.0, 8.0);
                            mob.wander_target = Some([
                                position.x + angle.cos() * distance,
                                position.y,
                                position.z + angle.sin() * distance,
                            ]);
                        }
                        if let Some(target) = mob.wander_target {
                            let to_target = Vec2::new(target[0] - position.x, target[2] - position.z);
                            let target_dist = to_target.length();
                            if target_dist > 0.0001 {
                                movement = to_target / target_dist * props.speed;
                                mob.yaw = movement.y.atan2(movement.x);
                            }
                            if target_dist <= 0.5 || mob.ai_timer <= 0.0 {
                                mob.ai_state = MobAiState::Idle;
                                mob.ai_timer = self.gameplay_rand_range(2.0, 5.0);
                                mob.wander_target = None;
                            }
                        }
                    }
                    MobAiState::Fleeing => {
                        let away = Vec2::new(position.x - player_pos.x, position.z - player_pos.z);
                        let away_dist = away.length();
                        if away_dist > 0.0001 {
                            movement = away / away_dist * props.speed * 1.6;
                            mob.yaw = movement.y.atan2(movement.x);
                        }
                        if mob.ai_timer <= 0.0 {
                            mob.ai_state = MobAiState::Idle;
                            mob.ai_timer = self.gameplay_rand_range(2.0, 5.0);
                            mob.wander_target = None;
                        }
                    }
                    MobAiState::Chasing => {
                        mob.ai_state = MobAiState::Idle;
                    }
                }
            }

            if movement.length_squared() > 0.0 {
                position.x += movement.x * dt;
                position.z += movement.y * dt;
            }

            if let Some(surface_y) =
                self.find_surface_y(position.x.floor() as i32, position.z.floor() as i32, registry)
            {
                ground_y = surface_y + 0.01;
                if position.y < ground_y {
                    position.y = ground_y;
                    velocity.y = 0.0;
                }
            }

            mob.position = position.to_array();
            mob.velocity = velocity.to_array();

            if mob.is_dead() {
                self.drop_mob_loot(mob.mob_type, position);
                continue;
            }

            alive_mobs.push(mob);
        }

        alive_mobs.reverse();
        self.mobs = alive_mobs;

        if pending_player_damage > 0.0 {
            self.apply_damage(pending_player_damage);
        }
    }

    fn held_attack_damage(&self) -> f32 {
        let Some(held_item) = self
            .inventory
            .get(self.selected_hotbar_slot)
            .filter(|stack| stack.count > 0)
            .map(|stack| stack.item)
        else {
            return 1.0;
        };

        if let Some((kind, tier)) = tool_properties(held_item) {
            if kind == ToolKind::Sword {
                let tier_bonus = match tier {
                    ToolTier::Wood => 0.0,
                    ToolTier::Stone => 1.0,
                    ToolTier::Iron => 2.0,
                    ToolTier::Diamond => 3.0,
                    ToolTier::Gold => 1.5,
                };
                return 4.0 + tier_bonus;
            }
            return 2.0;
        }
        1.0
    }

    fn try_attack_mob_in_front(&mut self) -> bool {
        if self.mobs.is_empty() {
            return false;
        }

        let mut forward = self.camera.forward_direction();
        forward.y = 0.0;
        forward = forward.normalize_or_zero();
        if forward.length_squared() <= 1e-6 {
            return false;
        }

        let attack_origin = self.player_pos + Vec3::new(0.0, EYE_HEIGHT * 0.5, 0.0);
        let mut target_idx: Option<usize> = None;
        let mut nearest_dist_sq = f32::MAX;

        for (idx, mob) in self.mobs.iter().enumerate() {
            let mob_position = Vec3::from_array(mob.position);
            let to_mob = mob_position - attack_origin;
            let dist_sq = to_mob.length_squared();
            if dist_sq > 9.0 {
                continue;
            }

            let horizontal = Vec3::new(to_mob.x, 0.0, to_mob.z);
            let direction = horizontal.normalize_or_zero();
            if direction.length_squared() <= 1e-6 {
                continue;
            }

            if direction.dot(forward) < 0.35 {
                continue;
            }

            if dist_sq < nearest_dist_sq {
                nearest_dist_sq = dist_sq;
                target_idx = Some(idx);
            }
        }

        let Some(target_idx) = target_idx else {
            return false;
        };

        let damage = self.held_attack_damage();
        let mut dead_mob: Option<(MobType, Vec3)> = None;

        {
            let mob = &mut self.mobs[target_idx];
            mob.health -= damage;
            mob.hurt_timer = 0.3;

            let mut knockback_dir = Vec3::new(
                mob.position[0] - self.player_pos.x,
                0.0,
                mob.position[2] - self.player_pos.z,
            )
            .normalize_or_zero();
            if knockback_dir.length_squared() <= 1e-6 {
                knockback_dir = forward;
            }
            mob.velocity[0] += knockback_dir.x * 5.0;
            mob.velocity[2] += knockback_dir.z * 5.0;

            if !mob_properties(mob.mob_type).hostile {
                mob.ai_state = MobAiState::Fleeing;
                mob.ai_timer = 3.0;
                mob.wander_target = None;
            }

            if mob.is_dead() {
                dead_mob = Some((mob.mob_type, Vec3::from_array(mob.position)));
            }
        }

        if let Some((mob_type, mob_pos)) = dead_mob {
            self.mobs.swap_remove(target_idx);
            self.drop_mob_loot(mob_type, mob_pos);
            let xp_reward = self.mob_xp_reward(mob_type);
            self.add_xp(xp_reward);
        }

        true
    }

    fn maybe_trample_farmland_on_landing(&mut self) {
        if !matches!(self.game_mode, Some(GameMode::Singleplayer)) {
            return;
        }

        let ground_pos = IVec3::new(
            self.player_pos.x.floor() as i32,
            (self.player_pos.y - 0.05).floor() as i32,
            self.player_pos.z.floor() as i32,
        );
        if block_at(ground_pos, &self.chunks) != Some(BlockId::FARMLAND) {
            return;
        }

        if simple_rng_next(&mut self.gameplay_rng_state) % 100 >= 20 {
            return;
        }

        if let Some((chunk_pos, local_pos, previous, next)) =
            try_set_world_block(&mut self.chunks, ground_pos, BlockId::LOAM)
        {
            self.dirty_chunks.insert(chunk_pos);
            self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
        }
    }

    fn respawn(&mut self) {
        self.player_pos = self.spawn_position;
        self.velocity = Vec3::ZERO;
        self.on_ground = false;
        self.health = MAX_HEALTH;
        self.hunger = MAX_HUNGER;
        self.air_supply = MAX_AIR_SUPPLY;
        self.fall_start_y = None;
        self.damage_flash_timer = 0.0;
    }

    fn take_chest_inventory_contents(&mut self, chest_pos: IVec3) -> Vec<ItemStack> {
        if self.open_chest == Some(chest_pos) || self.double_chest_partner == Some(chest_pos) {
            self.persist_open_double_chest();
        }

        let mut drops = Vec::new();
        if let Some(mut inventory) = self.chest_inventories.remove(&chest_pos) {
            for slot in 0..SINGLE_CHEST_SLOT_COUNT {
                if let Some(stack) = inventory.slots[slot].take() {
                    if stack.count > 0 {
                        drops.push(stack);
                    }
                }
            }
        }
        if self.open_chest == Some(chest_pos) || self.double_chest_partner == Some(chest_pos) {
            self.set_inventory_open(false);
        }
        drops
    }

    fn is_ore_xp_block(block: BlockId) -> bool {
        matches!(block.0, 11 | 16 | 17 | 18 | 19)
    }

    fn is_stone_xp_block(block: BlockId) -> bool {
        block == BlockId(2) || block == BlockId::RUBBLESTONE
    }

    fn add_xp_for_broken_block(&mut self, block: BlockId) {
        if !self.is_survival_mode() {
            return;
        }

        if Self::is_ore_xp_block(block) {
            let xp = 3 + (simple_rng_next(&mut self.gameplay_rng_state) % 5) as u32;
            self.add_xp(xp);
            return;
        }

        if Self::is_stone_xp_block(block)
            && simple_rng_next(&mut self.gameplay_rng_state) % 10 == 0
        {
            self.add_xp(1);
        }
    }

    fn adjacent_block_positions(pos: IVec3) -> [IVec3; 6] {
        [
            pos + IVec3::X,
            pos - IVec3::X,
            pos + IVec3::Y,
            pos - IVec3::Y,
            pos + IVec3::Z,
            pos - IVec3::Z,
        ]
    }

    fn set_block_and_collect_update(
        &mut self,
        world_pos: IVec3,
        new_block: BlockId,
        remesh_updates: &mut Vec<(ChunkPos, LocalPos, BlockId, BlockId)>,
        block_edits: &mut Vec<(IVec3, BlockId)>,
    ) -> bool {
        if let Some((chunk_pos, local_pos, previous, next)) =
            try_set_world_block(&mut self.chunks, world_pos, new_block)
        {
            self.dirty_chunks.insert(chunk_pos);
            remesh_updates.push((chunk_pos, local_pos, previous, next));
            block_edits.push((world_pos, new_block));
            return true;
        }
        false
    }

    fn set_adjacent_doors_open_with_updates(
        &mut self,
        source_pos: IVec3,
        open: bool,
        remesh_updates: &mut Vec<(ChunkPos, LocalPos, BlockId, BlockId)>,
        block_edits: &mut Vec<(IVec3, BlockId)>,
    ) {
        let mut processed_doors: HashSet<IVec3> = HashSet::new();
        for neighbor in Self::adjacent_block_positions(source_pos) {
            let Some(block) = block_at(neighbor, &self.chunks) else {
                continue;
            };
            let Some((lower_pos, upper_pos, facing, is_open)) =
                door_interaction_state(neighbor, block, &self.chunks)
            else {
                continue;
            };
            if !processed_doors.insert(lower_pos) || is_open == open {
                continue;
            }
            let lower_block = door_block_for_state(facing, open, false);
            let upper_block = door_block_for_state(facing, open, true);
            self.set_block_and_collect_update(lower_pos, lower_block, remesh_updates, block_edits);
            self.set_block_and_collect_update(upper_pos, upper_block, remesh_updates, block_edits);
        }
    }

    fn toggle_adjacent_doors_with_updates(
        &mut self,
        source_pos: IVec3,
        remesh_updates: &mut Vec<(ChunkPos, LocalPos, BlockId, BlockId)>,
        block_edits: &mut Vec<(IVec3, BlockId)>,
    ) {
        let mut processed_doors: HashSet<IVec3> = HashSet::new();
        for neighbor in Self::adjacent_block_positions(source_pos) {
            let Some(block) = block_at(neighbor, &self.chunks) else {
                continue;
            };
            let Some((lower_pos, upper_pos, facing, is_open)) =
                door_interaction_state(neighbor, block, &self.chunks)
            else {
                continue;
            };
            if !processed_doors.insert(lower_pos) {
                continue;
            }
            let new_open = !is_open;
            let lower_block = door_block_for_state(facing, new_open, false);
            let upper_block = door_block_for_state(facing, new_open, true);
            self.set_block_and_collect_update(lower_pos, lower_block, remesh_updates, block_edits);
            self.set_block_and_collect_update(upper_pos, upper_block, remesh_updates, block_edits);
        }
    }

    fn try_apply_bone_meal(
        &mut self,
        target_pos: IVec3,
        remesh_updates: &mut Vec<(ChunkPos, LocalPos, BlockId, BlockId)>,
        block_edits: &mut Vec<(IVec3, BlockId)>,
    ) -> bool {
        let is_holding_bone_meal = self
            .inventory
            .get(self.selected_hotbar_slot)
            .is_some_and(|stack| stack.item == ItemId::BONE_MEAL && stack.count > 0);
        if !is_holding_bone_meal {
            return false;
        }

        let Some(target_block) = block_at(target_pos, &self.chunks) else {
            return false;
        };

        let mut changed = false;
        if let Some(stage) = wheat_growth_stage(target_block) {
            if stage < 7 {
                changed = self.set_block_and_collect_update(
                    target_pos,
                    BlockId::WHEAT_STAGE_7,
                    remesh_updates,
                    block_edits,
                );
            }
        } else if target_block == BlockId::SAPLING {
            if matches!(self.game_mode, Some(GameMode::Singleplayer)) && self.try_grow_sapling_tree(target_pos) {
                self.sapling_growth_timers.remove(&target_pos);
                changed = true;
            } else {
                changed = self.set_block_and_collect_update(
                    target_pos,
                    BlockId::TIMBER_LOG,
                    remesh_updates,
                    block_edits,
                );
                let canopy_pos = target_pos + IVec3::Y;
                if matches!(block_at(canopy_pos, &self.chunks), Some(BlockId::AIR | BlockId::CANOPY_LEAVES)) {
                    changed |= self.set_block_and_collect_update(
                        canopy_pos,
                        BlockId::CANOPY_LEAVES,
                        remesh_updates,
                        block_edits,
                    );
                }
                if changed {
                    self.sapling_growth_timers.remove(&target_pos);
                }
            }
        } else if target_block == BlockId::SUGAR_CANE {
            let mut base_pos = target_pos;
            while block_at(base_pos - IVec3::Y, &self.chunks) == Some(BlockId::SUGAR_CANE) {
                base_pos -= IVec3::Y;
            }
            let height = sugar_cane_height(base_pos, &self.chunks);
            for y in height..SUGAR_CANE_MAX_HEIGHT {
                let grow_pos = base_pos + IVec3::new(0, y, 0);
                if block_at(grow_pos, &self.chunks) != Some(BlockId::AIR) {
                    break;
                }
                changed |= self.set_block_and_collect_update(
                    grow_pos,
                    BlockId::SUGAR_CANE,
                    remesh_updates,
                    block_edits,
                );
            }
        }

        if changed {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.spawn_break_particles(target_pos, [0.72, 0.92, 0.44]);
            }
        }
        changed
    }

    fn break_targeted_block_now(&mut self, targeted_block: IVec3) {
        let Some(registry) = self.registry.clone() else {
            return;
        };

        let Some(target_block_id) = block_at(targeted_block, &self.chunks) else {
            return;
        };
        if !is_block_break_target(target_block_id, &registry) {
            return;
        }

        let mut block_edits_to_send: Vec<(IVec3, BlockId)> = Vec::new();
        let mut break_particles: Option<(IVec3, [f32; 3])> = None;
        let mut remesh_updates: Vec<(ChunkPos, LocalPos, BlockId, BlockId)> = Vec::new();
        let mut drops_to_spawn: Vec<(IVec3, ItemStack)> = Vec::new();
        let mut chest_drops: Vec<ItemStack> = Vec::new();
        let mut detonated_tnt = false;

        if let Some((lower_pos, upper_pos)) =
            door_pair_positions_for_interaction(targeted_block, target_block_id, &self.chunks)
        {
            for pos in [lower_pos, upper_pos] {
                if let Some((chunk_pos, local_pos, previous, next)) =
                    try_set_world_block(&mut self.chunks, pos, BlockId::AIR)
                {
                    self.dirty_chunks.insert(chunk_pos);
                    remesh_updates.push((chunk_pos, local_pos, previous, next));
                    block_edits_to_send.push((pos, BlockId::AIR));
                }
            }

            if !block_edits_to_send.is_empty() {
                break_particles = Some((
                    targeted_block,
                    block_particle_color(target_block_id, &registry),
                ));
                if let Some(drop_block) = drop_block_for_break(target_block_id) {
                    drops_to_spawn.push((lower_pos, ItemStack::new(ItemId::from(drop_block), 1)));
                }
            }
        } else if let Some((chunk_pos, local_pos, previous, next)) =
            try_set_world_block(&mut self.chunks, targeted_block, BlockId::AIR)
        {
            self.dirty_chunks.insert(chunk_pos);
            remesh_updates.push((chunk_pos, local_pos, previous, next));
            block_edits_to_send.push((targeted_block, BlockId::AIR));
            self.add_xp_for_broken_block(target_block_id);
            break_particles = Some((
                targeted_block,
                block_particle_color(target_block_id, &registry),
            ));
            if target_block_id == BlockId::CHEST {
                chest_drops = self.take_chest_inventory_contents(targeted_block);
            }
            if target_block_id == BlockId::TNT && matches!(self.game_mode, Some(GameMode::Singleplayer)) {
                detonated_tnt = true;
            } else if is_wheat_block(target_block_id) {
                if wheat_growth_stage(target_block_id) == Some(7) {
                    drops_to_spawn.push((
                        targeted_block,
                        ItemStack::new(ItemId::WHEAT_ITEM, 1),
                    ));
                    let seed_count = (simple_rng_next(&mut self.gameplay_rng_state) % 4) as u8;
                    if seed_count > 0 {
                        drops_to_spawn.push((
                            targeted_block,
                            ItemStack::new(ItemId::WHEAT_SEEDS, seed_count),
                        ));
                    }
                } else {
                    drops_to_spawn.push((
                        targeted_block,
                        ItemStack::new(ItemId::WHEAT_SEEDS, 1),
                    ));
                }
            } else if let Some(drop_block) = drop_block_for_break(target_block_id) {
                drops_to_spawn.push((targeted_block, ItemStack::new(ItemId::from(drop_block), 1)));
            }
        }

        for (chunk_pos, local_pos, previous, next) in remesh_updates {
            self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
        }

        for (drop_pos, drop_stack) in drops_to_spawn {
            self.spawn_item_drop(drop_pos, drop_stack);
        }
        for stack in chest_drops {
            self.spawn_item_drop(targeted_block, stack);
        }
        if detonated_tnt {
            self.enqueue_tnt_explosion(targeted_block);
        }

        if let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode {
            for (world_pos, new_block) in &block_edits_to_send {
                info!("Sending block edit {world_pos:?} -> block {}", new_block.0);
                net.send_reliable(&C2S::BlockEdit {
                    world_pos: *world_pos,
                    new_block: *new_block,
                });
            }
        }

        if let Some((particle_pos, color)) = break_particles {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.spawn_break_particles(particle_pos, color);
            }
        }

        // Schedule fluid update only when the broken block is fluid or adjacent to fluid
        if !block_edits_to_send.is_empty() && self.is_near_fluid(targeted_block, target_block_id) {
            self.schedule_fluid_update(targeted_block);
        }
    }

    fn spawn_item_drop(&mut self, block_pos: IVec3, stack: ItemStack) {
        if stack.is_empty() {
            return;
        }

        let spawn_pos = Vec3::new(
            block_pos.x as f32 + 0.5,
            block_pos.y as f32 + 0.5,
            block_pos.z as f32 + 0.5,
        );
        let initial_velocity = Vec3::new(0.0, 2.35, 0.0);
        self.item_drops.push(ItemDrop::new(spawn_pos, initial_velocity, stack));
    }

    fn update_item_drops(&mut self, dt: f32) {
        if self.item_drops.is_empty() {
            return;
        }

        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }

        let pickup_target = self.player_pos + Vec3::new(0.0, EYE_HEIGHT * 0.5, 0.0);
        let magnet_radius_sq = ITEM_DROP_MAGNET_RADIUS * ITEM_DROP_MAGNET_RADIUS;
        let pickup_radius_sq = ITEM_DROP_PICKUP_RADIUS * ITEM_DROP_PICKUP_RADIUS;
        let mut inventory_changed = false;

        let mut i = 0usize;
        while i < self.item_drops.len() {
            let mut remove_drop = false;
            let mut pickup_request: Option<(ItemId, u8)> = None;

            {
                let drop = &mut self.item_drops[i];
                drop.age += dt;
                if drop.age >= drop.lifetime || drop.item.is_empty() {
                    remove_drop = true;
                } else {
                    let to_player = pickup_target - drop.position;
                    let dist_sq = to_player.length_squared();
                    if dist_sq <= magnet_radius_sq && dist_sq > 1e-6 {
                        let dist = dist_sq.sqrt();
                        let direction = to_player / dist;
                        let pull = ((ITEM_DROP_MAGNET_RADIUS - dist) / ITEM_DROP_MAGNET_RADIUS)
                            .clamp(0.2, 1.0);
                        drop.velocity += direction * ITEM_DROP_MAGNET_ACCEL * pull * dt;
                    }

                    drop.velocity.y = (drop.velocity.y + ITEM_DROP_GRAVITY * dt).max(-36.0);
                    drop.position += drop.velocity * dt;

                    if let Some(registry) = self.registry.as_ref() {
                        let block_x = drop.position.x.floor() as i32;
                        let block_y = (drop.position.y - ITEM_DROP_HALF_SIZE).floor() as i32;
                        let block_z = drop.position.z.floor() as i32;
                        if is_block_solid(block_x, block_y, block_z, &self.chunks, registry) {
                            let ground_y = block_y as f32 + 1.0 + ITEM_DROP_HALF_SIZE;
                            if drop.position.y < ground_y {
                                drop.position.y = ground_y;
                            }
                            if drop.velocity.y < 0.0 {
                                drop.velocity.y = 0.0;
                            }
                            let ground_drag = (1.0 - 10.0 * dt).clamp(0.0, 1.0);
                            drop.velocity.x *= ground_drag;
                            drop.velocity.z *= ground_drag;
                        } else {
                            let air_drag = (1.0 - 1.5 * dt).clamp(0.0, 1.0);
                            drop.velocity.x *= air_drag;
                            drop.velocity.z *= air_drag;
                        }
                    }

                    if drop.age >= drop.pickup_delay
                        && (pickup_target - drop.position).length_squared() <= pickup_radius_sq
                    {
                        pickup_request = Some((drop.item.item, drop.item.count));
                    }
                }
            }

            if remove_drop {
                self.item_drops.swap_remove(i);
                continue;
            }

            if let Some((item, count)) = pickup_request {
                let remaining = self.inventory.add_item(item, count);
                let picked_up = count.saturating_sub(remaining);
                if item == ItemId::COAL && picked_up > 0 && self.is_survival_mode() {
                    self.add_xp(u32::from(picked_up));
                }
                if remaining == 0 {
                    inventory_changed = true;
                    self.item_drops.swap_remove(i);
                    continue;
                }
                if remaining < count {
                    inventory_changed = true;
                }
                if let Some(drop) = self.item_drops.get_mut(i) {
                    drop.item.count = remaining;
                }
            }

            i += 1;
        }

        if inventory_changed {
            self.refresh_selected_block();
        }
    }

    fn build_item_drop_render_data(&self) -> Vec<ItemDropRenderData> {
        let Some(registry) = self.registry.as_ref() else {
            return Vec::new();
        };

        let atlas_mapping = self.renderer.as_ref().map(|r| r.atlas_mapping());

        self.item_drops
            .iter()
            .filter(|drop| !drop.item.is_empty())
            .map(|drop| {
                let tile_origin = atlas_mapping.and_then(|m| m.offset_for_item(drop.item.item));
                ItemDropRenderData {
                    position: drop.position,
                    color: item_particle_color(drop.item.item, registry),
                    age: drop.age,
                    tile_origin,
                }
            })
            .collect()
    }

    fn tick_falling_blocks(&mut self) {
        // Check blocks near the player for gravity-affected types
        let px = self.player_pos.x.floor() as i32;
        let py = self.player_pos.y.floor() as i32;
        let pz = self.player_pos.z.floor() as i32;
        let radius = 6;
        let y_min = (py - 8).max(0);
        let y_max = py + 16;
        let mut moves: Vec<(IVec3, BlockId)> = Vec::new();

        for x in (px - radius)..=(px + radius) {
            for z in (pz - radius)..=(pz + radius) {
                for y in y_min..=y_max {
                    let pos = IVec3::new(x, y, z);
                    if let Some(block) = block_at(pos, &self.chunks) {
                        // sand=29, dune_sand=5, gravel_bed=14
                        if block.0 == 29 || block.0 == 5 || block.0 == 14 {
                            let below = IVec3::new(x, y - 1, z);
                            if let Some(below_block) = block_at(below, &self.chunks) {
                                if below_block == BlockId::AIR {
                                    moves.push((pos, block));
                                }
                            }
                        }
                    }
                }
            }
        }

        for (pos, block) in moves {
            let below = IVec3::new(pos.x, pos.y - 1, pos.z);
            // Set top to air
            set_block_at(pos, BlockId::AIR, &mut self.chunks);
            // Set bottom to the block
            set_block_at(below, block, &mut self.chunks);
            // Mark chunks dirty
            let (cp1, _) = world_to_chunk(pos);
            let (cp2, _) = world_to_chunk(below);
            self.dirty_chunks.insert(cp1);
            self.dirty_chunks.insert(cp2);
            // Queue remesh
            if self.mesh_queue_set.insert(cp1) {
                self.mesh_queue.push_back(cp1);
            }
            if cp1 != cp2 && self.mesh_queue_set.insert(cp2) {
                self.mesh_queue.push_back(cp2);
            }
        }
    }

    /// Returns true if the block itself or any adjacent block is water or lava.
    fn is_near_fluid(&self, pos: IVec3, block_id: BlockId) -> bool {
        if is_water_block(block_id) || is_lava_block(block_id) {
            return true;
        }
        const OFFSETS: [IVec3; 6] = [
            IVec3::new(1, 0, 0),
            IVec3::new(-1, 0, 0),
            IVec3::new(0, 1, 0),
            IVec3::new(0, -1, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(0, 0, -1),
        ];
        for offset in OFFSETS {
            if let Some(neighbor) = block_at(pos + offset, &self.chunks) {
                if is_water_block(neighbor) || is_lava_block(neighbor) {
                    return true;
                }
            }
        }
        false
    }

    fn enqueue_tnt_explosion(&mut self, pos: IVec3) {
        if !self.tnt_explosion_queue.contains(&pos) {
            self.tnt_explosion_queue.push_back(pos);
        }
    }

    fn process_tnt_explosion_queue(&mut self) {
        let explosions_this_frame = self.tnt_explosion_queue.len().min(2);
        for _ in 0..explosions_this_frame {
            let Some(center) = self.tnt_explosion_queue.pop_front() else {
                break;
            };
            self.explode_tnt(center);
        }
    }

    fn tick_fire_spread(&mut self) {
        let fire_positions = std::mem::take(&mut self.block_scan_state.pending_fire);
        if fire_positions.is_empty() {
            return;
        }

        let offsets = [
            IVec3::X,
            IVec3::NEG_X,
            IVec3::Y,
            IVec3::NEG_Y,
            IVec3::Z,
            IVec3::NEG_Z,
        ];
        let mut extinguish: Vec<IVec3> = Vec::new();
        let mut new_fires: Vec<IVec3> = Vec::new();
        let mut remesh_chunks = HashSet::new();

        for fire_pos in fire_positions {
            if !block_at(fire_pos, &self.chunks).is_some_and(is_fire_block) {
                continue;
            }

            let mut flammable_neighbors = Vec::new();
            for offset in offsets {
                let neighbor = fire_pos + offset;
                if let Some(block) = block_at(neighbor, &self.chunks) {
                    if block == BlockId::TNT {
                        self.enqueue_tnt_explosion(neighbor);
                    }
                    if is_flammable(block) {
                        flammable_neighbors.push(neighbor);
                    }
                }
            }

            // Fire extinguishes immediately if there is no nearby fuel.
            if flammable_neighbors.is_empty() {
                extinguish.push(fire_pos);
                continue;
            }

            if matches!(self.weather_state, WeatherState::Rain) && self.gameplay_rand_f32() < 0.5 {
                extinguish.push(fire_pos);
                continue;
            }

            if self.gameplay_rand_f32() < 0.05 {
                extinguish.push(fire_pos);
                continue;
            }

            if self.gameplay_rand_f32() < 0.10 {
                let spread_idx = (self.next_gameplay_rand_u32() as usize) % flammable_neighbors.len();
                new_fires.push(flammable_neighbors[spread_idx]);
            }
        }

        for fire_pos in extinguish {
            if let Some((chunk_pos, _, _, _)) = try_set_world_block(&mut self.chunks, fire_pos, BlockId::AIR) {
                self.dirty_chunks.insert(chunk_pos);
                remesh_chunks.insert(chunk_pos);
            }
        }
        for spread_pos in new_fires {
            if !block_at(spread_pos, &self.chunks).is_some_and(is_flammable) {
                continue;
            }
            if let Some((chunk_pos, _, _, _)) = try_set_world_block(&mut self.chunks, spread_pos, BlockId::FIRE)
            {
                self.dirty_chunks.insert(chunk_pos);
                remesh_chunks.insert(chunk_pos);
            }
        }

        for chunk_pos in remesh_chunks {
            self.trigger_remesh(chunk_pos);
        }
    }

    fn tick_furnaces(&mut self, dt: f32) {
        for (&pos, state) in &mut self.furnace_data {
            // Only tick furnaces near player (within 8 chunks)
            let dx = (pos.x as f32 - self.player_pos.x).abs();
            let dz = (pos.z as f32 - self.player_pos.z).abs();
            if dx > 256.0 || dz > 256.0 {
                continue;
            }
            // Consume fuel if needed
            if state.fuel_remaining <= 0.0 {
                if let Some(fuel_stack) = &mut state.fuel {
                    if let Some(burn_time) = recipe::fuel_burn_time_secs(fuel_stack.item) {
                        state.fuel_remaining = burn_time;
                        state.fuel_total = burn_time;
                        fuel_stack.count -= 1;
                        if fuel_stack.count == 0 {
                            state.fuel = None;
                        }
                    }
                }
            }
            // Smelt if we have fuel and input
            if state.fuel_remaining > 0.0 {
                state.fuel_remaining -= dt;
                if let Some(input_stack) = &state.input {
                    if let Some(smelt_recipe) = recipe::find_smelting_recipe(input_stack.item) {
                        state.smelt_progress += dt;
                        if state.smelt_progress >= smelt_recipe.smelt_time_secs {
                            state.smelt_progress = 0.0;
                            // Move input to output
                            let output_item = smelt_recipe.output;
                            if let Some(out) = &mut state.output {
                                if out.item == output_item.item && out.count < 64 {
                                    out.count += output_item.count;
                                }
                            } else {
                                state.output = Some(output_item);
                            }
                            // Consume input
                            if let Some(inp) = &mut state.input {
                                inp.count -= 1;
                                if inp.count == 0 {
                                    state.input = None;
                                }
                            }
                        }
                    } else {
                        state.smelt_progress = 0.0;
                    }
                } else {
                    state.smelt_progress = 0.0;
                }
            } else {
                state.smelt_progress = 0.0;
            }
        }
    }

    fn explode_tnt(&mut self, center: IVec3) {
        self.push_chat_message(
            "System",
            &format!("TNT detonated at ({}, {}, {})", center.x, center.y, center.z),
        );

        let mut affected_chunks = HashSet::new();
        let radius_i = TNT_EXPLOSION_RADIUS.ceil() as i32;

        for x in -radius_i..=radius_i {
            for y in -radius_i..=radius_i {
                for z in -radius_i..=radius_i {
                    let offset = Vec3::new(x as f32, y as f32, z as f32);
                    if offset.length() > TNT_EXPLOSION_RADIUS {
                        continue;
                    }

                    let pos = center + IVec3::new(x, y, z);
                    let Some(block) = block_at(pos, &self.chunks) else {
                        continue;
                    };
                    if block == BlockId::AIR
                        || block == BlockId(1)
                        || block == BlockId::OBSIDIAN
                        || is_water_block(block)
                        || is_lava_block(block)
                    {
                        continue;
                    }

                    if let Some((chunk_pos, _, previous, _)) =
                        try_set_world_block(&mut self.chunks, pos, BlockId::AIR)
                    {
                        self.dirty_chunks.insert(chunk_pos);
                        affected_chunks.insert(chunk_pos);

                        if previous == BlockId::TNT && pos != center {
                            self.enqueue_tnt_explosion(pos);
                        }

                        if self.gameplay_rand_f32() < 0.30 {
                            if let Some(item) = block_drop_item(previous) {
                                let drop_position = Vec3::new(
                                    pos.x as f32 + 0.5,
                                    pos.y as f32 + 0.5,
                                    pos.z as f32 + 0.5,
                                );
                                let drop_velocity = Vec3::new(
                                    self.gameplay_rand_range(-1.5, 1.5),
                                    self.gameplay_rand_range(2.0, 4.0),
                                    self.gameplay_rand_range(-1.5, 1.5),
                                );
                                self.item_drops.push(ItemDrop::new(
                                    drop_position,
                                    drop_velocity,
                                    ItemStack::new(item, 1),
                                ));
                            }
                        }
                    }
                }
            }
        }

        for chunk_pos in affected_chunks {
            self.trigger_remesh(chunk_pos);
        }

        let center_pos = Vec3::new(
            center.x as f32 + 0.5,
            center.y as f32 + 0.5,
            center.z as f32 + 0.5,
        );
        let to_player = self.player_pos - center_pos;
        let distance = to_player.length();
        if distance <= TNT_EXPLOSION_RADIUS {
            let direction = if distance > 0.0001 {
                to_player / distance
            } else {
                Vec3::Y
            };
            let strength = (TNT_EXPLOSION_RADIUS - distance).max(0.0) * 2.0;
            self.velocity += direction * strength;
            let damage = (TNT_EXPLOSION_RADIUS - distance).max(0.0) * 5.0;
            self.apply_damage(damage);
        }
    }

    fn tick_pressure_plates(&mut self) {
        let plate_pos = IVec3::new(
            self.player_pos.x.floor() as i32,
            (self.player_pos.y - 0.05).floor() as i32,
            self.player_pos.z.floor() as i32,
        );

        let mut currently_active: HashSet<(i32, i32, i32)> = HashSet::new();
        if block_at(plate_pos, &self.chunks) == Some(BlockId::STONE_PRESSURE_PLATE) {
            currently_active.insert((plate_pos.x, plate_pos.y, plate_pos.z));
        }

        let mut remesh_updates: Vec<(ChunkPos, LocalPos, BlockId, BlockId)> = Vec::new();
        let mut block_edits: Vec<(IVec3, BlockId)> = Vec::new();

        for &(x, y, z) in &currently_active {
            if self.active_pressure_plates.contains(&(x, y, z)) {
                continue;
            }
            let pos = IVec3::new(x, y, z);
            self.set_adjacent_doors_open_with_updates(pos, true, &mut remesh_updates, &mut block_edits);
        }

        let previously_active: Vec<(i32, i32, i32)> =
            self.active_pressure_plates.iter().copied().collect();
        for (x, y, z) in previously_active {
            if currently_active.contains(&(x, y, z)) {
                continue;
            }
            let pos = IVec3::new(x, y, z);
            self.set_adjacent_doors_open_with_updates(
                pos,
                false,
                &mut remesh_updates,
                &mut block_edits,
            );
        }

        for (chunk_pos, local_pos, previous, next) in remesh_updates {
            self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
        }

        self.active_pressure_plates = currently_active;
    }

    fn tick_button_timers(&mut self, dt: f32) {
        if self.button_timers.is_empty() {
            return;
        }

        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }

        let mut expired_positions = Vec::new();
        for (pos, remaining) in &mut self.button_timers {
            *remaining -= dt;
            if *remaining <= 0.0 {
                expired_positions.push(*pos);
            }
        }
        self.button_timers.retain(|(_, remaining)| *remaining > 0.0);

        let mut remesh_updates: Vec<(ChunkPos, LocalPos, BlockId, BlockId)> = Vec::new();
        let mut block_edits: Vec<(IVec3, BlockId)> = Vec::new();
        for button_pos in expired_positions {
            if block_at(button_pos, &self.chunks) != Some(BlockId::STONE_BUTTON_ON) {
                continue;
            }
            self.set_block_and_collect_update(
                button_pos,
                BlockId::STONE_BUTTON_OFF,
                &mut remesh_updates,
                &mut block_edits,
            );
            self.set_adjacent_doors_open_with_updates(
                button_pos,
                false,
                &mut remesh_updates,
                &mut block_edits,
            );
        }

        for (chunk_pos, local_pos, previous, next) in remesh_updates {
            self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
        }

        if let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode {
            for (world_pos, new_block) in block_edits {
                net.send_reliable(&C2S::BlockEdit { world_pos, new_block });
            }
        }
    }

    /// Schedule fluid simulation for the chunk containing `world_pos`.
    /// Called when a block is placed or broken near water/lava.
    fn schedule_fluid_update(&mut self, world_pos: IVec3) {
        let (chunk_pos, _) = world_to_chunk(world_pos);
        self.pending_fluid_positions.insert(chunk_pos);
    }

    /// Process pending fluid updates — at most one chunk per frame to avoid freezes.
    fn process_pending_fluid_updates(&mut self) {
        if self.pending_fluid_positions.is_empty() {
            return;
        }
        // Take ONE pending position per frame to avoid stalling
        let center = {
            let pos = *self.pending_fluid_positions.iter().next().unwrap();
            self.pending_fluid_positions.remove(&pos);
            pos
        };
        // Simulate only the chunk and its immediate neighbors
        let water_changes = simulate_water_near(&mut self.chunks, Some((center, 1)));
        self.lava_simulation_frame_accumulator =
            self.lava_simulation_frame_accumulator.saturating_add(1);
        let lava_changes =
            if self.lava_simulation_frame_accumulator >= LAVA_SIMULATION_INTERVAL_FRAMES {
                self.lava_simulation_frame_accumulator = 0;
                simulate_lava_near(&mut self.chunks, Some((center, 1)))
            } else {
                Vec::new()
            };
        let mut remesh_set = HashSet::new();
        for change in water_changes.iter().chain(lava_changes.iter()) {
            let (chunk_pos, _) = world_to_chunk(change.world_pos);
            self.dirty_chunks.insert(chunk_pos);
            remesh_set.insert(chunk_pos);
        }
        for chunk_pos in remesh_set {
            self.remesh_chunk(chunk_pos);
        }
        // If there were changes, re-schedule this chunk for further propagation next frame
        if !water_changes.is_empty() || !lava_changes.is_empty() {
            self.pending_fluid_positions.insert(center);
        }
    }

    fn rebuild_block_scan_chunk_list(&mut self) {
        let player_chunk = self.player_chunk_pos();
        self.block_scan_state.chunk_list = self
            .chunks
            .keys()
            .copied()
            .filter(|chunk_pos| {
                let dx = (chunk_pos.x - player_chunk.x).abs();
                let dy = (chunk_pos.y - player_chunk.y).abs();
                let dz = (chunk_pos.z - player_chunk.z).abs();
                dx <= 2 && dy <= 2 && dz <= 2
            })
            .collect();
        Self::sort_chunks_nearest(&mut self.block_scan_state.chunk_list, player_chunk);
        self.block_scan_state.current_index = 0;
        self.block_scan_state.pending_leaves.clear();
        self.block_scan_state.pending_saplings.clear();
        self.block_scan_state.pending_sugar_cane.clear();
        self.block_scan_state.pending_wheat.clear();
        self.block_scan_state.pending_fire.clear();
    }

    fn scan_chunk_for_growth_targets(&mut self, chunk_pos: ChunkPos) {
        let Some(chunk) = self.chunks.get(&chunk_pos) else {
            return;
        };

        let base_x = chunk_pos.x * CHUNK_SIZE as i32;
        let base_y = chunk_pos.y * CHUNK_SIZE as i32;
        let base_z = chunk_pos.z * CHUNK_SIZE as i32;

        let mut leaves = Vec::new();
        let mut saplings = Vec::new();
        let mut sugar_cane = Vec::new();
        let mut wheat = Vec::new();
        let mut fire_blocks = Vec::new();

        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let local = LocalPos {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                    };
                    let world_pos = IVec3::new(
                        base_x + x as i32,
                        base_y + y as i32,
                        base_z + z as i32,
                    );
                    match chunk.get(local) {
                        BlockId::CANOPY_LEAVES => leaves.push(world_pos),
                        BlockId::SAPLING => saplings.push(world_pos),
                        block if is_fire_block(block) => fire_blocks.push(world_pos),
                        BlockId::SUGAR_CANE => {
                            if is_sugar_cane_base(world_pos, &self.chunks) {
                                sugar_cane.push(world_pos);
                            }
                        }
                        block if is_wheat_block(block) => wheat.push(world_pos),
                        _ => {}
                    }
                }
            }
        }

        self.block_scan_state.pending_leaves.extend(leaves);
        self.block_scan_state.pending_saplings.extend(saplings);
        self.block_scan_state.pending_sugar_cane.extend(sugar_cane);
        self.block_scan_state.pending_wheat.extend(wheat);
        self.block_scan_state.pending_fire.extend(fire_blocks);
    }

    fn process_block_scan_results(&mut self) {
        let leaf_positions = std::mem::take(&mut self.block_scan_state.pending_leaves);
        for leaf_pos in leaf_positions {
            if self.leaf_decay_timers.contains_key(&leaf_pos) {
                continue;
            }
            if self.leaf_has_log_support(leaf_pos) {
                continue;
            }
            let delay =
                self.gameplay_rand_range(LEAF_DECAY_MIN_DELAY_SECS, LEAF_DECAY_MAX_DELAY_SECS);
            self.leaf_decay_timers.insert(leaf_pos, delay);
        }

        let sapling_positions = std::mem::take(&mut self.block_scan_state.pending_saplings);
        for sapling_pos in sapling_positions {
            if self.sapling_growth_timers.contains_key(&sapling_pos) {
                continue;
            }
            let delay =
                self.gameplay_rand_range(SAPLING_GROWTH_MIN_SECS, SAPLING_GROWTH_MAX_SECS);
            self.sapling_growth_timers.insert(sapling_pos, delay);
        }

        let sugar_cane_positions = std::mem::take(&mut self.block_scan_state.pending_sugar_cane);
        for base_pos in sugar_cane_positions {
            if !is_sugar_cane_base(base_pos, &self.chunks) {
                continue;
            }
            if self.sugar_cane_growth_timers.contains_key(&base_pos) {
                continue;
            }
            let delay =
                self.gameplay_rand_range(SUGAR_CANE_GROWTH_MIN_SECS, SUGAR_CANE_GROWTH_MAX_SECS);
            self.sugar_cane_growth_timers.insert(base_pos, delay);
        }

        let wheat_positions = std::mem::take(&mut self.block_scan_state.pending_wheat);
        for wheat_pos in wheat_positions {
            let Some(wheat_block) = block_at(wheat_pos, &self.chunks) else {
                continue;
            };
            if !is_wheat_block(wheat_block) {
                continue;
            }

            let below = wheat_pos - IVec3::Y;
            if block_at(below, &self.chunks) != Some(BlockId::FARMLAND) {
                if let Some((chunk_pos, local_pos, previous, next)) =
                    try_set_world_block(&mut self.chunks, wheat_pos, BlockId::AIR)
                {
                    self.dirty_chunks.insert(chunk_pos);
                    self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
                    self.spawn_item_drop(wheat_pos, ItemStack::new(ItemId::WHEAT_SEEDS, 1));
                }
                continue;
            }

            let Some(stage) = wheat_growth_stage(wheat_block) else {
                continue;
            };
            if stage >= 7 {
                continue;
            }

            let grow_roll = simple_rng_next(&mut self.gameplay_rng_state) % 100;
            if grow_roll < 15 {
                let next_block = wheat_block_at_stage(stage + 1);
                if let Some((chunk_pos, local_pos, previous, next)) =
                    try_set_world_block(&mut self.chunks, wheat_pos, next_block)
                {
                    self.dirty_chunks.insert(chunk_pos);
                    self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
                }
            }
        }

        self.tick_fire_spread();
    }

    fn tick_incremental_block_scan(&mut self) {
        let player_chunk = self.player_chunk_pos();
        let player_moved_chunk = self
            .last_player_chunk
            .map_or(true, |previous| {
                previous.x != player_chunk.x || previous.z != player_chunk.z
            });

        if player_moved_chunk || self.block_scan_state.chunk_list.is_empty() {
            self.rebuild_block_scan_chunk_list();
        }

        let total_chunks = self.block_scan_state.chunk_list.len();
        if total_chunks == 0 {
            return;
        }

        let chunks_to_scan = self.block_scan_state.chunks_per_frame.max(1);
        for _ in 0..chunks_to_scan {
            if self.block_scan_state.current_index >= total_chunks {
                break;
            }
            let chunk_pos = self.block_scan_state.chunk_list[self.block_scan_state.current_index];
            self.block_scan_state.current_index += 1;
            self.scan_chunk_for_growth_targets(chunk_pos);
        }

        if self.block_scan_state.current_index >= total_chunks {
            self.process_block_scan_results();
            self.rebuild_block_scan_chunk_list();
        }
    }

    fn tick_leaf_decay_timers(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }

        let leaves = BlockId::CANOPY_LEAVES;
        let chunks = &self.chunks;
        let mut ready_to_decay = Vec::new();
        self.leaf_decay_timers.retain(|pos, remaining| {
            if block_at(*pos, chunks) != Some(leaves) {
                return false;
            }
            *remaining -= dt;
            if *remaining <= 0.0 {
                ready_to_decay.push(*pos);
                false
            } else {
                true
            }
        });

        for pos in ready_to_decay {
            if block_at(pos, &self.chunks) != Some(leaves) {
                continue;
            }
            if self.leaf_has_log_support(pos) {
                continue;
            }

            if let Some((chunk_pos, local_pos, previous, next)) =
                try_set_world_block(&mut self.chunks, pos, BlockId::AIR)
            {
                self.dirty_chunks.insert(chunk_pos);
                self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
                if self.gameplay_rand_f32() < LEAF_DECAY_SAPLING_DROP_CHANCE {
                    self.spawn_item_drop(pos, ItemStack::new(ItemId::from(BlockId::SAPLING), 1));
                }
            }
        }
    }

    fn leaf_has_log_support(&self, start: IVec3) -> bool {
        if block_at(start, &self.chunks) != Some(BlockId::CANOPY_LEAVES) {
            return false;
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        visited.insert(start);
        queue.push_back((start, 0i32));

        while let Some((pos, distance)) = queue.pop_front() {
            let Some(block) = block_at(pos, &self.chunks) else {
                continue;
            };
            if block == BlockId::TIMBER_LOG {
                return true;
            }
            if distance >= LEAF_SUPPORT_RADIUS || block != BlockId::CANOPY_LEAVES {
                continue;
            }

            for (dx, dy, dz) in [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                let next = pos + IVec3::new(dx, dy, dz);
                if !visited.insert(next) {
                    continue;
                }
                if let Some(next_block) = block_at(next, &self.chunks) {
                    if next_block == BlockId::CANOPY_LEAVES || next_block == BlockId::TIMBER_LOG {
                        queue.push_back((next, distance + 1));
                    }
                }
            }
        }

        false
    }

    fn tick_sapling_growth_timers(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }

        let chunks = &self.chunks;
        let mut ready_to_grow = Vec::new();
        self.sapling_growth_timers.retain(|pos, remaining| {
            if block_at(*pos, chunks) != Some(BlockId::SAPLING) {
                return false;
            }
            *remaining -= dt;
            if *remaining <= 0.0 {
                ready_to_grow.push(*pos);
                false
            } else {
                true
            }
        });

        for pos in ready_to_grow {
            if block_at(pos, &self.chunks) != Some(BlockId::SAPLING) {
                continue;
            }
            if !self.try_grow_sapling_tree(pos) {
                let delay =
                    self.gameplay_rand_range(SAPLING_GROWTH_MIN_SECS, SAPLING_GROWTH_MAX_SECS);
                self.sapling_growth_timers.insert(pos, delay);
            }
        }
    }

    fn try_grow_sapling_tree(&mut self, sapling_pos: IVec3) -> bool {
        if block_at(sapling_pos, &self.chunks) != Some(BlockId::SAPLING) {
            return false;
        }
        if !can_sapling_stay(sapling_pos, &self.chunks) {
            return false;
        }

        let trunk_height = 4 + (self.next_gameplay_rand_u32() % 3) as i32;
        let canopy_radius = 2;
        let canopy_center = sapling_pos + IVec3::new(0, trunk_height - 1, 0);
        let mut planned = HashMap::<IVec3, BlockId>::new();

        for dy in 0..trunk_height {
            let pos = sapling_pos + IVec3::new(0, dy, 0);
            let Some(existing) = block_at(pos, &self.chunks) else {
                return false;
            };
            if existing != BlockId::AIR && existing != BlockId::SAPLING && existing != BlockId::CANOPY_LEAVES {
                return false;
            }
            planned.insert(pos, BlockId::TIMBER_LOG);
        }

        for dy in -canopy_radius..=canopy_radius {
            for dz in -canopy_radius..=canopy_radius {
                for dx in -canopy_radius..=canopy_radius {
                    if dx * dx + dy * dy + dz * dz > canopy_radius * canopy_radius + 1 {
                        continue;
                    }
                    let pos = canopy_center + IVec3::new(dx, dy, dz);
                    if planned.get(&pos).is_some_and(|block| *block == BlockId::TIMBER_LOG) {
                        continue;
                    }
                    let Some(existing) = block_at(pos, &self.chunks) else {
                        return false;
                    };
                    if existing == BlockId::AIR || existing == BlockId::CANOPY_LEAVES {
                        planned.insert(pos, BlockId::CANOPY_LEAVES);
                    }
                }
            }
        }

        let mut updates = Vec::new();
        for (world_pos, block) in planned {
            if let Some((chunk_pos, local_pos, previous, next)) =
                try_set_world_block(&mut self.chunks, world_pos, block)
            {
                self.dirty_chunks.insert(chunk_pos);
                updates.push((chunk_pos, local_pos, previous, next));
            }
        }

        if updates.is_empty() {
            return false;
        }

        for (chunk_pos, local_pos, previous, next) in updates {
            self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
        }
        true
    }

    fn tick_sugar_cane_growth_timers(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }

        let chunks = &self.chunks;
        let mut ready_to_grow = Vec::new();
        self.sugar_cane_growth_timers.retain(|pos, remaining| {
            if !is_sugar_cane_base(*pos, chunks) {
                return false;
            }
            *remaining -= dt;
            if *remaining <= 0.0 {
                ready_to_grow.push(*pos);
                false
            } else {
                true
            }
        });

        for base_pos in ready_to_grow {
            if self.try_grow_sugar_cane(base_pos) {
                let delay = self
                    .gameplay_rand_range(SUGAR_CANE_GROWTH_MIN_SECS, SUGAR_CANE_GROWTH_MAX_SECS);
                self.sugar_cane_growth_timers.insert(base_pos, delay);
            } else if is_sugar_cane_base(base_pos, &self.chunks) {
                let delay = self
                    .gameplay_rand_range(SUGAR_CANE_GROWTH_MIN_SECS, SUGAR_CANE_GROWTH_MAX_SECS);
                self.sugar_cane_growth_timers.insert(base_pos, delay);
            }
        }
    }

    fn try_grow_sugar_cane(&mut self, base_pos: IVec3) -> bool {
        if !is_sugar_cane_base(base_pos, &self.chunks) {
            return false;
        }
        if !has_horizontal_water_neighbor(base_pos, &self.chunks) {
            return false;
        }

        let height = sugar_cane_height(base_pos, &self.chunks);
        if height >= SUGAR_CANE_MAX_HEIGHT {
            return false;
        }

        let grow_pos = base_pos + IVec3::new(0, height, 0);
        if block_at(grow_pos, &self.chunks) != Some(BlockId::AIR) {
            return false;
        }

        if let Some((chunk_pos, local_pos, previous, next)) =
            try_set_world_block(&mut self.chunks, grow_pos, BlockId::SUGAR_CANE)
        {
            self.dirty_chunks.insert(chunk_pos);
            self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
            return true;
        }

        false
    }

    fn save_world(&mut self) {
        if !matches!(self.game_mode, Some(GameMode::Singleplayer)) {
            return;
        }
        let Some(world_dir) = self.active_world_dir.clone() else {
            return;
        };
        self.persist_open_double_chest();
        // Stage dirty chunks to memory only (no disk flush)
        if let Some(ref mut persistence) = self.persistence {
            for pos in self.dirty_chunks.drain() {
                if let Some(chunk) = self.chunks.get(&pos) {
                    let _ = persistence.stage_chunk(pos, chunk);
                }
            }
        }
        if let Some(ref persistence) = self.persistence {
            if let Err(e) = persistence.save_chest_inventories(&self.chest_inventories) {
                warn!("Failed to save chest inventories: {e}");
            }
            if let Err(e) = persistence.save_inventory(&self.inventory) {
                warn!("Failed to save player inventory: {e}");
            }
        }
        // Save lightweight metadata to disk
        let meta = WorldMeta {
            world_name: self.active_world_name.clone(),
            world_seed: self.world_seed,
            player_position: self.player_pos.to_array(),
            player_yaw: self.camera.yaw,
            player_pitch: self.camera.pitch,
            time_of_day: self.time_of_day,
            play_mode: self.play_mode.into(),
        };
        if let Err(e) = meta.save(&world_dir) {
            warn!("Failed to save world meta: {e}");
        }
    }

    /// Full save on exit — flush all staged chunks to disk
    fn save_world_full(&mut self) {
        self.save_world();
        if let Some(ref mut persistence) = self.persistence {
            if let Err(e) = persistence.flush_all() {
                warn!("Failed to flush regions: {e}");
            }
        }
        info!("World saved (full flush)");
    }

    fn process_multiplayer(&mut self) {
        // Collect messages first to avoid double borrow
        let (reliable_msgs, unreliable_msgs, net_connected) = {
            let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode else {
                return;
            };
            net.update(std::time::Duration::from_millis(16));
            let reliable = net.receive_reliable();
            let unreliable = net.receive_unreliable();
            (reliable, unreliable, net.is_connected())
        };

        if !net_connected {
            warn!("Multiplayer connection lost; returning to main menu");
            self.return_to_menu();
            return;
        };

        // Process reliable messages
        for msg in reliable_msgs {
            match msg {
                S2C::HandshakeAccept { player_id, spawn_position, .. } => {
                    self.player_pos = spawn_position;
                    self.velocity = Vec3::ZERO;
                    self.my_player_id = Some(player_id);
                    self.last_player_chunk = None; // Force chunk re-evaluation
                    self.chunks_ready = false;
                    info!("Received HandshakeAccept: player={player_id}, spawn={spawn_position}");
                }
                S2C::HandshakeReject { reason } => {
                    warn!("Server rejected handshake: {reason}");
                    self.return_to_menu();
                    return;
                }
                S2C::ChunkData { pos, data, .. } => {
                    match protocol::decode::<ChunkData>(&data) {
                        Ok(chunk) => {
                            info!("Received ChunkData {pos:?} ({} bytes)", data.len());
                            self.pending_chunks.remove(&pos);
                            self.chunks.insert(pos, chunk);
                            if self.mesh_queue_set.insert(pos) {
                                self.mesh_queue.push_back(pos);
                            }
                        }
                        Err(e) => warn!("Failed to decode chunk {pos:?}: {e}"),
                    }
                }
                S2C::ChunkUnload { pos } => {
                    self.unload_chunk(pos);
                }
                S2C::ChunkDelta { pos, changes } => {
                    if self.apply_chunk_delta(pos, &changes) {
                        info!("Applied ChunkDelta {pos:?} ({} change(s))", changes.len());
                    }
                }
                S2C::BlockEditConfirm { world_pos, block } => {
                    info!("Block edit confirmed at {world_pos:?} -> block {}", block.0);
                    let (chunk_pos, local_pos) = world_to_chunk(world_pos);
                    let mut remesh_update: Option<(BlockId, BlockId)> = None;
                    if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                        let idx = local_to_index(local_pos);
                        let previous = chunk.blocks[idx];
                        chunk.blocks[idx] = block;
                        remesh_update = Some((previous, block));
                        self.dirty_chunks.insert(chunk_pos);
                    }
                    if let Some((previous, next)) = remesh_update {
                        if previous == BlockId::CHEST && next != BlockId::CHEST {
                            let _ = self.take_chest_inventory_contents(world_pos);
                        }
                        self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
                    }
                }
                S2C::PlayerJoined { player_id, username, position } => {
                    info!("Player {username} ({player_id}) joined at {position}");
                }
                S2C::PlayerLeft { player_id } => {
                    self.remote_players.retain(|p| p.player_id != player_id);
                    self.remote_player_states.remove(&player_id);
                    info!("Player {player_id} left");
                }
                S2C::Chat {
                    sender_name,
                    message,
                    ..
                } => {
                    self.push_chat_message(&sender_name, &message);
                }
                S2C::TimeSync { time_of_day, .. } => {
                    self.time_of_day = time_of_day.rem_euclid(1.0);
                }
                _ => {}
            }
        }

        // Process unreliable messages (player states)
        for msg in unreliable_msgs {
            if let S2C::PlayerStates { states, .. } = msg {
                debug!("Received player states: {}", states.len());
                let mut active_remote_ids = HashSet::new();
                for snap in states {
                    if Some(snap.player_id) == self.my_player_id {
                        continue;
                    }
                    active_remote_ids.insert(snap.player_id);
                    let entry = self
                        .remote_player_states
                        .entry(snap.player_id)
                        .or_insert_with(|| RemotePlayerSyncState {
                            previous_position: snap.position,
                            target_position: snap.position,
                            display_position: snap.position,
                            interpolation_time: REMOTE_INTERPOLATION_DURATION,
                            interpolation_duration: REMOTE_INTERPOLATION_DURATION,
                            yaw: snap.yaw,
                            pitch: snap.pitch,
                            flags: snap.flags,
                            attack_animation: snap.attack_animation,
                            breaking_block: snap.breaking_block,
                            break_progress: snap.break_progress,
                            animation_phase: 0.0,
                        });
                    entry.previous_position = entry.display_position;
                    entry.target_position = snap.position;
                    entry.interpolation_time = 0.0;
                    entry.interpolation_duration = REMOTE_INTERPOLATION_DURATION;
                    entry.yaw = snap.yaw;
                    entry.pitch = snap.pitch;
                    entry.flags = snap.flags;
                    entry.attack_animation = snap.attack_animation;
                    entry.breaking_block = snap.breaking_block;
                    entry.break_progress = snap.break_progress;
                }
                self.remote_player_states
                    .retain(|player_id, _| active_remote_ids.contains(player_id));
            }
        }

        // Send player input
        if self.my_player_id.is_some() {
            let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode else {
                return;
            };
            let mut flags = PlayerInputFlags::empty();
            if self.sprinting {
                flags |= PlayerInputFlags::SPRINTING;
            }
            if !self.fly_mode
                && (self.input.is_pressed(KeyCode::ShiftLeft)
                    || self.input.is_pressed(KeyCode::ShiftRight))
            {
                flags |= PlayerInputFlags::SNEAKING;
            }
            if self.input.is_pressed(KeyCode::Space) {
                flags |= PlayerInputFlags::JUMPING;
            }
            let input_msg = C2S::PlayerInput {
                tick: 0,
                position: self.player_pos,
                yaw: self.camera.yaw,
                pitch: self.camera.pitch,
                flags: flags.bits(),
                attack_animation: self.attack_animation,
                breaking_block: self.breaking_block,
                break_progress: self.break_progress,
            };
            net.send_unreliable(&input_msg);
        }
    }

    fn update_remote_players(&mut self, dt: f32) {
        self.remote_players.clear();
        self.remote_break_overlays.clear();

        let mut player_ids: Vec<u64> = self.remote_player_states.keys().copied().collect();
        player_ids.sort_unstable();

        for player_id in player_ids {
            let Some(state) = self.remote_player_states.get_mut(&player_id) else {
                continue;
            };

            let interpolation_duration = state.interpolation_duration.max(0.0001);
            state.interpolation_time = (state.interpolation_time + dt).min(interpolation_duration);
            let t = (state.interpolation_time / interpolation_duration).clamp(0.0, 1.0);

            let previous_display_position = state.display_position;
            state.display_position = state.previous_position.lerp(state.target_position, t);

            let delta = state.display_position - previous_display_position;
            let horizontal_distance = Vec2::new(delta.x, delta.z).length();
            if horizontal_distance > 0.0001 {
                state.animation_phase += horizontal_distance * REMOTE_WALK_CYCLE_SPEED;
            } else {
                state.animation_phase = 0.0;
            }

            self.remote_players.push(RemotePlayer {
                player_id,
                position: state.display_position,
                yaw: state.yaw,
                pitch: state.pitch,
                animation_phase: state.animation_phase,
                attack_animation: state.attack_animation,
                is_crouching: (state.flags & PlayerInputFlags::SNEAKING.bits()) != 0,
            });

            if let Some(block_pos) = state.breaking_block {
                let progress = state.break_progress.clamp(0.0, 1.0);
                if progress > 0.0 {
                    self.remote_break_overlays.push((block_pos, progress));
                }
            }
        }
    }

    fn player_chunk_pos(&self) -> ChunkPos {
        world_to_chunk(self.player_pos.floor().as_ivec3()).0
    }

    fn chunk_neighbors(chunk_pos: ChunkPos) -> [ChunkPos; 6] {
        [
            ChunkPos {
                x: chunk_pos.x + 1,
                y: chunk_pos.y,
                z: chunk_pos.z,
            },
            ChunkPos {
                x: chunk_pos.x - 1,
                y: chunk_pos.y,
                z: chunk_pos.z,
            },
            ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y + 1,
                z: chunk_pos.z,
            },
            ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y - 1,
                z: chunk_pos.z,
            },
            ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y,
                z: chunk_pos.z + 1,
            },
            ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y,
                z: chunk_pos.z - 1,
            },
        ]
    }

    fn unload_chunk(&mut self, pos: ChunkPos) {
        if self.chunks.remove(&pos).is_none() {
            self.pending_chunks.remove(&pos);
            self.mesh_jobs_in_flight.remove(&pos);
            return;
        }

        self.pending_chunks.remove(&pos);
        self.dirty_chunks.remove(&pos);
        self.chunk_lods.remove(&pos);
        self.mesh_versions.remove(&pos);
        self.mesh_queue.retain(|queued| *queued != pos);
        self.mesh_queue_set.remove(&pos);
        self.mesh_jobs_in_flight.remove(&pos);

        if let Some(renderer) = self.renderer.as_mut() {
            renderer.remove_chunk_mesh(pos);
        }

        for neighbor in Self::chunk_neighbors(pos) {
            if self.chunks.contains_key(&neighbor) && self.mesh_queue_set.insert(neighbor) {
                self.mesh_queue.push_back(neighbor);
            }
        }

        info!("Unloaded chunk {pos:?}");
    }

    fn apply_chunk_delta(&mut self, pos: ChunkPos, changes: &[(LocalPos, BlockId)]) -> bool {
        if changes.is_empty() {
            return false;
        }

        let mut remesh_targets = HashSet::from([pos]);
        let mut force_neighbor_light_update = false;
        let mut chests_to_clear: Vec<IVec3> = Vec::new();
        let registry = self.registry.clone();
        {
            let Some(chunk) = self.chunks.get_mut(&pos) else {
                warn!("Ignoring ChunkDelta for missing chunk {pos:?}");
                return false;
            };

            for &(local_pos, block) in changes {
                let previous = chunk.get(local_pos);
                chunk.set(local_pos, block);
                if previous == BlockId::CHEST && block != BlockId::CHEST {
                    chests_to_clear.push(chunk_to_world(pos, local_pos));
                }
                if let Some(registry) = registry.as_ref() {
                    let previous_light = registry.get_properties(previous).light_level;
                    let next_light = registry.get_properties(block).light_level;
                    if previous_light > 0 || next_light > 0 {
                        force_neighbor_light_update = true;
                    }
                }

                if local_pos.x == 0 {
                    remesh_targets.insert(ChunkPos {
                        x: pos.x - 1,
                        y: pos.y,
                        z: pos.z,
                    });
                } else if local_pos.x == (CHUNK_SIZE - 1) as u8 {
                    remesh_targets.insert(ChunkPos {
                        x: pos.x + 1,
                        y: pos.y,
                        z: pos.z,
                    });
                }

                if local_pos.y == 0 {
                    remesh_targets.insert(ChunkPos {
                        x: pos.x,
                        y: pos.y - 1,
                        z: pos.z,
                    });
                } else if local_pos.y == (CHUNK_SIZE - 1) as u8 {
                    remesh_targets.insert(ChunkPos {
                        x: pos.x,
                        y: pos.y + 1,
                        z: pos.z,
                    });
                }

                if local_pos.z == 0 {
                    remesh_targets.insert(ChunkPos {
                        x: pos.x,
                        y: pos.y,
                        z: pos.z - 1,
                    });
                } else if local_pos.z == (CHUNK_SIZE - 1) as u8 {
                    remesh_targets.insert(ChunkPos {
                        x: pos.x,
                        y: pos.y,
                        z: pos.z + 1,
                    });
                }
            }
        }
        for chest_pos in chests_to_clear {
            let _ = self.take_chest_inventory_contents(chest_pos);
        }

        if force_neighbor_light_update {
            remesh_targets.extend(Self::chunk_neighbors(pos));
        }

        self.dirty_chunks.insert(pos);
        let mut remesh_targets: Vec<ChunkPos> = remesh_targets.into_iter().collect();
        remesh_targets.sort_by_key(|pos| (pos.x, pos.y, pos.z));
        for chunk_pos in remesh_targets {
            self.remesh_chunk(chunk_pos);
        }

        true
    }

    fn gather_stream_targets(&mut self, center: ChunkPos) -> Vec<ChunkPos> {
        let mut desired: Vec<ChunkPos> = Vec::new();
        let render_distance = self.settings.render_distance;
        let vertical_below = if self.fly_mode {
            let candidate_floor = center.y - self.settings.stream_flight_below;
            let floor = self
                .flight_stream_floor_y
                .map_or(candidate_floor, |existing| existing.min(candidate_floor));
            self.flight_stream_floor_y = Some(floor);
            floor
        } else {
            self.flight_stream_floor_y = None;
            center.y - self.settings.stream_surface_below
        };
        let vertical_above = center.y + self.settings.stream_above;
        for y in vertical_below..=vertical_above {
            for dz in -render_distance..=render_distance {
                for dx in -render_distance..=render_distance {
                    desired.push(ChunkPos {
                        x: center.x + dx,
                        y,
                        z: center.z + dz,
                    });
                }
            }
        }
        desired
    }

    fn sort_chunks_nearest(chunks: &mut [ChunkPos], center: ChunkPos) {
        chunks.sort_by_key(|pos| {
            let dx = i64::from(pos.x - center.x);
            let dz = i64::from(pos.z - center.z);
            let dy = i64::from((pos.y - center.y).abs());
            let horizontal_distance_sq = dx * dx + dz * dz;
            (horizontal_distance_sq, dy, pos.y, pos.z, pos.x)
        });
    }

    fn stream_chunks_multiplayer(&mut self) {
        // Wait for HandshakeAccept before requesting chunks
        if self.my_player_id.is_none() {
            return;
        }
        let player_chunk = self.player_chunk_pos();
        let player_moved = self.last_player_chunk.map_or(true, |last| last.x != player_chunk.x || last.z != player_chunk.z);
        self.last_player_chunk = Some(player_chunk);

        if player_moved {
            self.check_lod_transitions(player_chunk);
        }

        if self.pending_chunks.len() >= MULTIPLAYER_MAX_IN_FLIGHT_CHUNKS {
            return;
        }

        let mut missing: Vec<ChunkPos> = self
            .gather_stream_targets(player_chunk)
            .into_iter()
            .filter(|pos| !self.chunks.contains_key(pos) && !self.pending_chunks.contains(pos))
            .collect();

        if missing.is_empty() {
            return;
        }

        Self::sort_chunks_nearest(&mut missing, player_chunk);

        if let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode {
            let slots_left = MULTIPLAYER_MAX_IN_FLIGHT_CHUNKS - self.pending_chunks.len();
            let batch_size = MULTIPLAYER_CHUNK_BATCH_SIZE.min(slots_left);
            let batch: Vec<ChunkPos> = missing.into_iter().take(batch_size).collect();
            if batch.is_empty() {
                return;
            }
            for &pos in &batch {
                self.pending_chunks.insert(pos);
            }
            let preview: Vec<String> = batch
                .iter()
                .take(3)
                .map(|pos| format!("{pos:?}"))
                .collect();
            info!(
                "Requesting {} chunk(s) near {:?} (pending={} first3=[{}])",
                batch.len(),
                player_chunk,
                self.pending_chunks.len(),
                preview.join(", ")
            );
            net.send_reliable(&C2S::RequestChunks { positions: batch });
        }
    }

    fn stream_chunks(&mut self) {
        let player_chunk = self.player_chunk_pos();
        let chunk_x = player_chunk.x;
        let chunk_z = player_chunk.z;

        if let Some(last) = self.last_player_chunk {
            if last.x == chunk_x && last.z == chunk_z {
                return;
            }
        }
        self.last_player_chunk = Some(player_chunk);

        let mut desired = self.gather_stream_targets(player_chunk);
        Self::sort_chunks_nearest(&mut desired, player_chunk);
        let desired_set: HashSet<ChunkPos> = desired.iter().copied().collect();

        let to_remove: Vec<ChunkPos> = self
            .chunks
            .keys()
            .filter(|p| !desired_set.contains(p))
            .copied()
            .collect();

        // Send a capped, distance-prioritized generation batch each frame.
        let Some(tx) = self.chunk_request_tx.as_ref() else { return; };
        let mut missing: Vec<ChunkPos> = desired
            .into_iter()
            .filter(|pos| !self.chunks.contains_key(pos) && !self.pending_chunks.contains(pos))
            .collect();
        Self::sort_chunks_nearest(&mut missing, player_chunk);
        for pos in missing
            .into_iter()
            .take(SINGLEPLAYER_CHUNK_REQUEST_BATCH_SIZE)
        {
            let _ = tx.send(pos);
            self.pending_chunks.insert(pos);
        }

        for &pos in &to_remove {
            self.chunks.remove(&pos);
            self.pending_chunks.remove(&pos);
            self.chunk_lods.remove(&pos);
            self.mesh_versions.remove(&pos);
            self.mesh_jobs_in_flight.remove(&pos);
        }
        if let Some(renderer) = self.renderer.as_mut() {
            for &pos in &to_remove {
                renderer.remove_chunk_mesh(pos);
            }
        }
        if !to_remove.is_empty() {
            let removed: HashSet<ChunkPos> = to_remove.iter().copied().collect();
            self.pending_mesh_uploads
                .retain(|pending| !removed.contains(&pending.chunk_pos));
        }

        let removed_count = to_remove.len();

        // Also remesh existing neighbors of removed chunks
        let mut to_remesh = Vec::new();
        for &pos in &to_remove {
            for &(dx, dy, dz) in &[
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                let neighbor = ChunkPos {
                    x: pos.x + dx,
                    y: pos.y + dy,
                    z: pos.z + dz,
                };
                if self.chunks.contains_key(&neighbor) {
                    to_remesh.push(neighbor);
                }
            }
        }
        to_remesh.sort_by_key(|p| (p.x, p.y, p.z));
        to_remesh.dedup();

        // Queue meshes instead of processing all at once
        for pos in to_remesh {
            if self.mesh_queue_set.insert(pos) {
                self.mesh_queue.push_back(pos);
            }
        }

        if removed_count > 0 {
            info!(
                "Streamed chunks: {} removed, {} total",
                removed_count,
                self.chunks.len()
            );
        }

        // Check for LOD transitions — re-queue chunks whose LOD changed
        self.check_lod_transitions(player_chunk);
    }

    fn check_lod_transitions(&mut self, player_chunk: ChunkPos) {
        // Only check chunks near the LOD boundary (threshold ± 1)
        // since the player moves 1 chunk at a time
        let lo = self.settings.lod1_distance;
        let hi = lo + 1;
        let vertical_below = if self.fly_mode {
            self.settings.stream_flight_below
        } else {
            self.settings.stream_surface_below
        };
        let vertical_above = self.settings.stream_above;
        for y_off in -vertical_below..=vertical_above {
            for dz in -hi..=hi {
                for dx in -hi..=hi {
                    let dist = dx.abs().max(dz.abs());
                    if dist < lo || dist > hi {
                        continue;
                    }
                    let chunk_pos = ChunkPos {
                        x: player_chunk.x + dx,
                        y: player_chunk.y + y_off,
                        z: player_chunk.z + dz,
                    };
                    if !self.chunks.contains_key(&chunk_pos) {
                        continue;
                    }
                    let new_lod: u8 = if dist > lo { 1 } else { 0 };
                    let old_lod = self.chunk_lods.get(&chunk_pos).copied().unwrap_or(0);
                    if new_lod != old_lod {
                        if self.mesh_queue_set.insert(chunk_pos) {
                            self.mesh_queue.push_back(chunk_pos);
                        }
                    }
                }
            }
        }
    }

    fn remesh_chunk(&mut self, chunk_pos: ChunkPos) {
        if self.mesh_jobs_in_flight.contains(&chunk_pos) {
            if self.mesh_queue_set.insert(chunk_pos) {
                self.mesh_queue.push_back(chunk_pos);
            }
            return;
        }

        let Some(chunk) = self.chunks.get(&chunk_pos) else {
            return;
        };
        let Some(registry) = self.registry.as_ref() else {
            return;
        };
        let Some(mesh_worker) = self.mesh_worker.as_ref() else {
            return;
        };

        let neighbors = [
            self.chunks
                .get(&ChunkPos {
                    x: chunk_pos.x + 1,
                    y: chunk_pos.y,
                    z: chunk_pos.z,
                })
                .cloned(),
            self.chunks
                .get(&ChunkPos {
                    x: chunk_pos.x - 1,
                    y: chunk_pos.y,
                    z: chunk_pos.z,
                })
                .cloned(),
            self.chunks
                .get(&ChunkPos {
                    x: chunk_pos.x,
                    y: chunk_pos.y + 1,
                    z: chunk_pos.z,
                })
                .cloned(),
            self.chunks
                .get(&ChunkPos {
                    x: chunk_pos.x,
                    y: chunk_pos.y - 1,
                    z: chunk_pos.z,
                })
                .cloned(),
            self.chunks
                .get(&ChunkPos {
                    x: chunk_pos.x,
                    y: chunk_pos.y,
                    z: chunk_pos.z + 1,
                })
                .cloned(),
            self.chunks
                .get(&ChunkPos {
                    x: chunk_pos.x,
                    y: chunk_pos.y,
                    z: chunk_pos.z - 1,
                })
                .cloned(),
        ];

        let version = self.mesh_versions.entry(chunk_pos).or_insert(0);
        *version += 1;
        let current_version = *version;

        let player_chunk = self.player_chunk_pos();
        let dx = (chunk_pos.x - player_chunk.x).abs();
        let dz = (chunk_pos.z - player_chunk.z).abs();
        let horizontal_dist = dx.max(dz);
        let lod_level: u8 = if horizontal_dist > self.settings.lod1_distance {
            1
        } else {
            0
        };
        self.chunk_lods.insert(chunk_pos, lod_level);

        let request = MeshRequest {
            chunk_pos,
            chunk: chunk.clone(),
            neighbors,
            registry: registry.clone(),
            world_seed: self.world_seed,
            version: current_version,
            lod_level,
        };
        self.mesh_jobs_in_flight.insert(chunk_pos);
        mesh_worker.submit(request);
    }

    fn poll_mesh_results(&mut self) {
        let completed = match self.mesh_worker.as_ref() {
            Some(mesh_worker) => mesh_worker.poll(),
            None => return,
        };

        for (chunk_pos, meshes, version) in completed {
            self.mesh_jobs_in_flight.remove(&chunk_pos);
            let current_version = self.mesh_versions.get(&chunk_pos).copied().unwrap_or(0);
            if version < current_version {
                continue;
            }
            self.pending_mesh_uploads.push_back(PendingMeshUpload {
                chunk_pos,
                meshes,
                version,
            });
        }
    }

    fn remesh_for_block_change(
        &mut self,
        chunk_pos: ChunkPos,
        local_pos: LocalPos,
        previous_block: BlockId,
        next_block: BlockId,
    ) {
        let force_neighbor_light_update = self.registry.as_ref().is_some_and(|registry| {
            let previous_light = registry.get_properties(previous_block).light_level;
            let next_light = registry.get_properties(next_block).light_level;
            previous_light > 0 || next_light > 0
        });
        self.remesh_chunk_and_neighbors(chunk_pos, local_pos, force_neighbor_light_update);
    }

    fn trigger_remesh(&mut self, chunk_pos: ChunkPos) {
        self.remesh_chunk(chunk_pos);
        for neighbor in Self::chunk_neighbors(chunk_pos) {
            self.remesh_chunk(neighbor);
        }
    }

    fn remesh_chunk_and_neighbors(
        &mut self,
        chunk_pos: ChunkPos,
        local_pos: LocalPos,
        force_all_neighbors: bool,
    ) {
        // Always remesh the chunk containing the modified block
        self.remesh_chunk(chunk_pos);

        if force_all_neighbors {
            for neighbor in Self::chunk_neighbors(chunk_pos) {
                self.remesh_chunk(neighbor);
            }
            return;
        }

        // Remesh neighboring chunks if block is on chunk boundary
        if local_pos.x == 0 {
            self.remesh_chunk(ChunkPos {
                x: chunk_pos.x - 1,
                y: chunk_pos.y,
                z: chunk_pos.z,
            });
        } else if local_pos.x == (CHUNK_SIZE - 1) as u8 {
            self.remesh_chunk(ChunkPos {
                x: chunk_pos.x + 1,
                y: chunk_pos.y,
                z: chunk_pos.z,
            });
        }

        if local_pos.y == 0 {
            self.remesh_chunk(ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y - 1,
                z: chunk_pos.z,
            });
        } else if local_pos.y == (CHUNK_SIZE - 1) as u8 {
            self.remesh_chunk(ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y + 1,
                z: chunk_pos.z,
            });
        }

        if local_pos.z == 0 {
            self.remesh_chunk(ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y,
                z: chunk_pos.z - 1,
            });
        } else if local_pos.z == (CHUNK_SIZE - 1) as u8 {
            self.remesh_chunk(ChunkPos {
                x: chunk_pos.x,
                y: chunk_pos.y,
                z: chunk_pos.z + 1,
            });
        }
    }

    fn process_mesh_queue(&mut self) {
        let budget = self.mesh_budget_per_frame();
        let count = self.mesh_queue.len().min(budget);
        if count == 0 {
            return;
        }

        let player_chunk = self.player_chunk_pos();
        for _ in 0..count {
            let best_index = self
                .mesh_queue
                .iter()
                .enumerate()
                .min_by_key(|(_, pos)| {
                    let dx = (pos.x - player_chunk.x).abs();
                    let dz = (pos.z - player_chunk.z).abs();
                    let dy = (pos.y - player_chunk.y).abs();
                    (dx.max(dz), dy)
                })
                .map(|(index, _)| index);

            let Some(index) = best_index else {
                break;
            };
            let Some(pos) = self.mesh_queue.remove(index) else {
                continue;
            };
            self.mesh_queue_set.remove(&pos);
            self.remesh_chunk(pos);
        }
    }

    fn mesh_budget_per_frame(&self) -> usize {
        // Startup recovery mode: when no world chunks are rendered yet, build meshes aggressively.
        if self.last_render_stats.rendered_chunks == 0 {
            return 96;
        }
        if self.mesh_queue.len() > 2048 {
            return 48;
        }
        if self.mesh_queue.len() > 1024 {
            return 24;
        }
        let fps = self.fps;
        let budget = if fps <= 1.0 || fps >= 90.0 {
            MAX_CHUNKS_PER_FRAME
        } else if fps >= 72.0 {
            3
        } else if fps >= 55.0 {
            2
        } else {
            MIN_CHUNKS_PER_FRAME
        };
        budget.clamp(MIN_CHUNKS_PER_FRAME, MAX_CHUNKS_PER_FRAME)
    }

    fn process_mesh_upload_queue(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            self.last_upload_stats = UploadFrameStats::default();
            return;
        };
        if self.pending_mesh_uploads.is_empty() {
            self.last_upload_stats = UploadFrameStats::default();
            return;
        }

        let mut budget = mesh_upload_budget_for_fps(self.fps);
        if self.last_render_stats.rendered_chunks == 0 {
            budget.max_bytes = budget.max_bytes.max(64 * 1024 * 1024);
            budget.max_chunks = budget.max_chunks.max(256);
        }
        let mut frame_stats = UploadFrameStats::default();

        while frame_stats.uploaded_chunks < budget.max_chunks as u32 {
            let Some(front) = self.pending_mesh_uploads.front() else {
                break;
            };
            let estimated_bytes = mesh_upload_size_bytes(&front.meshes);
            if !should_upload_next_chunk(
                frame_stats.uploaded_chunks,
                frame_stats.uploaded_bytes,
                estimated_bytes,
                budget,
            ) {
                break;
            }

            let pending = self
                .pending_mesh_uploads
                .pop_front()
                .expect("pending mesh upload queue front must exist");
            if !self.chunks.contains_key(&pending.chunk_pos) {
                continue;
            }
            let current_version = self
                .mesh_versions
                .get(&pending.chunk_pos)
                .copied()
                .unwrap_or(0);
            if pending.version < current_version {
                continue;
            }

            let upload_stats = renderer.replace_chunk_mesh(
                &pending.meshes,
                pending.chunk_pos,
                self.render_time_seconds,
            );
            frame_stats.uploaded_bytes += upload_stats.uploaded_bytes;
            frame_stats.uploaded_chunks += 1;
            frame_stats.buffer_reallocations += upload_stats.buffer_reallocations;
        }

        self.last_upload_stats = frame_stats;
    }

    fn poll_generated_chunks(&mut self) {
        let Some(rx) = self.chunk_result_rx.as_ref() else { return; };

        // Process up to N chunks per frame to avoid blocking
        let limit = if self.last_render_stats.rendered_chunks == 0 {
            128
        } else {
            16
        };
        for _ in 0..limit {
            match rx.try_recv() {
                Ok((pos, chunk)) => {
                    self.pending_chunks.remove(&pos);
                    self.chunks.insert(pos, chunk);
                    // Queue for meshing
                    if self.mesh_queue_set.insert(pos) {
                        self.mesh_queue.push_back(pos);
                    }
                    // Also queue neighbors for remeshing
                    for &(dx, dy, dz) in &[
                        (1, 0, 0),
                        (-1, 0, 0),
                        (0, 1, 0),
                        (0, -1, 0),
                        (0, 0, 1),
                        (0, 0, -1),
                    ] {
                        let neighbor = ChunkPos {
                            x: pos.x + dx,
                            y: pos.y + dy,
                            z: pos.z + dz,
                        };
                        if self.chunks.contains_key(&neighbor)
                            && self.mesh_queue_set.insert(neighbor)
                        {
                            self.mesh_queue.push_back(neighbor);
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    fn update_and_render(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        let size = window.inner_size();

        if size.width == 0 || size.height == 0 {
            return;
        }

        self.camera.aspect = size.width as f32 / size.height as f32;

        let now = Instant::now();
        let dt = self
            .last_frame
            .map(|last| (now - last).as_secs_f32())
            .unwrap_or(1.0 / 60.0);
        self.last_frame = Some(now);
        let dt = dt.min(0.05);
        self.render_time_seconds += dt;
        self.record_frame_time_sample(dt * 1000.0);
        self.perf_log_timer_seconds += dt;

        // Main Menu and World Select states
        if !matches!(self.app_state, AppState::InGame) {
            let Some(renderer) = self.renderer.as_mut() else {
                return;
            };
            match self.app_state {
                AppState::MainMenu => {
                    let show_ip = self.menu_selected == 1;
                    renderer.update_main_menu(self.menu_selected, show_ip, &self.server_ip);
                }
                AppState::WorldSelect => {
                    let world_views: Vec<WorldListEntryView<'_>> = self
                        .world_entries
                        .iter()
                        .map(|world| WorldListEntryView {
                            name: &world.display_name,
                            seed: world.world_seed,
                            size_label: &world.size_label,
                            last_opened_label: &world.last_opened_label,
                        })
                        .collect();
                    let view = WorldSelectView {
                        worlds: &world_views,
                        selected_world: self.world_selected,
                        create_form_open: self.world_create_form_open,
                        create_name_input: &self.world_create_name_input,
                        create_seed_input: &self.world_create_seed_input,
                        create_play_mode_label: match self.world_create_play_mode {
                            PlayMode::Survival => "SURVIVAL",
                            PlayMode::Creative => "CREATIVE",
                        },
                        create_play_mode_is_creative: matches!(
                            self.world_create_play_mode,
                            PlayMode::Creative
                        ),
                        active_input_field: self.world_create_active_field,
                        delete_confirmation_open: self.world_delete_confirmation_open,
                    };
                    renderer.update_world_select_menu(&view);
                }
                AppState::InGame => {}
            }
            renderer.update_sky(&self.camera, 0.25, 0.0); // Night sky for menu
            match renderer.render_main_menu_frame() {
                Ok(()) => {}
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                    renderer.resize(size.width, size.height);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    event_loop.exit();
                }
                Err(_) => {}
            }
            window.set_title("Veldspar");
            self.input.clear_frame();
            return;
        }

        // InGame state
        // Always process chunk loading regardless of chunks_ready
        if matches!(self.game_mode, Some(GameMode::Singleplayer)) {
            self.tick_incremental_block_scan();
        }
        // Chunk streaming based on game mode
        match &self.game_mode {
            Some(GameMode::Multiplayer { .. }) => {
                self.process_multiplayer();
                self.stream_chunks_multiplayer();
            }
            _ => {
                self.stream_chunks();
                self.poll_generated_chunks();
            }
        }
        self.process_mesh_queue();
        self.poll_mesh_results();
        self.process_mesh_upload_queue();
        self.update_remote_players(dt);

        // Find spawn point FIRST so chunks_ready checks the right position
        if !self.spawn_found && !self.chunks.is_empty() && matches!(self.game_mode, Some(GameMode::Singleplayer)) {
            if let Some(registry) = self.registry.as_ref() {
                for y in (0..80).rev() {
                    if is_block_solid(0, y, 0, &self.chunks, registry) {
                        self.player_pos.y = (y + 2) as f32;
                        self.spawn_found = true;
                        self.spawn_position = self.player_pos;
                        self.velocity = Vec3::ZERO;
                        break;
                    }
                }
            }
        }
        // In multiplayer, spawn is set by HandshakeAccept
        if !self.spawn_found && matches!(self.game_mode, Some(GameMode::Multiplayer { .. })) {
            if self.my_player_id.is_some() {
                self.spawn_found = true;
            }
        }

        // Check if chunks around player are ready (at actual spawn position)
        if !self.chunks_ready {
            let player_chunk = self.player_chunk_pos();
            let layers: &[i32] = if matches!(self.game_mode, Some(GameMode::Multiplayer { .. })) {
                &[player_chunk.y, player_chunk.y - 1]
            } else {
                &[player_chunk.y]
            };
            let mut needed: Vec<ChunkPos> = Vec::with_capacity(layers.len() * 5);
            for &y in layers {
                needed.push(ChunkPos {
                    x: player_chunk.x,
                    y,
                    z: player_chunk.z,
                });
                needed.push(ChunkPos {
                    x: player_chunk.x + 1,
                    y,
                    z: player_chunk.z,
                });
                needed.push(ChunkPos {
                    x: player_chunk.x - 1,
                    y,
                    z: player_chunk.z,
                });
                needed.push(ChunkPos {
                    x: player_chunk.x,
                    y,
                    z: player_chunk.z + 1,
                });
                needed.push(ChunkPos {
                    x: player_chunk.x,
                    y,
                    z: player_chunk.z - 1,
                });
            }
            let loaded = needed.iter().filter(|p| self.chunks.contains_key(p)).count();
            if loaded == needed.len() {
                self.chunks_ready = true;
                info!("Chunks ready, starting gameplay");
            } else {
                // Render a loading frame
                self.camera.position = self.player_pos + Vec3::new(0.0, EYE_HEIGHT, 0.0);
                let Some(renderer) = self.renderer.as_mut() else { return; };
                renderer.update_camera_uniform(
                    &self.camera,
                    FOG_START,
                    FOG_END,
                    self.time_of_day,
                    false,
                    self.render_time_seconds,
                    0.0,
                );
                renderer.update_sky(&self.camera, self.time_of_day, 0.0);
                match renderer.render_main_menu_frame() {
                    Ok(()) => {}
                    Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                        renderer.resize(size.width, size.height);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        event_loop.exit();
                    }
                    Err(_) => {}
                }
                window.set_title("Veldspar - Loading...");
                self.input.clear_frame();
                return;
            }
        }

        let gameplay_input_blocked = self.console_open
            || self.chat_open
            || self.inventory_open
            || self.is_in_game_menu_open();
        let underwater;
        let mut pending_damage: f32 = 0.0;
        if !gameplay_input_blocked {
            self.camera
                .update_look(&self.input, self.settings.mouse_sensitivity * 0.001);
            let mut trample_farmland_on_landing = false;

            if self.fly_mode {
                let move_dir = self.camera.horizontal_movement_dir(&self.input);
                let mut movement = move_dir;
                if self.input.is_pressed(KeyCode::Space) {
                    movement.y += 1.0;
                }
                if self.input.is_pressed(KeyCode::ShiftLeft)
                    || self.input.is_pressed(KeyCode::ShiftRight)
                {
                    movement.y -= 1.0;
                }
                if movement.length_squared() > 0.0 {
                    self.player_pos += movement.normalize() * FLY_SPEED * dt;
                }
            } else if let Some(registry) = self.registry.as_ref() {
                let move_dir = self.camera.horizontal_movement_dir(&self.input);
                if move_dir.length_squared() <= 1e-6 {
                    self.sprinting = false;
                }
                let speed = if self.sprinting {
                    SPRINT_SPEED
                } else if self.input.is_pressed(KeyCode::ShiftLeft)
                    || self.input.is_pressed(KeyCode::ShiftRight)
                {
                    CROUCH_SPEED
                } else {
                    WALK_SPEED
                };
                let target_vx = move_dir.x * speed;
                let target_vz = move_dir.z * speed;

                // Check if standing on ice for slippery movement
                let ground_block = if self.on_ground {
                    let ground_pos = IVec3::new(
                        self.player_pos.x.floor() as i32,
                        (self.player_pos.y - 0.05).floor() as i32,
                        self.player_pos.z.floor() as i32,
                    );
                    block_at(ground_pos, &self.chunks)
                } else {
                    None
                };
                let ice_friction = match ground_block {
                    Some(b) if b.0 == 26 => 0.01_f32, // blue_ice - very slippery
                    Some(b) if b.0 == 24 || b.0 == 25 => 0.04_f32, // ice, packed_ice
                    _ => 1.0_f32, // normal - instant velocity change
                };

                if ice_friction < 1.0 {
                    let lerp_factor = 1.0 - (1.0 - ice_friction).powf(dt * 60.0);
                    self.velocity.x += (target_vx - self.velocity.x) * lerp_factor;
                    self.velocity.z += (target_vz - self.velocity.z) * lerp_factor;
                } else if !self.on_ground {
                    // Air control: reduced acceleration so player keeps momentum mid-jump
                    let air_accel: f32 = 0.12;
                    let lerp_factor = 1.0 - (1.0 - air_accel).powf(dt * 60.0);
                    self.velocity.x += (target_vx - self.velocity.x) * lerp_factor;
                    self.velocity.z += (target_vz - self.velocity.z) * lerp_factor;
                } else {
                    self.velocity.x = target_vx;
                    self.velocity.z = target_vz;
                }

                let touching_ladder = is_player_touching_ladder(self.player_pos, &self.chunks);

                if touching_ladder {
                    if self.input.is_pressed(KeyCode::KeyW) || self.input.is_pressed(KeyCode::Space)
                    {
                        self.velocity.y = LADDER_CLIMB_SPEED;
                    } else if self.velocity.y < 0.0 {
                        self.velocity.y = 0.0;
                    }
                } else {
                    if self.on_ground && self.input.is_pressed(KeyCode::Space) {
                        self.velocity.y = JUMP_VELOCITY;
                        self.on_ground = false;
                        if self.sprinting {
                            self.velocity.x *= 1.3;
                            self.velocity.z *= 1.3;
                        }
                    }

                    self.velocity.y += GRAVITY * dt;
                    self.velocity.y = self.velocity.y.clamp(-50.0, 50.0);
                }

                let is_crouching = self.on_ground
                    && (self.input.is_pressed(KeyCode::ShiftLeft)
                        || self.input.is_pressed(KeyCode::ShiftRight));

                // Resolve X axis
                let prev_x = self.player_pos.x;
                self.player_pos.x += self.velocity.x * dt;
                if collides_with_terrain(self.player_pos, &self.chunks, registry) {
                    if self.velocity.x > 0.0 {
                        self.player_pos.x =
                            (self.player_pos.x + PLAYER_HALF_W).floor() - PLAYER_HALF_W;
                    } else {
                        self.player_pos.x =
                            (self.player_pos.x - PLAYER_HALF_W).ceil() + PLAYER_HALF_W;
                    }
                    self.velocity.x = 0.0;
                }
                // Sneaking edge safety: revert X if player would walk off edge
                if is_crouching && !collides_with_terrain(
                    self.player_pos - Vec3::new(0.0, 0.1, 0.0),
                    &self.chunks,
                    registry,
                ) {
                    self.player_pos.x = prev_x;
                    self.velocity.x = 0.0;
                }

                // Resolve Y axis
                self.player_pos.y += self.velocity.y * dt;
                if collides_with_terrain(self.player_pos, &self.chunks, registry) {
                    if self.velocity.y < 0.0 {
                        let landed_this_frame = !self.on_ground;
                        if let Some(start_y) = self.fall_start_y.take() {
                            let fall_distance = start_y - self.player_pos.y;
                            if self.is_survival_mode() && fall_distance > FALL_DAMAGE_SAFE_DISTANCE {
                                pending_damage += fall_distance - FALL_DAMAGE_SAFE_DISTANCE;
                            }
                        }
                        self.player_pos.y = find_ground_snap(self.player_pos, &self.chunks, registry);
                        self.on_ground = true;
                        if landed_this_frame {
                            trample_farmland_on_landing = true;
                        }
                    } else {
                        self.player_pos.y =
                            (self.player_pos.y + PLAYER_HEIGHT).floor() - PLAYER_HEIGHT;
                        self.fall_start_y = Some(self.player_pos.y);
                    }
                    self.velocity.y = 0.0;
                } else {
                    self.on_ground = false;
                    if self.velocity.y < 0.0 {
                        if self.fall_start_y.is_none() {
                            self.fall_start_y = Some(self.player_pos.y);
                        }
                    } else {
                        self.fall_start_y = Some(self.player_pos.y);
                    }
                }

                // Resolve Z axis
                let prev_z = self.player_pos.z;
                self.player_pos.z += self.velocity.z * dt;
                if collides_with_terrain(self.player_pos, &self.chunks, registry) {
                    if self.velocity.z > 0.0 {
                        self.player_pos.z =
                            (self.player_pos.z + PLAYER_HALF_W).floor() - PLAYER_HALF_W;
                    } else {
                        self.player_pos.z =
                            (self.player_pos.z - PLAYER_HALF_W).ceil() + PLAYER_HALF_W;
                    }
                    self.velocity.z = 0.0;
                }
                // Sneaking edge safety: revert Z if player would walk off edge
                if is_crouching && !collides_with_terrain(
                    self.player_pos - Vec3::new(0.0, 0.1, 0.0),
                    &self.chunks,
                    registry,
                ) {
                    self.player_pos.z = prev_z;
                    self.velocity.z = 0.0;
                }
            }
            if trample_farmland_on_landing {
                self.maybe_trample_farmland_on_landing();
            }

            // Sprint FOV change (smooth lerp)
            let base_fov = self.settings.fov.to_radians();
            let target_fov = if self.sprinting && !self.fly_mode {
                base_fov + 0.15 // ~8.5 degrees wider when sprinting
            } else {
                base_fov
            };
            self.camera.fov += (target_fov - self.camera.fov) * (dt * 8.0).min(1.0);

            self.camera.position = self.player_pos + Vec3::new(0.0, EYE_HEIGHT, 0.0);
            underwater = is_block_water(
                self.camera.position.x as i32,
                self.camera.position.y as i32,
                self.camera.position.z as i32,
                &self.chunks,
            );

            // --- Survival damage + hunger systems ---
            if self.is_survival_mode() {
                if pending_damage > 0.0 {
                    self.apply_damage(pending_damage);
                }
                if underwater && !self.fly_mode {
                    self.air_supply = (self.air_supply - dt * 20.0).max(0.0);
                    if self.air_supply <= 0.0 {
                        self.apply_damage(DROWN_DAMAGE_PER_SEC * dt);
                    }
                    self.fall_start_y = None;
                } else {
                    self.air_supply = (self.air_supply + dt * 100.0).min(MAX_AIR_SUPPLY);
                }
                if self.player_pos.y < VOID_Y {
                    self.apply_damage(VOID_DAMAGE_PER_SEC * dt);
                }
                if is_player_touching_lava(self.player_pos, &self.chunks) {
                    self.apply_damage(LAVA_CONTACT_DAMAGE_PER_SEC * dt);
                }
                if !self.fly_mode {
                    let head_pos = IVec3::new(
                        self.player_pos.x.floor() as i32,
                        (self.player_pos.y + EYE_HEIGHT).floor() as i32,
                        self.player_pos.z.floor() as i32,
                    );
                    if let Some(registry) = self.registry.as_ref() {
                        if let Some(block) = block_at(head_pos, &self.chunks) {
                            let props = registry.get_properties(block);
                            if props.solid && block != BlockId::AIR {
                                self.apply_damage(1.0 * dt);
                            }
                        }
                    }
                }
                self.last_damage_time += dt;
                let sprinting_drain = if self.sprinting { HUNGER_SPRINT_DRAIN_RATE } else { 0.0 };
                self.hunger = (self.hunger - (HUNGER_PASSIVE_DRAIN_RATE + sprinting_drain) * dt)
                    .clamp(0.0, MAX_HUNGER);
                if self.hunger <= 0.0 {
                    self.apply_damage(STARVATION_DAMAGE_PER_SEC * dt);
                }
                if self.hunger > HUNGER_REGEN_THRESHOLD
                    && self.last_damage_time > HEALTH_REGEN_DELAY
                    && self.health < MAX_HEALTH
                    && self.health > 0.0
                {
                    self.health = (self.health + HEALTH_REGEN_RATE * dt).min(MAX_HEALTH);
                }
                self.damage_flash_timer = (self.damage_flash_timer - dt).max(0.0);
            } else {
                self.health = MAX_HEALTH;
                self.hunger = MAX_HUNGER;
                self.air_supply = MAX_AIR_SUPPLY;
                self.damage_flash_timer = 0.0;
                self.last_damage_time = -999.0;
            }

            // Falling blocks (sand, gravel) - tick every 0.25s
            self.block_physics_timer += dt;
            if self.block_physics_timer >= 0.25 && matches!(self.game_mode, Some(GameMode::Singleplayer)) {
                self.block_physics_timer = 0.0;
                self.tick_falling_blocks();
            }

            if matches!(self.game_mode, Some(GameMode::Singleplayer)) {
                // Process any pending fluid updates (triggered by block changes)
                self.process_pending_fluid_updates();

                self.tick_leaf_decay_timers(dt);

                self.tick_sapling_growth_timers(dt);

                self.tick_sugar_cane_growth_timers(dt);

                // Furnace smelting
                self.tick_furnaces(dt);

                // TNT explosions (processed in a queue to avoid recursive chain calls).
                self.process_tnt_explosion_queue();

                // Pressure plates
                self.tick_pressure_plates();
            }
            let horizontal_speed = Vec2::new(self.velocity.x, self.velocity.z).length();
            let is_walking_on_ground =
                !self.fly_mode && self.on_ground && horizontal_speed > 0.25 && !underwater;
            if is_walking_on_ground {
                self.walk_particle_timer -= dt.max(0.0);
            } else {
                self.walk_particle_timer = 0.0;
            }
            if is_walking_on_ground && self.walk_particle_timer <= 0.0 {
                let ground_pos = IVec3::new(
                    self.player_pos.x.floor() as i32,
                    (self.player_pos.y - 0.05).floor() as i32,
                    self.player_pos.z.floor() as i32,
                );
                if let (Some(registry), Some(ground_block)) =
                    (self.registry.as_ref(), block_at(ground_pos, &self.chunks))
                {
                    if ground_block != BlockId::AIR {
                        let props = registry.get_properties(ground_block);
                        if props.solid && props.name.as_str() != "still_water" {
                            let mut dust_color = block_particle_color(ground_block, registry);
                            dust_color[0] *= 0.85;
                            dust_color[1] *= 0.85;
                            dust_color[2] *= 0.85;
                            if let Some(renderer) = self.renderer.as_mut() {
                                renderer.spawn_walk_particles(
                                    Vec3::new(
                                        self.player_pos.x,
                                        self.player_pos.y + 0.04,
                                        self.player_pos.z,
                                    ),
                                    dust_color,
                                );
                            }
                        }
                    }
                }
                self.walk_particle_timer = if self.sprinting { 0.11 } else { 0.17 };
            }

            // Compute targeted block for highlight
            self.targeted_block = None;
            if let Some(registry) = self.registry.as_ref() {
                let ray = Ray {
                    origin: self.camera.position,
                    direction: self.camera.forward_direction(),
                };
                for (block_pos, _face) in raycast_blocks(&ray, 8.0) {
                    let (chunk_pos, local_pos) = world_to_chunk(block_pos);
                    if let Some(chunk) = self.chunks.get(&chunk_pos) {
                        let idx = local_to_index(local_pos);
                        let block = chunk.blocks[idx];
                        if is_block_break_target(block, registry) {
                            self.targeted_block = Some(block_pos);
                            break;
                        }
                    }
                }
            }

            let left_click_just_pressed = self.input.left_click && !self.was_left_click_down;
            if left_click_just_pressed {
                self.attack_animation = 1.0;
                if self.targeted_block.is_none() {
                    let _ = self.try_attack_mob_in_front();
                }
            }

            // Handle block breaking
            if matches!(self.play_mode, PlayMode::Creative) {
                self.break_progress = 0.0;
                self.breaking_block = None;
                if left_click_just_pressed {
                    if let Some(targeted_block) = self.targeted_block {
                        self.break_targeted_block_now(targeted_block);
                        self.refresh_selected_block();
                    }
                }
            } else if !self.input.left_click {
                self.break_progress = 0.0;
                self.breaking_block = None;
            } else if let Some(targeted_block) = self.targeted_block {
                if self.breaking_block != Some(targeted_block) {
                    self.break_progress = 0.0;
                    self.breaking_block = Some(targeted_block);
                } else {
                    if let Some(target_block_type) = block_at(targeted_block, &self.chunks) {
                        let held_item = self
                            .inventory
                            .get(self.selected_hotbar_slot)
                            .map(|s| s.item)
                            .unwrap_or(ItemId::from(BlockId::AIR));
                        let effective_time = self
                            .registry
                            .as_ref()
                            .map(|registry| effective_break_time(target_block_type, held_item, registry))
                            .unwrap_or(0.05);
                        if effective_time <= 0.0 {
                            self.break_progress = 1.0;
                        } else {
                            self.break_progress += dt / effective_time;
                        }
                        if self.break_progress >= 1.0 {
                            self.break_targeted_block_now(targeted_block);
                            self.break_progress = 0.0;
                            self.breaking_block = None;
                            if matches!(self.play_mode, PlayMode::Survival) {
                                self.consume_held_tool_durability_on_break();
                            }
                            self.refresh_selected_block();
                        }
                    } else {
                        self.break_progress = 0.0;
                        self.breaking_block = None;
                    }
                }
            } else {
                self.break_progress = 0.0;
                self.breaking_block = None;
            }

            // Handle block placement
            if self.input.consume_right_click() {
                self.attack_animation = 1.0;
                let registry = self.registry.clone();
                let mut block_edits_to_send: Vec<(IVec3, BlockId)> = Vec::new();
                let mut remesh_updates: Vec<(ChunkPos, LocalPos, BlockId, BlockId)> = Vec::new();
                let mut consume_selected_item = false;
                let ray = Ray {
                    origin: self.camera.position,
                    direction: self.camera.forward_direction(),
                };
                for (block_pos, face) in raycast_blocks(&ray, 8.0) {
                    let Some(block) = block_at(block_pos, &self.chunks) else {
                        continue;
                    };
                    if block == BlockId::AIR {
                        continue;
                    }
                    if block == BlockId::CHEST {
                        self.open_chest_ui(block_pos);
                        break;
                    }
                    if block == BlockId::CRAFTING_TABLE {
                        self.open_inventory_ui(inventory::CraftingUiMode::CraftingTable3x3);
                        break;
                    }
                    // Furnace interaction
                    if block == BlockId::FURNACE {
                        self.open_furnace = Some(block_pos);
                        self.furnace_data.entry(block_pos).or_insert_with(FurnaceState::new);
                        self.inventory_open = true;
                        self.reset_inventory_drag_distribution();
                        self.set_cursor_grab(false);
                        break;
                    }
                    // Trapdoor interaction
                    if is_trapdoor_block(block) {
                        let toggled = trapdoor_toggle(block);
                        if let Some((chunk_pos, local_pos, previous, next)) =
                            try_set_world_block(&mut self.chunks, block_pos, toggled)
                        {
                            self.dirty_chunks.insert(chunk_pos);
                            remesh_updates.push((chunk_pos, local_pos, previous, next));
                            block_edits_to_send.push((block_pos, toggled));
                        }
                        break;
                    }
                    // Bed interaction
                    if is_bed_block(block) {
                        let is_night = self.time_of_day > 0.75 || self.time_of_day < 0.25;
                        if is_night {
                            self.spawn_position = self.player_pos;
                            self.bed_spawn_point = Some(self.player_pos);
                            if matches!(self.game_mode, Some(GameMode::Singleplayer)) {
                                self.time_of_day = 0.3;
                            }
                            self.push_chat_message("System", "Spawn point set. Good morning!");
                        } else {
                            self.push_chat_message("System", "You can only sleep at night");
                        }
                        break;
                    }

                    if is_lever(block) {
                        let toggled = if block == BlockId::LEVER_OFF {
                            BlockId::LEVER_ON
                        } else {
                            BlockId::LEVER_OFF
                        };
                        self.set_block_and_collect_update(
                            block_pos,
                            toggled,
                            &mut remesh_updates,
                            &mut block_edits_to_send,
                        );
                        self.toggle_adjacent_doors_with_updates(
                            block_pos,
                            &mut remesh_updates,
                            &mut block_edits_to_send,
                        );
                        break;
                    }

                    if is_button(block) {
                        self.set_block_and_collect_update(
                            block_pos,
                            BlockId::STONE_BUTTON_ON,
                            &mut remesh_updates,
                            &mut block_edits_to_send,
                        );
                        self.set_adjacent_doors_open_with_updates(
                            block_pos,
                            true,
                            &mut remesh_updates,
                            &mut block_edits_to_send,
                        );
                        self.button_timers.retain(|(pos, _)| *pos != block_pos);
                        self.button_timers.push((block_pos, 1.0));
                        break;
                    }

                    if let Some((lower_pos, upper_pos, facing, is_open)) =
                        door_interaction_state(block_pos, block, &self.chunks)
                    {
                        let new_lower = door_block_for_state(facing, !is_open, false);
                        let new_upper = door_block_for_state(facing, !is_open, true);
                        for (world_pos, new_block) in
                            [(lower_pos, new_lower), (upper_pos, new_upper)]
                        {
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, world_pos, new_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((world_pos, new_block));
                            }
                        }
                        break;
                    }

                    // Hoe: convert verdant turf/soil to farmland.
                    if self
                        .inventory
                        .get(self.selected_hotbar_slot)
                        .is_some_and(|stack| {
                            tool_properties(stack.item)
                                .is_some_and(|(kind, _)| kind == ToolKind::Hoe)
                        })
                        && (block == BlockId::VERDANT_TURF || block == BlockId::LOAM)
                    {
                        if let Some((chunk_pos, local_pos, previous, next)) =
                            try_set_world_block(&mut self.chunks, block_pos, BlockId::FARMLAND)
                        {
                            self.dirty_chunks.insert(chunk_pos);
                            remesh_updates.push((chunk_pos, local_pos, previous, next));
                            block_edits_to_send.push((block_pos, BlockId::FARMLAND));
                            if let Some(registry_ref) = registry.as_ref() {
                                if let Some(renderer) = self.renderer.as_mut() {
                                    renderer.spawn_break_particles(
                                        block_pos,
                                        block_particle_color(block, registry_ref),
                                    );
                                }
                            }
                            if self.is_survival_mode() {
                                self.consume_held_tool_durability_on_use(ToolKind::Hoe);
                                self.refresh_selected_block();
                            }
                        }
                        break;
                    }

                    if self.try_apply_bone_meal(
                        block_pos,
                        &mut remesh_updates,
                        &mut block_edits_to_send,
                    ) {
                        consume_selected_item = true;
                        break;
                    }

                    let Some(registry) = registry.as_ref() else {
                        break;
                    };
                    if !block_supports_attachment(block_pos, block, &self.chunks, registry) {
                        continue;
                    }

                    let has_item = matches!(self.play_mode, PlayMode::Creative)
                        || self
                            .inventory
                            .get(self.selected_hotbar_slot)
                            .is_some_and(|stack| stack.count > 0);
                    if !has_item {
                        break;
                    }

                    let place_pos = block_pos + face.normal_ivec3();
                    let held_item = self
                        .inventory
                        .get(self.selected_hotbar_slot)
                        .filter(|stack| stack.count > 0)
                        .map(|stack| stack.item);
                    let place_block = if let Some(item) = held_item {
                        if item == ItemId::WHEAT_SEEDS {
                            BlockId::WHEAT_STAGE_0
                        } else {
                            normalize_place_block(item.as_block_id().unwrap_or(BlockId::AIR))
                        }
                    } else {
                        normalize_place_block(self.selected_block)
                    };
                    if place_block == BlockId::AIR {
                        break;
                    }

                    if place_block == BlockId::WOODEN_DOOR {
                        let upper_pos = place_pos + IVec3::Y;
                        let lower_air = block_at(place_pos, &self.chunks) == Some(BlockId::AIR);
                        let upper_air = block_at(upper_pos, &self.chunks) == Some(BlockId::AIR);
                        let space_free = lower_air
                            && upper_air
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                            && !player_aabb_overlaps_block(self.player_pos, upper_pos);
                        if space_free {
                            let facing = facing_from_yaw(self.camera.yaw);
                            let lower_block = door_block_for_state(facing, false, false);
                            let upper_block = door_block_for_state(facing, false, true);
                            for (world_pos, new_block) in
                                [(place_pos, lower_block), (upper_pos, upper_block)]
                            {
                                if let Some((chunk_pos, local_pos, previous, next)) =
                                    try_set_world_block(&mut self.chunks, world_pos, new_block)
                                {
                                    self.dirty_chunks.insert(chunk_pos);
                                    remesh_updates.push((chunk_pos, local_pos, previous, next));
                                    block_edits_to_send.push((world_pos, new_block));
                                }
                            }
                            consume_selected_item = true;
                        }
                        break;
                    }

                    if place_block == BlockId::LADDER {
                        let Some(facing) = facing_from_placement_face(face) else {
                            break;
                        };
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                        {
                            let ladder_block = ladder_block_for_facing(facing);
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, ladder_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, ladder_block));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    if is_sign(place_block) {
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                        {
                            let sign_block = sign_block_for_yaw(self.camera.yaw);
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, sign_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, sign_block));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    if place_block == BlockId::SAPLING {
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                            && can_sapling_stay(place_pos, &self.chunks)
                        {
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, place_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, place_block));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    if place_block == BlockId::SUGAR_CANE {
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                            && can_place_sugar_cane(place_pos, &self.chunks, registry)
                        {
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, place_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, place_block));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    // Trapdoor placement - orient by facing
                    if place_block == BlockId::TRAPDOOR_CLOSED {
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                        {
                            let facing = facing_from_yaw(self.camera.yaw);
                            let trapdoor_block = trapdoor_block_for_facing(facing);
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, trapdoor_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, trapdoor_block));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    // Bed placement - 2 blocks
                    if place_block == BlockId::BED_FOOT {
                        let facing = facing_from_yaw(self.camera.yaw);
                        let head_offset = facing_to_ivec3(facing);
                        let head_pos = place_pos + head_offset;
                        let foot_air = block_at(place_pos, &self.chunks) == Some(BlockId::AIR);
                        let head_air = block_at(head_pos, &self.chunks) == Some(BlockId::AIR);
                        if foot_air && head_air
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                            && !player_aabb_overlaps_block(self.player_pos, head_pos)
                        {
                            let (foot_block, head_block) = bed_blocks_for_facing(facing);
                            for (world_pos, new_block) in [(place_pos, foot_block), (head_pos, head_block)] {
                                if let Some((chunk_pos, local_pos, previous, next)) =
                                    try_set_world_block(&mut self.chunks, world_pos, new_block)
                                {
                                    self.dirty_chunks.insert(chunk_pos);
                                    remesh_updates.push((chunk_pos, local_pos, previous, next));
                                    block_edits_to_send.push((world_pos, new_block));
                                }
                            }
                            consume_selected_item = true;
                        }
                        break;
                    }

                    // Slab placement - top or bottom based on hit position
                    if place_block == BlockId::STONE_SLAB_BOTTOM || place_block == BlockId::WOODEN_SLAB_BOTTOM {
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                        {
                            let slab_block = if face == Face::NegY {
                                // Placing against bottom face -> top slab
                                if place_block == BlockId::STONE_SLAB_BOTTOM { BlockId::STONE_SLAB_TOP } else { BlockId::WOODEN_SLAB_TOP }
                            } else {
                                place_block
                            };
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, slab_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, slab_block));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    // Wheat seeds placement - on farmland only
                    if place_block == BlockId::WHEAT_STAGE_0 {
                        let below = place_pos - IVec3::Y;
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && block_at(below, &self.chunks) == Some(BlockId::FARMLAND)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                        {
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, BlockId::WHEAT_STAGE_0)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, BlockId::WHEAT_STAGE_0));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    // Pressure plate placement - on solid blocks
                    if place_block == BlockId::STONE_PRESSURE_PLATE {
                        if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                            && !player_aabb_overlaps_block(self.player_pos, place_pos)
                        {
                            if let Some((chunk_pos, local_pos, previous, next)) =
                                try_set_world_block(&mut self.chunks, place_pos, place_block)
                            {
                                self.dirty_chunks.insert(chunk_pos);
                                remesh_updates.push((chunk_pos, local_pos, previous, next));
                                block_edits_to_send.push((place_pos, place_block));
                                consume_selected_item = true;
                            }
                        }
                        break;
                    }

                    if block_at(place_pos, &self.chunks) == Some(BlockId::AIR)
                        && !player_aabb_overlaps_block(self.player_pos, place_pos)
                    {
                        if let Some((chunk_pos, local_pos, previous, next)) =
                            try_set_world_block(&mut self.chunks, place_pos, place_block)
                        {
                            self.dirty_chunks.insert(chunk_pos);
                            remesh_updates.push((chunk_pos, local_pos, previous, next));
                            block_edits_to_send.push((place_pos, place_block));
                            consume_selected_item = true;
                        }
                    }
                    break;
                }

                for (chunk_pos, local_pos, previous, next) in remesh_updates {
                    self.remesh_for_block_change(chunk_pos, local_pos, previous, next);
                }

                // Schedule fluid update only when placing a block near water/lava
                for (edit_pos, edit_block) in &block_edits_to_send {
                    if self.is_near_fluid(*edit_pos, *edit_block) {
                        self.schedule_fluid_update(*edit_pos);
                    }
                }

                if consume_selected_item && self.is_survival_mode() {
                    self.inventory.remove_item(self.selected_hotbar_slot, 1);
                    self.refresh_selected_block();
                }

                if let Some(GameMode::Multiplayer { ref mut net }) = self.game_mode {
                    for (world_pos, new_block) in block_edits_to_send {
                        info!("Sending block edit {world_pos:?} -> block {}", new_block.0);
                        net.send_reliable(&C2S::BlockEdit { world_pos, new_block });
                    }
                }
            }
        } else {
            self.input.left_click = false;
            self.input.right_click = false;
            self.break_progress = 0.0;
            self.breaking_block = None;
            self.targeted_block = None;
            self.camera.position = self.player_pos + Vec3::new(0.0, EYE_HEIGHT, 0.0);
            underwater = is_block_water(
                self.camera.position.x as i32,
                self.camera.position.y as i32,
                self.camera.position.z as i32,
                &self.chunks,
            );
            self.walk_particle_timer = 0.0;
        }

        if let Some(registry) = self.registry.clone() {
            self.update_mobs(dt, &registry);
        }

        self.tick_button_timers(dt);
        self.update_item_drops(dt);
        self.tick_weather(dt);
        let weather_dim = self.weather_dim_amount();
        let mut rain_spawn_positions: Vec<Vec3> = Vec::new();
        let mut snow_spawn_positions: Vec<Vec3> = Vec::new();
        match self.weather_state {
            WeatherState::Clear => {}
            WeatherState::Rain => {
                let spawn_target = self.weather_random_count(
                    WEATHER_RAIN_MIN_SPAWN_PER_FRAME,
                    WEATHER_RAIN_MAX_SPAWN_PER_FRAME,
                );
                let candidates = self.generate_weather_spawn_candidates(
                    spawn_target * 3,
                    WEATHER_RAIN_MIN_HEIGHT,
                    WEATHER_RAIN_MAX_HEIGHT,
                );
                if let Some(registry) = self.registry.as_ref() {
                    for candidate in candidates {
                        if self.can_spawn_weather_at(candidate, registry) {
                            rain_spawn_positions.push(candidate);
                            if rain_spawn_positions.len() >= spawn_target {
                                break;
                            }
                        }
                    }
                }
            }
            WeatherState::Snow => {
                let spawn_target = self.weather_random_count(
                    WEATHER_SNOW_MIN_SPAWN_PER_FRAME,
                    WEATHER_SNOW_MAX_SPAWN_PER_FRAME,
                );
                let candidates = self.generate_weather_spawn_candidates(
                    spawn_target * 3,
                    WEATHER_SNOW_MIN_HEIGHT,
                    WEATHER_SNOW_MAX_HEIGHT,
                );
                if let Some(registry) = self.registry.as_ref() {
                    for candidate in candidates {
                        if self.can_spawn_weather_at(candidate, registry) {
                            snow_spawn_positions.push(candidate);
                            if snow_spawn_positions.len() >= spawn_target {
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Advance time of day (full day in 20 minutes)
        if !self.time_frozen {
            self.time_of_day = (self.time_of_day + dt / 1200.0) % 1.0;
        }

        self.attack_animation = (self.attack_animation - dt * ATTACK_ANIMATION_DECAY).max(0.0);
        self.was_left_click_down = self.input.left_click;

        if let Some(chest_pos) = self.open_chest {
            if block_at(chest_pos, &self.chunks) != Some(BlockId::CHEST) {
                self.set_inventory_open(false);
            }
        }
        if let Some(partner_pos) = self.double_chest_partner {
            if block_at(partner_pos, &self.chunks) != Some(BlockId::CHEST) {
                self.set_inventory_open(false);
            }
        }

        self.fps_frame_count = self.fps_frame_count.saturating_add(1);
        if self.fps <= 0.0 {
            self.fps = 1.0 / dt.max(0.0001);
        }
        match self.fps_sample_start {
            Some(sample_start) => {
                let elapsed = (now - sample_start).as_secs_f32();
                if elapsed >= 1.0 {
                    self.fps = self.fps_frame_count as f32 / elapsed;
                    self.fps_frame_count = 0;
                    self.fps_sample_start = Some(now);
                }
            }
            None => {
                self.fps_sample_start = Some(now);
                self.fps_frame_count = 0;
            }
        }

        let overlay_lines = self.build_overlay_lines();
        let chat_lines = self.build_chat_overlay_lines(now);
        let chat_input_line = self.chat_open.then(|| format!("> {}", self.chat_input));
        if self.show_debug {
            let (mode, connection_status) = self.mode_and_connection_status();
            let frame_time_stats = self.frame_time_stats();
            let debug_info = DebugInfo::from_camera(
                &self.camera,
                self.fps,
                self.selected_block,
                self.fly_mode,
                self.chunks.len(),
                self.pending_chunks.len(),
                self.mesh_queue.len(),
                mode,
                connection_status,
                self.last_render_stats,
                self.last_upload_stats.uploaded_bytes,
                self.last_upload_stats.uploaded_chunks,
                self.last_upload_stats.buffer_reallocations,
                frame_time_stats.avg_ms,
                frame_time_stats.p95_ms,
                frame_time_stats.p99_ms,
                frame_time_stats.max_ms,
            );
            window.set_title(&debug_info.window_title());
        } else if self.is_in_game_menu_open() {
            window.set_title("Veldspar Client | Paused");
        } else if self.console_open {
            window.set_title("Veldspar Client | Console");
        } else if self.chat_open {
            window.set_title("Veldspar Client | Chat");
        } else if self.inventory_open {
            if self.open_chest.is_some() {
                window.set_title("Veldspar Client | Chest");
            } else if self.is_creative_inventory_ui_active() {
                window.set_title("Veldspar Client | Creative Inventory");
            } else if matches!(
                self.crafting_ui_mode,
                inventory::CraftingUiMode::CraftingTable3x3
            ) {
                window.set_title("Veldspar Client | Crafting Table");
            } else {
                window.set_title("Veldspar Client | Inventory");
            }
        } else {
            window.set_title("Veldspar Client");
        }

        let show_in_game_menu = self.is_in_game_menu_open();
        let show_survival_hud = self.is_survival_mode();
        let show_crosshair = !self.inventory_open
            && !self.console_open
            && !self.chat_open
            && !show_in_game_menu;
        let item_drop_render_data = self.build_item_drop_render_data();
        let mut crafting_inputs = [None; 9];
        match self.crafting_ui_mode {
            inventory::CraftingUiMode::Inventory2x2 => {
                crafting_inputs[..4].copy_from_slice(&self.inventory_crafting_slots);
            }
            inventory::CraftingUiMode::CraftingTable3x3 => {
                crafting_inputs.copy_from_slice(&self.table_crafting_slots);
            }
        }
        let crafting_output = self.active_crafting_result();
        let chest_inventory_view = if let Some(slots) = self.double_chest_slots.as_ref() {
            // The current UI exposes 27 chest slots; keep the first half visible and persist all 54 internally.
            let mut view = Inventory::new();
            for slot in 0..SINGLE_CHEST_SLOT_COUNT {
                view.slots[slot] = slots[slot];
            }
            Some(view)
        } else {
            self.open_chest
                .and_then(|chest_pos| self.chest_inventories.get(&chest_pos).cloned())
        };
        inventory::set_creative_ui_state(
            self.is_creative_inventory_ui_active(),
            &self.creative_search,
            self.creative_scroll,
            &self.creative_catalog,
        );
        let xp_progress = self.xp_progress();
        let xp_level = self.xp_level;
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        renderer.reserve_chunk_mesh_capacity(self.chunks.len());

        renderer.update_camera_uniform(
            &self.camera,
            FOG_START,
            FOG_END,
            self.time_of_day,
            underwater,
            self.render_time_seconds,
            weather_dim,
        );
        renderer.update_sky(&self.camera, self.time_of_day, weather_dim);
        renderer.update_clouds(&self.camera, self.time_of_day);
        if !rain_spawn_positions.is_empty() {
            renderer.spawn_rain_particles(&rain_spawn_positions);
        }
        if !snow_spawn_positions.is_empty() {
            renderer.spawn_snow_particles(&snow_spawn_positions);
        }
        renderer.update_particles(dt, &self.camera);
        renderer.update_item_drops(&item_drop_render_data);
        renderer.update_highlight(self.targeted_block);
        renderer.update_break_indicator(
            self.breaking_block,
            self.break_progress,
            &self.remote_break_overlays,
        );
        renderer.update_inventory_ui(
            self.settings.gui_scale,
            &self.inventory,
            self.selected_hotbar_slot,
            self.inventory_open,
            self.crafting_ui_mode,
            &crafting_inputs,
            crafting_output,
            chest_inventory_view.as_ref(),
            self.cursor_stack,
            self.cursor_position,
            self.registry.as_deref(),
        );
        renderer.update_health_hud(
            self.settings.gui_scale,
            self.health,
            self.hunger,
            self.damage_flash_timer,
            show_survival_hud,
            self.air_supply,
            MAX_AIR_SUPPLY,
            xp_progress,
            xp_level,
        );
        renderer.set_crosshair_visible(show_crosshair);
        renderer.update_players(&self.remote_players);
        let mob_infos: Vec<MobRenderInfo> = self
            .mobs
            .iter()
            .map(|mob| {
                let props = mob_properties(mob.mob_type);
                MobRenderInfo {
                    position: Vec3::from_array(mob.position),
                    yaw: mob.yaw,
                    width: props.width,
                    height: props.height,
                    color: mob_color(mob.mob_type),
                    hurt_flash: mob.hurt_timer > 0.0,
                }
            })
            .collect();
        renderer.update_mob_data(&mob_infos);
        let mut hand_item = selected_item_from_inventory(&self.inventory, self.selected_hotbar_slot);
        if hand_item == ItemId::from(BlockId::AIR)
            && matches!(self.play_mode, PlayMode::Creative)
            && self.selected_block != BlockId::AIR
        {
            hand_item = ItemId::from(self.selected_block);
        }
        renderer.update_first_person_hand_item(hand_item);
        renderer.update_first_person_hand(
            self.camera.position,
            self.camera.forward_direction(),
            self.attack_animation,
        );
        renderer.update_overlay_lines(&overlay_lines, &chat_lines, chat_input_line.as_deref());
        if matches!(self.in_game_menu, InGameMenuState::Pause) {
            renderer.update_pause_menu(self.pause_menu_selected);
        } else if matches!(self.in_game_menu, InGameMenuState::Settings) {
            renderer.update_settings_menu(
                self.settings_menu_selected,
                &SettingsMenuView {
                    render_distance: self.settings.render_distance,
                    stream_surface_below: self.settings.stream_surface_below,
                    stream_flight_below: self.settings.stream_flight_below,
                    stream_above: self.settings.stream_above,
                    lod1_distance: self.settings.lod1_distance,
                    mouse_sensitivity: self.settings.mouse_sensitivity,
                    fov: self.settings.fov,
                    gui_scale: self.settings.gui_scale,
                    show_fps: self.settings.show_fps,
                },
            );
        }

        // Auto-save for singleplayer (do before renderer borrow scope)
        let should_save = matches!(self.game_mode, Some(GameMode::Singleplayer))
            && self.last_save_time.map_or(false, |t| now.duration_since(t).as_secs_f32() >= SAVE_INTERVAL_SECS);

        self.input.clear_frame();

        match renderer.render_frame(show_in_game_menu) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                renderer.resize(size.width, size.height);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                error!("Out of GPU memory; shutting down client event loop");
                event_loop.exit();
            }
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {}
        }
        self.last_render_stats = renderer.last_frame_stats();

        if should_save {
            self.save_world();
            self.last_save_time = Some(now);
        }
        self.log_performance_metrics_if_due();
    }
}

impl ApplicationHandler for ClientApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes().with_title("Veldspar Client");
        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                match Renderer::new(window.clone()) {
                    Ok(renderer) => {
                        let size = window.inner_size();
                        if size.width > 0 && size.height > 0 {
                            self.camera.aspect = size.width as f32 / size.height as f32;
                        }
                        self.player_pos = Vec3::new(0.0, 40.0, 0.0);
                        self.camera.position =
                            self.player_pos + Vec3::new(0.0, EYE_HEIGHT, 0.0);
                        renderer.update_camera_uniform(
                            &self.camera,
                            FOG_START,
                            FOG_END,
                            self.time_of_day,
                            false,
                            self.render_time_seconds,
                            0.0,
                        );

                        info!("Client window and renderer initialized");
                        self.window = Some(window.clone());
                        self.renderer = Some(renderer);
                        // Start in MainMenu — don't grab cursor or load chunks yet
                        self.app_state = AppState::MainMenu;
                        let now = Instant::now();
                        self.last_frame = Some(now);
                        self.fps_sample_start = Some(now);
                        self.fps_frame_count = 0;
                        self.fps = 0.0;
                    }
                    Err(err) => {
                        error!("failed to initialize renderer: {err}");
                        event_loop.exit();
                    }
                }
            }
            Err(err) => {
                error!("failed to create client window: {err}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                info!("Close requested; shutting down client event loop");
                self.save_world_full();
                event_loop.exit();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Some((position.x, position.y));
                if let Some(window) = self.window.as_ref() {
                    let size = window.inner_size();
                    match self.app_state {
                        AppState::MainMenu => {
                            let hovered = crate::ui::main_menu::MainMenuRenderer::hit_test(
                                position.x as f32,
                                position.y as f32,
                                size.width,
                                size.height,
                            );
                            if let Some(btn) = hovered {
                                self.menu_selected = btn;
                            }
                        }
                        AppState::WorldSelect => {
                            let hovered =
                                crate::ui::main_menu::MainMenuRenderer::hit_test_world_select(
                                    position.x as f32,
                                    position.y as f32,
                                    size.width,
                                    size.height,
                                    self.world_entries.len(),
                                    self.world_selected,
                                    self.world_create_form_open,
                                    self.world_delete_confirmation_open,
                                );
                            if let Some(WorldSelectHitTarget::WorldEntry(index)) = hovered {
                                self.world_selected = Some(index);
                            }
                        }
                        AppState::InGame => {
                            if self.inventory_open && self.drag_distributing {
                                if let Some(hit_target) = inventory::hit_test_open_target(
                                    position.x as f32,
                                    position.y as f32,
                                    size.width,
                                    size.height,
                                    self.settings.gui_scale,
                                    self.crafting_ui_mode,
                                    self.open_chest.is_some(),
                                ) {
                                    if let inventory::OpenInventoryHitTarget::InventorySlot(slot) =
                                        hit_target
                                    {
                                        self.track_inventory_drag_slot(slot);
                                    }
                                }
                            }

                            if matches!(self.in_game_menu, InGameMenuState::Pause) {
                                if let Some(hit) = crate::ui::main_menu::MainMenuRenderer::hit_test_pause(
                                    position.x as f32,
                                    position.y as f32,
                                    size.width,
                                    size.height,
                                ) {
                                    self.pause_menu_selected = match hit {
                                        PauseMenuHitTarget::Resume => 0,
                                        PauseMenuHitTarget::Settings => 1,
                                        PauseMenuHitTarget::SaveAndQuit => 2,
                                    };
                                }
                            } else if matches!(self.in_game_menu, InGameMenuState::Settings) {
                                if let Some(slider) = self.active_settings_slider {
                                    let fraction =
                                        crate::ui::main_menu::MainMenuRenderer::settings_slider_fraction(
                                            slider,
                                            position.x as f32,
                                            size.width,
                                            size.height,
                                        );
                                    self.set_slider_from_fraction(slider, fraction);
                                }
                                if let Some(hit) = crate::ui::main_menu::MainMenuRenderer::hit_test_settings(
                                    position.x as f32,
                                    position.y as f32,
                                    size.width,
                                    size.height,
                                ) {
                                    self.settings_menu_selected =
                                        crate::ui::main_menu::MainMenuRenderer::settings_target_to_selection(hit);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };

                // Main Menu input handling
                if matches!(self.app_state, AppState::MainMenu) {
                    if event.state == ElementState::Pressed {
                        match code {
                            KeyCode::ArrowUp => {
                                self.menu_selected = self.menu_selected.saturating_sub(1);
                            }
                            KeyCode::ArrowDown => {
                                if self.menu_selected < 2 {
                                    self.menu_selected += 1;
                                }
                            }
                            KeyCode::Enter => match self.menu_selected {
                                0 => self.enter_world_select(),
                                1 => self.start_multiplayer(),
                                2 => event_loop.exit(),
                                _ => {}
                            },
                            KeyCode::Backspace => {
                                if self.menu_selected == 1 {
                                    self.server_ip.pop();
                                }
                            }
                            _ => {}
                        }
                        // Text input for IP field
                        if self.menu_selected == 1 {
                            if let Some(text) = &event.text {
                                for ch in text.chars() {
                                    if ch.is_ascii_digit() || ch == '.' || ch == ':' {
                                        self.server_ip.push(ch);
                                    }
                                }
                            }
                        }
                    }
                    return;
                }

                // World Select input handling
                if matches!(self.app_state, AppState::WorldSelect) {
                    if event.state == ElementState::Pressed {
                        if self.world_delete_confirmation_open {
                            match code {
                                KeyCode::Escape => {
                                    self.world_delete_confirmation_open = false;
                                }
                                KeyCode::Enter => self.delete_selected_world(),
                                _ => {}
                            }
                            return;
                        }

                        if self.world_create_form_open {
                            match code {
                                KeyCode::Escape => self.close_create_world_form(),
                                KeyCode::Tab => {
                                    self.world_create_active_field =
                                        match self.world_create_active_field {
                                            WorldCreateInputField::Name => {
                                                WorldCreateInputField::Seed
                                            }
                                            WorldCreateInputField::Seed => {
                                                WorldCreateInputField::Name
                                            }
                                        };
                                }
                                KeyCode::Enter => {
                                    if self.world_create_active_field == WorldCreateInputField::Name
                                    {
                                        self.world_create_active_field = WorldCreateInputField::Seed;
                                    } else {
                                        self.create_world_from_form();
                                    }
                                }
                                KeyCode::Backspace => match self.world_create_active_field {
                                    WorldCreateInputField::Name => {
                                        self.world_create_name_input.pop();
                                    }
                                    WorldCreateInputField::Seed => {
                                        self.world_create_seed_input.pop();
                                    }
                                },
                                KeyCode::KeyG => self.toggle_world_create_play_mode(),
                                _ => {
                                    if let Some(text) = &event.text {
                                        self.append_world_form_text(text);
                                    }
                                }
                            }
                            return;
                        }

                        match code {
                            KeyCode::ArrowUp => self.select_previous_world(),
                            KeyCode::ArrowDown => self.select_next_world(),
                            KeyCode::Enter => self.play_selected_world(),
                            KeyCode::Delete => {
                                if self.world_selected.is_some() {
                                    self.world_delete_confirmation_open = true;
                                }
                            }
                            KeyCode::KeyN => self.open_create_world_form(),
                            KeyCode::Escape => {
                                self.app_state = AppState::MainMenu;
                                self.world_delete_confirmation_open = false;
                            }
                            _ => {}
                        }
                    }
                    return;
                }

                // InGame input handling
                if event.state == ElementState::Pressed && !event.repeat && code == KeyCode::F3 {
                    self.set_show_fps(!self.settings.show_fps);
                    if !self.show_debug {
                        if let Some(window) = self.window.as_ref() {
                            window.set_title("Veldspar Client");
                        }
                    }
                    return;
                }

                if event.state == ElementState::Pressed && !event.repeat && code == KeyCode::KeyE {
                    if !self.is_in_game_menu_open() && !self.console_open && !self.chat_open {
                        self.toggle_inventory();
                        return;
                    }
                }

                if self.inventory_open {
                    if event.state == ElementState::Pressed {
                        if !event.repeat && code == KeyCode::Escape {
                            self.set_inventory_open(false);
                            return;
                        }

                        if self.is_creative_inventory_ui_active() {
                            match code {
                                KeyCode::Backspace => {
                                    if self.creative_search.pop().is_some() {
                                        self.filter_creative_catalog();
                                    }
                                }
                                _ => {
                                    if let Some(text) = &event.text {
                                        self.append_creative_search_text(text);
                                    }
                                }
                            }
                        }
                    }
                    return;
                }

                if self.is_in_game_menu_open() {
                    if event.state == ElementState::Pressed {
                        match self.in_game_menu {
                            InGameMenuState::Pause => self.handle_pause_menu_keyboard(code),
                            InGameMenuState::Settings => self.handle_settings_menu_keyboard(code),
                            InGameMenuState::None => {}
                        }
                    }
                    return;
                }

                if self.chat_open {
                    if event.state == ElementState::Pressed {
                        match code {
                            KeyCode::Enter => self.submit_chat_input(),
                            KeyCode::Backspace => {
                                self.chat_input.pop();
                            }
                            KeyCode::Escape => {
                                self.set_chat_open(false);
                            }
                            _ => {
                                if let Some(text) = &event.text {
                                    self.append_chat_text(text);
                                }
                            }
                        }
                    }
                    return;
                }

                if event.state == ElementState::Pressed
                    && !event.repeat
                    && (code == KeyCode::F1 || code == KeyCode::Backquote)
                {
                    self.toggle_console();
                    return;
                }

                if self.console_open {
                    if event.state == ElementState::Pressed {
                        match code {
                            KeyCode::Enter => {
                                let command = std::mem::take(&mut self.text_input);
                                self.execute_console_command(&command);
                            }
                            KeyCode::Backspace => {
                                self.text_input.pop();
                            }
                            KeyCode::Escape => {
                                self.set_console_open(false);
                            }
                            _ => {
                                if let Some(text) = &event.text {
                                    self.append_console_text(text);
                                }
                            }
                        }
                    }
                    return;
                }

                if event.state == ElementState::Pressed
                    && !event.repeat
                    && (code == KeyCode::KeyT || code == KeyCode::Enter)
                {
                    self.set_chat_open(true);
                    return;
                }

                match event.state {
                    ElementState::Pressed => {
                        if !event.repeat && code == KeyCode::Escape {
                            self.open_pause_menu();
                            return;
                        }

                        let is_new_press = !self.input.is_pressed(code);
                        self.input.press_key(code);
                        if code == KeyCode::F4 && is_new_press {
                            if matches!(self.play_mode, PlayMode::Creative) {
                                self.set_fly_mode(!self.fly_mode);
                            }
                        }
                        if code == KeyCode::KeyR && !event.repeat {
                            self.sprinting = !self.sprinting;
                        }

                        // C key: toggle creative/survival
                        if code == KeyCode::KeyC
                            && !event.repeat
                            && !self.chat_open
                            && !self.inventory_open
                            && matches!(self.game_mode, Some(GameMode::Singleplayer { .. }))
                        {
                            if self.is_survival_mode() {
                                self.set_play_mode(PlayMode::Creative);
                                self.push_chat_message("System", "Switched to Creative mode");
                            } else {
                                self.set_play_mode(PlayMode::Survival);
                                self.push_chat_message("System", "Switched to Survival mode");
                            }
                        }

                        // Handle hotbar block selection (1-9 keys select inventory slot)
                        let slot_for_key = match code {
                            KeyCode::Digit1 => Some(0),
                            KeyCode::Digit2 => Some(1),
                            KeyCode::Digit3 => Some(2),
                            KeyCode::Digit4 => Some(3),
                            KeyCode::Digit5 => Some(4),
                            KeyCode::Digit6 => Some(5),
                            KeyCode::Digit7 => Some(6),
                            KeyCode::Digit8 => Some(7),
                            KeyCode::Digit9 => Some(8),
                            _ => None,
                        };
                        if let Some(slot) = slot_for_key {
                            self.selected_hotbar_slot = slot;
                            self.refresh_selected_block();
                        }
                    }
                    ElementState::Released => {
                        self.input.release_key(code);
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if matches!(self.app_state, AppState::InGame) && !self.console_open && !self.chat_open {
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => -y as i32,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => {
                            if pos.y > 10.0 {
                                -1
                            } else if pos.y < -10.0 {
                                1
                            } else {
                                0
                            }
                        }
                    };
                    if scroll == 0 {
                        return;
                    }

                    if self.is_creative_inventory_ui_active() {
                        let max_scroll = inventory::creative_max_scroll(self.creative_catalog.len());
                        self.creative_scroll = (self.creative_scroll as i32 + scroll)
                            .clamp(0, max_scroll as i32)
                            as usize;
                    } else if !self.inventory_open {
                        let new_slot = (self.selected_hotbar_slot as i32 + scroll)
                            .rem_euclid(Inventory::HOTBAR_SIZE as i32) as usize;
                        self.selected_hotbar_slot = new_slot;
                        self.refresh_selected_block();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                // Menu click handling
                if matches!(self.app_state, AppState::MainMenu) {
                    if button == MouseButton::Left && state == ElementState::Pressed {
                        if let (Some(window), Some((cx, cy))) = (self.window.as_ref(), self.cursor_position) {
                            let size = window.inner_size();
                            let clicked = crate::ui::main_menu::MainMenuRenderer::hit_test(
                                cx as f32,
                                cy as f32,
                                size.width,
                                size.height,
                            );
                            if let Some(btn) = clicked {
                                match btn {
                                    0 => self.enter_world_select(),
                                    1 => self.start_multiplayer(),
                                    2 => event_loop.exit(),
                                    _ => {}
                                }
                            }
                        }
                    }
                    return;
                }
                if matches!(self.app_state, AppState::WorldSelect) {
                    if button == MouseButton::Left && state == ElementState::Pressed {
                        if let (Some(window), Some((cx, cy))) =
                            (self.window.as_ref(), self.cursor_position)
                        {
                            let size = window.inner_size();
                            let clicked =
                                crate::ui::main_menu::MainMenuRenderer::hit_test_world_select(
                                    cx as f32,
                                    cy as f32,
                                    size.width,
                                    size.height,
                                    self.world_entries.len(),
                                    self.world_selected,
                                    self.world_create_form_open,
                                    self.world_delete_confirmation_open,
                                );
                            if let Some(target) = clicked {
                                self.handle_world_select_hit(target);
                            }
                        }
                    }
                    return;
                }

                if self.is_in_game_menu_open() {
                    if button == MouseButton::Left {
                        match state {
                            ElementState::Pressed => {
                                if let (Some(window), Some((cx, cy))) =
                                    (self.window.as_ref(), self.cursor_position)
                                {
                                    let size = window.inner_size();
                                    match self.in_game_menu {
                                        InGameMenuState::Pause => {
                                            if let Some(hit) =
                                                crate::ui::main_menu::MainMenuRenderer::hit_test_pause(
                                                    cx as f32,
                                                    cy as f32,
                                                    size.width,
                                                    size.height,
                                                )
                                            {
                                                self.pause_menu_selected = match hit {
                                                    PauseMenuHitTarget::Resume => 0,
                                                    PauseMenuHitTarget::Settings => 1,
                                                    PauseMenuHitTarget::SaveAndQuit => 2,
                                                };
                                                self.apply_pause_menu_hit(hit);
                                            }
                                        }
                                        InGameMenuState::Settings => {
                                            if let Some(target) =
                                                crate::ui::main_menu::MainMenuRenderer::hit_test_settings(
                                                    cx as f32,
                                                    cy as f32,
                                                    size.width,
                                                    size.height,
                                                )
                                            {
                                                self.settings_menu_selected =
                                                    crate::ui::main_menu::MainMenuRenderer::settings_target_to_selection(
                                                        target,
                                                    );
                                                if let SettingsHitTarget::Slider(slider, _) = target {
                                                    self.active_settings_slider = Some(slider);
                                                }
                                                self.apply_settings_target(target);
                                            }
                                        }
                                        InGameMenuState::None => {}
                                    }
                                }
                            }
                            ElementState::Released => {
                                self.active_settings_slider = None;
                            }
                        }
                    }
                    return;
                }

                if self.inventory_open {
                    if self.is_creative_inventory_ui_active() {
                        if matches!(
                            (button, state),
                            (MouseButton::Left, ElementState::Pressed)
                                | (MouseButton::Right, ElementState::Pressed)
                        ) {
                            let is_right = button == MouseButton::Right;
                            if let (Some(window), Some((cx, cy))) =
                                (self.window.as_ref(), self.cursor_position)
                            {
                                let size = window.inner_size();
                                let cursor_x = cx as f32;
                                let cursor_y = cy as f32;
                                if let Some(hit_target) = inventory::hit_test_creative(
                                    cursor_x,
                                    cursor_y,
                                    size.width as f32,
                                    size.height as f32,
                                    self.settings.gui_scale,
                                    self.creative_catalog.len(),
                                    self.creative_scroll,
                                ) {
                                    match hit_target {
                                        inventory::CreativeHitTarget::CatalogSlot(idx) => {
                                            if let Some(&item) = self.creative_catalog.get(idx) {
                                                let mut picked =
                                                    ItemStack::new(item, max_stack_for_item(item));
                                                Self::initialize_tool_durability(&mut picked);
                                                match self.cursor_stack.as_mut() {
                                                    Some(cursor) if cursor.item == item => {
                                                        cursor.count = picked.count;
                                                        cursor.durability = picked.durability;
                                                    }
                                                    _ => {
                                                        self.cursor_stack = Some(picked);
                                                    }
                                                }
                                            }
                                        }
                                        inventory::CreativeHitTarget::HotbarSlot(slot) => {
                                            if is_right {
                                                self.handle_inventory_slot_right_click(slot);
                                            } else {
                                                self.handle_inventory_slot_click(slot);
                                            }
                                        }
                                        inventory::CreativeHitTarget::ClearButton => {
                                            self.inventory.slots.fill(None);
                                            self.armor_slots = [None; 4];
                                            self.cursor_stack = None;
                                            self.refresh_selected_block();
                                        }
                                        inventory::CreativeHitTarget::SearchBar => {}
                                    }
                                } else if !inventory::is_creative_panel_hit(
                                    cursor_x,
                                    cursor_y,
                                    size.width as f32,
                                    size.height as f32,
                                    self.settings.gui_scale,
                                ) {
                                    self.set_inventory_open(false);
                                }
                            }
                        }
                    } else {
                        match (button, state) {
                            (MouseButton::Left, ElementState::Released) => {
                                if self.drag_distributing {
                                    self.finish_inventory_drag_distribution();
                                }
                            }
                            (MouseButton::Left, ElementState::Pressed)
                            | (MouseButton::Right, ElementState::Pressed) => {
                                let is_right = button == MouseButton::Right;
                                if let (Some(window), Some((cx, cy))) =
                                    (self.window.as_ref(), self.cursor_position)
                                {
                                    let size = window.inner_size();
                                    let cursor_x = cx as f32;
                                    let cursor_y = cy as f32;
                                    if let Some(armor_slot) = self.inventory_armor_slot_hit_test(
                                        cursor_x,
                                        cursor_y,
                                        size.width,
                                        size.height,
                                    ) {
                                        self.handle_armor_slot_click(armor_slot);
                                    } else if let Some(hit_target) = inventory::hit_test_open_target(
                                        cursor_x,
                                        cursor_y,
                                        size.width,
                                        size.height,
                                        self.settings.gui_scale,
                                        self.crafting_ui_mode,
                                        self.open_chest.is_some(),
                                    ) {
                                        match hit_target {
                                            inventory::OpenInventoryHitTarget::InventorySlot(slot) => {
                                                if is_right {
                                                    self.handle_inventory_slot_right_click(slot);
                                                } else {
                                                    // Track click count for triple-click
                                                    let now = Instant::now();
                                                    if self.last_inv_click_slot == Some(slot)
                                                        && now.duration_since(self.last_inv_click_time).as_millis() < 400
                                                    {
                                                        self.inv_click_count += 1;
                                                    } else {
                                                        self.inv_click_count = 1;
                                                    }
                                                    self.last_inv_click_slot = Some(slot);
                                                    self.last_inv_click_time = now;

                                                    if self.inv_click_count >= 3 && self.cursor_stack.is_none() {
                                                        self.gather_matching_items_to_slot(slot);
                                                        self.inv_click_count = 0;
                                                    } else if !self
                                                        .try_begin_inventory_drag_distribution(slot)
                                                    {
                                                        self.handle_inventory_slot_click(slot);
                                                    }
                                                }
                                            }
                                            inventory::OpenInventoryHitTarget::ChestSlot(slot) => {
                                                if is_right {
                                                    self.handle_chest_slot_right_click(slot);
                                                } else {
                                                    self.handle_chest_slot_click(slot);
                                                }
                                            }
                                            inventory::OpenInventoryHitTarget::CraftingInput(input_idx) => {
                                                if is_right {
                                                    self.handle_crafting_input_slot_right_click(input_idx);
                                                } else {
                                                    self.handle_crafting_input_slot_click(input_idx);
                                                }
                                            }
                                            inventory::OpenInventoryHitTarget::CraftingOutput => {
                                                self.handle_crafting_output_click();
                                            }
                                        }
                                    } else if !inventory::is_open_inventory_panel_hit(
                                        cursor_x,
                                        cursor_y,
                                        size.width,
                                        size.height,
                                        self.settings.gui_scale,
                                        self.crafting_ui_mode,
                                        self.open_chest.is_some(),
                                    ) {
                                        self.set_inventory_open(false);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    return;
                }

                if self.cursor_grabbed {
                    if self.console_open || self.chat_open {
                        return;
                    }
                    match (button, state) {
                        (MouseButton::Left, ElementState::Pressed) => {
                            self.input.left_click = true;
                        }
                        (MouseButton::Left, ElementState::Released) => {
                            self.input.left_click = false;
                            self.breaking_block = None;
                            self.break_progress = 0.0;
                        }
                        (MouseButton::Right, ElementState::Pressed) => {
                            self.input.right_click = true;
                        }
                        _ => {}
                    }
                } else if state == ElementState::Pressed {
                    // Re-grab cursor on any click when not grabbed
                    self.set_cursor_grab(true);
                }
            }
            WindowEvent::Focused(false) => {
                self.input.left_click = false;
                self.breaking_block = None;
                self.break_progress = 0.0;
                self.sprinting = false;
                self.active_settings_slider = None;
                self.set_inventory_open(false);
                self.set_console_open(false);
                self.set_chat_open(false);
                self.set_cursor_grab(false);
            }
            WindowEvent::Resized(size) => {
                info!("Window resized to {}x{}", size.width, size.height);
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                if size.height > 0 {
                    self.camera.aspect = size.width as f32 / size.height as f32;
                }
            }
            WindowEvent::RedrawRequested => {
                self.update_and_render(event_loop);
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if !self.cursor_grabbed
            || self.console_open
            || self.chat_open
            || self.inventory_open
            || self.is_in_game_menu_open()
        {
            return;
        }

        if let DeviceEvent::MouseMotion { delta } = event {
            self.input
                .add_mouse_delta(Vec2::new(delta.0 as f32, delta.1 as f32));
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

fn parse_seed_input(input: &str) -> u64 {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return random_seed();
    }

    if let Ok(number) = trimmed.parse::<i64>() {
        return number as u64;
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    trimmed.hash(&mut hasher);
    hasher.finish()
}

fn random_seed() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(DEFAULT_WORLD_SEED);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    now.hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    hasher.finish()
}

fn sanitize_world_folder_name(name: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_separator = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if matches!(ch, ' ' | '-' | '_') && !last_was_separator {
            sanitized.push('_');
            last_was_separator = true;
        }
    }

    let trimmed = sanitized.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "world".to_string()
    } else {
        trimmed
    }
}

fn unique_world_folder_name(worlds_dir: &Path, base_name: &str) -> String {
    let mut candidate = base_name.to_string();
    let mut suffix = 2u32;
    while worlds_dir.join(&candidate).exists() {
        candidate = format!("{base_name}_{suffix}");
        suffix = suffix.saturating_add(1);
    }
    candidate
}

fn format_size_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MIB {
        format!("{:.1} KB", bytes as f64 / KIB)
    } else if (bytes as f64) < GIB {
        format!("{:.1} MB", bytes as f64 / MIB)
    } else {
        format!("{:.1} GB", bytes as f64 / GIB)
    }
}

fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "UNKNOWN".to_string();
    };
    let Ok(duration) = time.duration_since(UNIX_EPOCH) else {
        return "UNKNOWN".to_string();
    };

    let total_secs = duration.as_secs() as i64;
    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hours = secs_of_day / 3_600;
    let minutes = (secs_of_day % 3_600) / 60;

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_param = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_param + 2) / 5 + 1;
    let month = month_param + if month_param < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }

    (year as i32, month as u32, day as u32)
}

fn block_at(world_pos: IVec3, chunks: &HashMap<ChunkPos, ChunkData>) -> Option<BlockId> {
    let (chunk_pos, local_pos) = world_to_chunk(world_pos);
    let chunk = chunks.get(&chunk_pos)?;
    Some(chunk.blocks[local_to_index(local_pos)])
}

fn set_block_at(world_pos: IVec3, block: BlockId, chunks: &mut HashMap<ChunkPos, ChunkData>) {
    let (chunk_pos, local_pos) = world_to_chunk(world_pos);
    if let Some(chunk) = chunks.get_mut(&chunk_pos) {
        chunk.blocks[local_to_index(local_pos)] = block;
    }
}

fn block_particle_color(block: BlockId, registry: &BlockRegistry) -> [f32; 3] {
    let block_name = registry.get_properties(block).name.as_str();
    if block_name == "still_water" || block_name.starts_with("flowing_water_") {
        return [0.3, 0.48, 0.8];
    }
    if block_name == "lava_source" || block_name.starts_with("flowing_lava_") {
        return [0.95, 0.36, 0.08];
    }

    match block_name {
        "air" => [0.75, 0.75, 0.75],
        "bedstone" => [0.18, 0.18, 0.2],
        "granite" => [0.55, 0.56, 0.59],
        "loam" => [0.44, 0.3, 0.2],
        "verdant_turf" => [0.35, 0.62, 0.27],
        "dune_sand" | "sand" => [0.84, 0.77, 0.55],
        "timber_log" => [0.47, 0.32, 0.2],
        "hewn_plank" => [0.66, 0.49, 0.3],
        "canopy_leaves" | "tall_grass" | "wildflower" | "sapling" | "sugar_cane" => {
            [0.3, 0.62, 0.26]
        }
        "rubblestone" | "mossy_rubble" | "tuff" => [0.5, 0.52, 0.54],
        "iron_vein" => [0.64, 0.58, 0.5],
        "crystal_pane" => [0.74, 0.87, 0.96],
        "kiln_brick" => [0.65, 0.34, 0.28],
        "gravel_bed" => [0.59, 0.57, 0.53],
        "snowcap" => [0.93, 0.95, 0.98],
        "coal_vein" => [0.2, 0.2, 0.22],
        "copper_vein" => [0.76, 0.47, 0.3],
        "gold_vein" => [0.87, 0.77, 0.31],
        "diamond_vein" => [0.42, 0.83, 0.88],
        "clay_deposit" | "hardened_clay" => [0.69, 0.58, 0.5],
        "ice" => [0.73, 0.86, 0.96],
        "packed_ice" => [0.66, 0.79, 0.92],
        "blue_ice" => [0.42, 0.66, 0.96],
        "magma_block" => [0.9, 0.44, 0.2],
        "obsidian" => [0.16, 0.1, 0.2],
        "torch" => [0.98, 0.76, 0.22],
        "wooden_door" | "fence" => [0.66, 0.49, 0.3],
        "crafting_table" => [0.61, 0.45, 0.27],
        "furnace" => [0.43, 0.44, 0.46],
        "chest" => [0.66, 0.49, 0.3],
        name if name.starts_with("door_") || name.starts_with("ladder") => [0.66, 0.49, 0.3],
        "netherite_block" | "polished_blackstone" => [0.3, 0.3, 0.34],
        "dripstone_block" => [0.57, 0.45, 0.32],
        _ => [0.62, 0.62, 0.62],
    }
}

fn item_particle_color(item: ItemId, registry: &BlockRegistry) -> [f32; 3] {
    if let Some(block) = item.as_block_id() {
        return block_particle_color(block, registry);
    }

    match item {
        ItemId::STICK => [0.76, 0.61, 0.39],
        ItemId::WOODEN_PICKAXE => [0.7, 0.51, 0.31],
        ItemId::WOODEN_SWORD => [0.74, 0.56, 0.33],
        _ => [0.62, 0.62, 0.62],
    }
}

fn is_block_water(
    x: i32,
    y: i32,
    z: i32,
    chunks: &HashMap<ChunkPos, ChunkData>,
) -> bool {
    let (chunk_pos, local_pos) = world_to_chunk(IVec3::new(x, y, z));
    let Some(chunk) = chunks.get(&chunk_pos) else {
        return false;
    };
    let idx = local_to_index(local_pos);
    is_water_block(chunk.blocks[idx])
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum HorizontalFacing {
    North,
    East,
    South,
    West,
}

fn is_block_solid(
    x: i32,
    y: i32,
    z: i32,
    chunks: &HashMap<ChunkPos, ChunkData>,
    registry: &BlockRegistry,
) -> bool {
    let world_pos = IVec3::new(x, y, z);
    let Some(block) = block_at(world_pos, chunks) else {
        return false;
    };
    block_collision_bounds(world_pos, block, registry).is_some()
}

fn is_block_break_target(block: BlockId, registry: &BlockRegistry) -> bool {
    if block == BlockId::AIR {
        return false;
    }
    if is_door_block(block)
        || is_ladder_block(block)
        || is_fence_block(block)
        || is_cross_plant_block(block)
    {
        return true;
    }
    let props = registry.get_properties(block);
    props.solid || props.light_level > 0
}

fn player_aabb_overlaps_block(player_pos: Vec3, block_pos: IVec3) -> bool {
    let (player_min, player_max) = player_aabb(player_pos);
    let block_min = [block_pos.x as f32, block_pos.y as f32, block_pos.z as f32];
    let block_max = [block_min[0] + 1.0, block_min[1] + 1.0, block_min[2] + 1.0];
    aabb_intersects(player_min, player_max, block_min, block_max)
}

fn collides_with_terrain(
    pos: Vec3,
    chunks: &HashMap<ChunkPos, ChunkData>,
    registry: &BlockRegistry,
) -> bool {
    let (player_min, player_max) = player_aabb(pos);
    let min_x = player_min[0].floor() as i32;
    let max_x = (player_max[0] - 1e-4).floor() as i32;
    let min_y = (player_min[1] - 1.0).floor() as i32;
    let max_y = (player_max[1] - 1e-4).floor() as i32;
    let min_z = player_min[2].floor() as i32;
    let max_z = (player_max[2] - 1e-4).floor() as i32;

    for by in min_y..=max_y {
        for bz in min_z..=max_z {
            for bx in min_x..=max_x {
                let world_pos = IVec3::new(bx, by, bz);
                let Some(block) = block_at(world_pos, chunks) else {
                    continue;
                };
                let Some((block_min, block_max)) =
                    block_collision_bounds(world_pos, block, registry)
                else {
                    continue;
                };
                if aabb_intersects(player_min, player_max, block_min, block_max) {
                    return true;
                }
            }
        }
    }
    false
}

fn find_ground_snap(
    pos: Vec3,
    chunks: &HashMap<ChunkPos, ChunkData>,
    registry: &BlockRegistry,
) -> f32 {
    let (player_min, player_max) = player_aabb(pos);
    let min_x = player_min[0].floor() as i32;
    let max_x = (player_max[0] - 1e-4).floor() as i32;
    let min_y = (player_min[1] - 1.0).floor() as i32;
    let max_y = (player_max[1] - 1e-4).floor() as i32;
    let min_z = player_min[2].floor() as i32;
    let max_z = (player_max[2] - 1e-4).floor() as i32;

    let mut highest_top = pos.y;
    for by in min_y..=max_y {
        for bz in min_z..=max_z {
            for bx in min_x..=max_x {
                let world_pos = IVec3::new(bx, by, bz);
                let Some(block) = block_at(world_pos, chunks) else {
                    continue;
                };
                let Some((block_min, block_max)) =
                    block_collision_bounds(world_pos, block, registry)
                else {
                    continue;
                };
                if aabb_intersects(player_min, player_max, block_min, block_max) {
                    highest_top = highest_top.max(block_max[1]);
                }
            }
        }
    }
    highest_top
}

fn is_player_touching_ladder(pos: Vec3, chunks: &HashMap<ChunkPos, ChunkData>) -> bool {
    let (player_min, player_max) = player_aabb(pos);
    let min_x = player_min[0].floor() as i32;
    let max_x = (player_max[0] - 1e-4).floor() as i32;
    let min_y = player_min[1].floor() as i32;
    let max_y = (player_max[1] - 1e-4).floor() as i32;
    let min_z = player_min[2].floor() as i32;
    let max_z = (player_max[2] - 1e-4).floor() as i32;

    for by in min_y..=max_y {
        for bz in min_z..=max_z {
            for bx in min_x..=max_x {
                let world_pos = IVec3::new(bx, by, bz);
                let Some(block) = block_at(world_pos, chunks) else {
                    continue;
                };
                if !is_ladder_block(block) {
                    continue;
                }
                let block_min = [bx as f32, by as f32, bz as f32];
                let block_max = [bx as f32 + 1.0, by as f32 + 1.0, bz as f32 + 1.0];
                if aabb_intersects(player_min, player_max, block_min, block_max) {
                    return true;
                }
            }
        }
    }

    false
}

fn is_player_touching_lava(pos: Vec3, chunks: &HashMap<ChunkPos, ChunkData>) -> bool {
    let (player_min, player_max) = player_aabb(pos);
    let min_x = player_min[0].floor() as i32;
    let max_x = (player_max[0] - 1e-4).floor() as i32;
    let min_y = player_min[1].floor() as i32;
    let max_y = (player_max[1] - 1e-4).floor() as i32;
    let min_z = player_min[2].floor() as i32;
    let max_z = (player_max[2] - 1e-4).floor() as i32;

    for by in min_y..=max_y {
        for bz in min_z..=max_z {
            for bx in min_x..=max_x {
                let world_pos = IVec3::new(bx, by, bz);
                let Some(block) = block_at(world_pos, chunks) else {
                    continue;
                };
                if !is_lava_block(block) {
                    continue;
                }

                let block_min = [bx as f32, by as f32, bz as f32];
                let block_max = [bx as f32 + 1.0, by as f32 + 1.0, bz as f32 + 1.0];
                if aabb_intersects(player_min, player_max, block_min, block_max) {
                    return true;
                }
            }
        }
    }

    false
}

fn try_set_world_block(
    chunks: &mut HashMap<ChunkPos, ChunkData>,
    world_pos: IVec3,
    new_block: BlockId,
) -> Option<(ChunkPos, LocalPos, BlockId, BlockId)> {
    let (chunk_pos, local_pos) = world_to_chunk(world_pos);
    let chunk = chunks.get_mut(&chunk_pos)?;
    let idx = local_to_index(local_pos);
    let previous = chunk.blocks[idx];
    if previous == new_block {
        return None;
    }
    chunk.blocks[idx] = new_block;
    Some((chunk_pos, local_pos, previous, new_block))
}

fn normalize_place_block(block: BlockId) -> BlockId {
    if is_door_block(block) || block == BlockId::WOODEN_DOOR {
        return BlockId::WOODEN_DOOR;
    }
    if is_ladder_block(block) {
        return BlockId::LADDER;
    }
    if is_sign(block) {
        return BlockId::SIGN_NORTH;
    }
    if block == BlockId(226) { // WHEAT_SEEDS -> wheat_stage_0
        return BlockId::WHEAT_STAGE_0;
    }
    block
}

fn drop_block_for_break(block: BlockId) -> Option<BlockId> {
    if block == BlockId::AIR {
        return None;
    }
    if is_door_block(block) || block == BlockId::WOODEN_DOOR {
        return Some(BlockId::WOODEN_DOOR);
    }
    if is_ladder_block(block) {
        return Some(BlockId::LADDER);
    }
    Some(block)
}

fn is_cross_plant_block(block: BlockId) -> bool {
    block == BlockId::TALL_GRASS
        || block == BlockId::WILDFLOWER
        || block == BlockId::SAPLING
        || block == BlockId::SUGAR_CANE
}

fn can_sapling_stay(pos: IVec3, chunks: &HashMap<ChunkPos, ChunkData>) -> bool {
    let below = pos - IVec3::Y;
    matches!(
        block_at(below, chunks),
        Some(BlockId::VERDANT_TURF | BlockId::LOAM)
    )
}

fn can_place_sugar_cane(
    pos: IVec3,
    chunks: &HashMap<ChunkPos, ChunkData>,
    registry: &BlockRegistry,
) -> bool {
    let below = pos - IVec3::Y;
    let Some(below_block) = block_at(below, chunks) else {
        return false;
    };

    if below_block == BlockId::SUGAR_CANE {
        let mut base = below;
        while block_at(base - IVec3::Y, chunks) == Some(BlockId::SUGAR_CANE) {
            base -= IVec3::Y;
        }
        let height = sugar_cane_height(base, chunks);
        let expected_pos = base + IVec3::new(0, height, 0);
        return pos == expected_pos
            && height < SUGAR_CANE_MAX_HEIGHT
            && has_horizontal_water_neighbor(base, chunks);
    }

    registry.get_properties(below_block).solid && has_horizontal_water_neighbor(pos, chunks)
}

fn is_sugar_cane_base(pos: IVec3, chunks: &HashMap<ChunkPos, ChunkData>) -> bool {
    block_at(pos, chunks) == Some(BlockId::SUGAR_CANE)
        && block_at(pos - IVec3::Y, chunks) != Some(BlockId::SUGAR_CANE)
}

fn sugar_cane_height(base: IVec3, chunks: &HashMap<ChunkPos, ChunkData>) -> i32 {
    let mut height = 0;
    loop {
        let pos = base + IVec3::new(0, height, 0);
        if block_at(pos, chunks) == Some(BlockId::SUGAR_CANE) {
            height += 1;
        } else {
            break;
        }
    }
    height
}

fn has_horizontal_water_neighbor(pos: IVec3, chunks: &HashMap<ChunkPos, ChunkData>) -> bool {
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let neighbor = pos + IVec3::new(dx, 0, dz);
        if let Some(block) = block_at(neighbor, chunks) {
            if is_water_block(block) {
                return true;
            }
        }
    }
    false
}

fn block_supports_attachment(
    world_pos: IVec3,
    block: BlockId,
    _chunks: &HashMap<ChunkPos, ChunkData>,
    registry: &BlockRegistry,
) -> bool {
    block_collision_bounds(world_pos, block, registry).is_some()
}

fn facing_from_yaw(yaw: f32) -> HorizontalFacing {
    let forward_x = yaw.cos();
    let forward_z = yaw.sin();
    if forward_x.abs() >= forward_z.abs() {
        if forward_x >= 0.0 {
            HorizontalFacing::East
        } else {
            HorizontalFacing::West
        }
    } else if forward_z >= 0.0 {
        HorizontalFacing::South
    } else {
        HorizontalFacing::North
    }
}

fn sign_block_for_yaw(yaw: f32) -> BlockId {
    let yaw_degrees = yaw.to_degrees().rem_euclid(360.0);
    if (45.0..135.0).contains(&yaw_degrees) {
        BlockId::SIGN_EAST
    } else if (135.0..225.0).contains(&yaw_degrees) {
        BlockId::SIGN_NORTH
    } else if (225.0..315.0).contains(&yaw_degrees) {
        BlockId::SIGN_WEST
    } else {
        BlockId::SIGN_SOUTH
    }
}

fn facing_from_placement_face(face: Face) -> Option<HorizontalFacing> {
    match face {
        Face::NegZ => Some(HorizontalFacing::North),
        Face::PosX => Some(HorizontalFacing::East),
        Face::PosZ => Some(HorizontalFacing::South),
        Face::NegX => Some(HorizontalFacing::West),
        Face::PosY | Face::NegY => None,
    }
}

fn ladder_block_for_facing(facing: HorizontalFacing) -> BlockId {
    match facing {
        HorizontalFacing::North => BlockId::LADDER,
        HorizontalFacing::East => BlockId::LADDER_EAST,
        HorizontalFacing::South => BlockId::LADDER_SOUTH,
        HorizontalFacing::West => BlockId::LADDER_WEST,
    }
}

fn door_block_for_state(facing: HorizontalFacing, is_open: bool, is_upper: bool) -> BlockId {
    match (facing, is_open, is_upper) {
        (HorizontalFacing::North, false, false) => BlockId::DOOR_LOWER,
        (HorizontalFacing::North, false, true) => BlockId::DOOR_UPPER,
        (HorizontalFacing::East, false, false) => BlockId::DOOR_LOWER_EAST,
        (HorizontalFacing::East, false, true) => BlockId::DOOR_UPPER_EAST,
        (HorizontalFacing::South, false, false) => BlockId::DOOR_LOWER_SOUTH,
        (HorizontalFacing::South, false, true) => BlockId::DOOR_UPPER_SOUTH,
        (HorizontalFacing::West, false, false) => BlockId::DOOR_LOWER_WEST,
        (HorizontalFacing::West, false, true) => BlockId::DOOR_UPPER_WEST,
        (HorizontalFacing::North, true, false) => BlockId::DOOR_LOWER_OPEN,
        (HorizontalFacing::North, true, true) => BlockId::DOOR_UPPER_OPEN,
        (HorizontalFacing::East, true, false) => BlockId::DOOR_LOWER_OPEN_EAST,
        (HorizontalFacing::East, true, true) => BlockId::DOOR_UPPER_OPEN_EAST,
        (HorizontalFacing::South, true, false) => BlockId::DOOR_LOWER_OPEN_SOUTH,
        (HorizontalFacing::South, true, true) => BlockId::DOOR_UPPER_OPEN_SOUTH,
        (HorizontalFacing::West, true, false) => BlockId::DOOR_LOWER_OPEN_WEST,
        (HorizontalFacing::West, true, true) => BlockId::DOOR_UPPER_OPEN_WEST,
    }
}

fn door_state_for_block(block: BlockId) -> Option<(HorizontalFacing, bool, bool)> {
    match block {
        BlockId::DOOR_LOWER => Some((HorizontalFacing::North, false, false)),
        BlockId::DOOR_UPPER => Some((HorizontalFacing::North, false, true)),
        BlockId::DOOR_LOWER_EAST => Some((HorizontalFacing::East, false, false)),
        BlockId::DOOR_UPPER_EAST => Some((HorizontalFacing::East, false, true)),
        BlockId::DOOR_LOWER_SOUTH => Some((HorizontalFacing::South, false, false)),
        BlockId::DOOR_UPPER_SOUTH => Some((HorizontalFacing::South, false, true)),
        BlockId::DOOR_LOWER_WEST => Some((HorizontalFacing::West, false, false)),
        BlockId::DOOR_UPPER_WEST => Some((HorizontalFacing::West, false, true)),
        BlockId::DOOR_LOWER_OPEN => Some((HorizontalFacing::North, true, false)),
        BlockId::DOOR_UPPER_OPEN => Some((HorizontalFacing::North, true, true)),
        BlockId::DOOR_LOWER_OPEN_EAST => Some((HorizontalFacing::East, true, false)),
        BlockId::DOOR_UPPER_OPEN_EAST => Some((HorizontalFacing::East, true, true)),
        BlockId::DOOR_LOWER_OPEN_SOUTH => Some((HorizontalFacing::South, true, false)),
        BlockId::DOOR_UPPER_OPEN_SOUTH => Some((HorizontalFacing::South, true, true)),
        BlockId::DOOR_LOWER_OPEN_WEST => Some((HorizontalFacing::West, true, false)),
        BlockId::DOOR_UPPER_OPEN_WEST => Some((HorizontalFacing::West, true, true)),
        _ => None,
    }
}

fn door_pair_positions_for_interaction(
    clicked_pos: IVec3,
    clicked_block: BlockId,
    chunks: &HashMap<ChunkPos, ChunkData>,
) -> Option<(IVec3, IVec3)> {
    let (lower_pos, upper_pos, _, _) = door_interaction_state(clicked_pos, clicked_block, chunks)?;
    Some((lower_pos, upper_pos))
}

fn door_interaction_state(
    clicked_pos: IVec3,
    clicked_block: BlockId,
    chunks: &HashMap<ChunkPos, ChunkData>,
) -> Option<(IVec3, IVec3, HorizontalFacing, bool)> {
    let (_, _, is_upper) = door_state_for_block(clicked_block)?;
    let lower_pos = if is_upper { clicked_pos - IVec3::Y } else { clicked_pos };
    let upper_pos = lower_pos + IVec3::Y;
    let lower_block = block_at(lower_pos, chunks)?;
    let upper_block = block_at(upper_pos, chunks)?;
    let (lower_facing, lower_open, lower_is_upper) = door_state_for_block(lower_block)?;
    let (upper_facing, upper_open, upper_is_upper) = door_state_for_block(upper_block)?;
    if lower_is_upper || !upper_is_upper {
        return None;
    }
    if lower_facing != upper_facing || lower_open != upper_open {
        return None;
    }
    Some((lower_pos, upper_pos, lower_facing, lower_open))
}

fn block_collision_bounds(
    world_pos: IVec3,
    block: BlockId,
    registry: &BlockRegistry,
) -> Option<([f32; 3], [f32; 3])> {
    if block == BlockId::AIR || block == BlockId::WOODEN_DOOR || is_ladder_block(block) {
        return None;
    }

    let wx = world_pos.x as f32;
    let wy = world_pos.y as f32;
    let wz = world_pos.z as f32;

    if is_fence_block(block) {
        return Some(([wx, wy, wz], [wx + 1.0, wy + 1.5, wz + 1.0]));
    }

    if let Some((facing, is_open, _)) = door_state_for_block(block) {
        if is_open {
            return None;
        }
        let (local_min, local_max) = door_local_bounds(facing, is_open);
        return Some((
            [wx + local_min[0], wy + local_min[1], wz + local_min[2]],
            [wx + local_max[0], wy + local_max[1], wz + local_max[2]],
        ));
    }

    let props = registry.get_properties(block);
    if props.solid {
        Some(([wx, wy, wz], [wx + 1.0, wy + 1.0, wz + 1.0]))
    } else {
        None
    }
}

fn door_local_bounds(facing: HorizontalFacing, is_open: bool) -> ([f32; 3], [f32; 3]) {
    match (facing, is_open) {
        (HorizontalFacing::North, false) => ([0.0, 0.0, 1.0 - DOOR_THICKNESS], [1.0, 1.0, 1.0]),
        (HorizontalFacing::East, false) => ([0.0, 0.0, 0.0], [DOOR_THICKNESS, 1.0, 1.0]),
        (HorizontalFacing::South, false) => ([0.0, 0.0, 0.0], [1.0, 1.0, DOOR_THICKNESS]),
        (HorizontalFacing::West, false) => ([1.0 - DOOR_THICKNESS, 0.0, 0.0], [1.0, 1.0, 1.0]),
        (HorizontalFacing::North, true) => ([1.0 - DOOR_THICKNESS, 0.0, 0.0], [1.0, 1.0, 1.0]),
        (HorizontalFacing::East, true) => ([0.0, 0.0, 0.0], [1.0, 1.0, DOOR_THICKNESS]),
        (HorizontalFacing::South, true) => ([0.0, 0.0, 0.0], [DOOR_THICKNESS, 1.0, 1.0]),
        (HorizontalFacing::West, true) => ([0.0, 0.0, 1.0 - DOOR_THICKNESS], [1.0, 1.0, 1.0]),
    }
}

fn player_aabb(pos: Vec3) -> ([f32; 3], [f32; 3]) {
    (
        [pos.x - PLAYER_HALF_W, pos.y, pos.z - PLAYER_HALF_W],
        [pos.x + PLAYER_HALF_W, pos.y + PLAYER_HEIGHT, pos.z + PLAYER_HALF_W],
    )
}

fn aabb_intersects(
    lhs_min: [f32; 3],
    lhs_max: [f32; 3],
    rhs_min: [f32; 3],
    rhs_max: [f32; 3],
) -> bool {
    lhs_min[0] < rhs_max[0]
        && lhs_max[0] > rhs_min[0]
        && lhs_min[1] < rhs_max[1]
        && lhs_max[1] > rhs_min[1]
        && lhs_min[2] < rhs_max[2]
        && lhs_max[2] > rhs_min[2]
}

fn is_door_block(block: BlockId) -> bool {
    (BlockId::DOOR_LOWER.0..=BlockId::DOOR_UPPER_OPEN_WEST.0).contains(&block.0)
}

fn is_ladder_block(block: BlockId) -> bool {
    (BlockId::LADDER.0..=BlockId::LADDER_WEST.0).contains(&block.0)
}

fn is_fence_block(block: BlockId) -> bool {
    block == BlockId::FENCE
}

fn populate_creative_hotbar(inventory: &mut Inventory) {
    for (i, &block) in CREATIVE_HOTBAR_BLOCKS.iter().enumerate() {
        inventory.set(i, Some(ItemStack::new(ItemId::from(block), 64)));
    }
}

fn swap_or_merge_slot_with_cursor(
    slot: &mut Option<ItemStack>,
    cursor_stack: &mut Option<ItemStack>,
) {
    if cursor_stack.is_none() {
        *cursor_stack = slot.take();
        return;
    }

    let mut held = cursor_stack.take().expect("cursor stack was checked");
    match slot.as_mut() {
        None => {
            *slot = Some(held);
        }
        Some(target) if target.can_merge(&held) => {
            target.merge(&mut held);
            if held.count > 0 {
                *cursor_stack = Some(held);
            }
        }
        Some(target) => {
            let swapped = *target;
            *target = held;
            *cursor_stack = Some(swapped);
        }
    }
}

fn right_click_slot(
    slot: &mut Option<ItemStack>,
    cursor_stack: &mut Option<ItemStack>,
) {
    match cursor_stack.as_mut() {
        None => {
            // Right-click with empty cursor: pick up half the stack
            let Some(stack) = slot.as_mut() else {
                return;
            };
            let half = (stack.count + 1) / 2; // round up for cursor
            let remain = stack.count - half;
            *cursor_stack = Some(ItemStack {
                item: stack.item,
                count: half,
                durability: stack.durability,
            });
            if remain == 0 {
                *slot = None;
            } else {
                stack.count = remain;
            }
        }
        Some(held) => {
            // Right-click with cursor holding items: place one item
            match slot.as_mut() {
                None => {
                    *slot = Some(ItemStack {
                        item: held.item,
                        count: 1,
                        durability: held.durability,
                    });
                    held.count -= 1;
                    if held.count == 0 {
                        *cursor_stack = None;
                    }
                }
                Some(target) if target.can_merge(held) => {
                    target.count += 1;
                    held.count -= 1;
                    if held.count == 0 {
                        *cursor_stack = None;
                    }
                }
                _ => {
                    // Different item or full stack: swap (same as left-click)
                    swap_or_merge_slot_with_cursor(slot, cursor_stack);
                }
            }
        }
    }
}

fn selected_block_from_inventory(inventory: &Inventory, slot: usize) -> BlockId {
    inventory
        .hotbar_slot(slot)
        .filter(|stack| stack.count > 0)
        .and_then(|stack| stack.item.as_block_id())
        .unwrap_or(BlockId::AIR)
}

fn selected_item_from_inventory(inventory: &Inventory, slot: usize) -> ItemId {
    inventory
        .hotbar_slot(slot)
        .filter(|stack| stack.count > 0)
        .map(|stack| stack.item)
        .unwrap_or(ItemId::from(BlockId::AIR))
}

fn effective_break_time(block: BlockId, held_item: ItemId, registry: &BlockRegistry) -> f32 {
    let hardness = registry.get_properties(block).hardness;
    if hardness <= 0.0 {
        return 0.0;
    }

    let base_time = hardness * 0.6;
    let speed_mult = tool_properties(held_item)
        .and_then(|(kind, tier)| {
            tool_is_effective_on_block(kind, block).then_some(tool_speed_multiplier(tier))
        })
        .unwrap_or(1.0);

    (base_time / speed_mult).clamp(0.05, 10.0)
}

fn tool_is_effective_on_block(kind: ToolKind, block: BlockId) -> bool {
    match kind {
        ToolKind::Pickaxe => {
            block == BlockId::RUBBLESTONE
                || block == BlockId::OBSIDIAN
                || matches!(block.0, 2 | 11 | 16 | 17 | 18 | 19 | 23)
        }
        ToolKind::Axe => {
            block == BlockId::TIMBER_LOG
                || block.0 == 7
                || block == BlockId::CRAFTING_TABLE
                || block == BlockId::CHEST
                || is_fence_block(block)
                || is_ladder_block(block)
                || is_trapdoor_block(block)
                || block == BlockId::WOODEN_DOOR
                || is_door_block(block)
                || block == BlockId::WOODEN_SLAB_BOTTOM
                || block == BlockId::WOODEN_SLAB_TOP
        }
        ToolKind::Shovel => {
            matches!(block, BlockId::LOAM | BlockId::VERDANT_TURF | BlockId::FARMLAND)
                || matches!(block.0, 5 | 14 | 15 | 22)
        }
        ToolKind::Hoe => is_wheat_block(block) || block == BlockId::CANOPY_LEAVES,
        ToolKind::Sword => false,
    }
}

fn trapdoor_toggle(block: BlockId) -> BlockId {
    match block {
        b if b == BlockId::TRAPDOOR_CLOSED => BlockId::TRAPDOOR_OPEN,
        b if b == BlockId::TRAPDOOR_OPEN => BlockId::TRAPDOOR_CLOSED,
        b if b == BlockId::TRAPDOOR_CLOSED_EAST => BlockId::TRAPDOOR_OPEN_EAST,
        b if b == BlockId::TRAPDOOR_OPEN_EAST => BlockId::TRAPDOOR_CLOSED_EAST,
        b if b == BlockId::TRAPDOOR_CLOSED_SOUTH => BlockId::TRAPDOOR_OPEN_SOUTH,
        b if b == BlockId::TRAPDOOR_OPEN_SOUTH => BlockId::TRAPDOOR_CLOSED_SOUTH,
        b if b == BlockId::TRAPDOOR_CLOSED_WEST => BlockId::TRAPDOOR_OPEN_WEST,
        b if b == BlockId::TRAPDOOR_OPEN_WEST => BlockId::TRAPDOOR_CLOSED_WEST,
        other => other,
    }
}

fn trapdoor_block_for_facing(facing: HorizontalFacing) -> BlockId {
    match facing {
        HorizontalFacing::North => BlockId::TRAPDOOR_CLOSED,
        HorizontalFacing::East => BlockId::TRAPDOOR_CLOSED_EAST,
        HorizontalFacing::South => BlockId::TRAPDOOR_CLOSED_SOUTH,
        HorizontalFacing::West => BlockId::TRAPDOOR_CLOSED_WEST,
    }
}

fn facing_to_ivec3(facing: HorizontalFacing) -> IVec3 {
    match facing {
        HorizontalFacing::North => IVec3::new(0, 0, -1),
        HorizontalFacing::East => IVec3::new(1, 0, 0),
        HorizontalFacing::South => IVec3::new(0, 0, 1),
        HorizontalFacing::West => IVec3::new(-1, 0, 0),
    }
}

fn bed_blocks_for_facing(facing: HorizontalFacing) -> (BlockId, BlockId) {
    match facing {
        HorizontalFacing::North => (BlockId::BED_FOOT, BlockId::BED_HEAD),
        HorizontalFacing::East => (BlockId::BED_FOOT_EAST, BlockId::BED_HEAD_EAST),
        HorizontalFacing::South => (BlockId::BED_FOOT_SOUTH, BlockId::BED_HEAD_SOUTH),
        HorizontalFacing::West => (BlockId::BED_FOOT_WEST, BlockId::BED_HEAD_WEST),
    }
}

fn block_drop_item(block: BlockId) -> Option<ItemId> {
    if block == BlockId::AIR || block == BlockId(1) { return None; }
    if is_wheat_block(block) {
        return if wheat_growth_stage(block) == Some(7) {
            Some(ItemId::WHEAT_ITEM)
        } else {
            Some(ItemId::WHEAT_SEEDS)
        };
    }
    if block.0 < FIRST_NON_BLOCK_ITEM_ID {
        Some(ItemId(block.0))
    } else {
        None
    }
}

fn percentile_index(len: usize, percentile: f32) -> usize {
    if len == 0 {
        return 0;
    }
    let rank = (len as f32 * percentile).ceil() as usize;
    rank.saturating_sub(1).min(len - 1)
}

fn mesh_upload_budget_for_fps(fps: f32) -> MeshUploadBudget {
    if fps >= 90.0 {
        MeshUploadBudget {
            max_bytes: UPLOAD_BUDGET_HIGH_BYTES,
            max_chunks: 8,
        }
    } else if fps >= 72.0 {
        MeshUploadBudget {
            max_bytes: UPLOAD_BUDGET_MEDIUM_BYTES,
            max_chunks: 4,
        }
    } else if fps >= 55.0 {
        MeshUploadBudget {
            max_bytes: UPLOAD_BUDGET_LOW_BYTES,
            max_chunks: 2,
        }
    } else {
        MeshUploadBudget {
            max_bytes: UPLOAD_BUDGET_MIN_BYTES,
            max_chunks: 1,
        }
    }
}

fn mesh_upload_size_bytes(meshes: &ChunkMeshes) -> u64 {
    let vertex_count = meshes.opaque.vertices.len() + meshes.water.vertices.len();
    let index_bytes = meshes.opaque.indices.index_bytes() + meshes.water.indices.index_bytes();
    (vertex_count * std::mem::size_of::<ChunkVertex>()) as u64
        + index_bytes
}

fn should_upload_next_chunk(
    uploaded_chunks: u32,
    uploaded_bytes: u64,
    next_chunk_bytes: u64,
    budget: MeshUploadBudget,
) -> bool {
    if uploaded_chunks as usize >= budget.max_chunks {
        return false;
    }
    if uploaded_bytes + next_chunk_bytes <= budget.max_bytes {
        return true;
    }
    uploaded_chunks == 0
}

#[cfg(test)]
mod tests {
    use super::{
        mesh_upload_budget_for_fps, should_upload_next_chunk, MeshUploadBudget,
        UPLOAD_BUDGET_HIGH_BYTES, UPLOAD_BUDGET_LOW_BYTES, UPLOAD_BUDGET_MEDIUM_BYTES,
        UPLOAD_BUDGET_MIN_BYTES,
    };

    #[test]
    fn mesh_upload_budget_selects_expected_tiers() {
        let high = mesh_upload_budget_for_fps(95.0);
        assert_eq!(high.max_bytes, UPLOAD_BUDGET_HIGH_BYTES);
        assert_eq!(high.max_chunks, 8);

        let medium = mesh_upload_budget_for_fps(80.0);
        assert_eq!(medium.max_bytes, UPLOAD_BUDGET_MEDIUM_BYTES);
        assert_eq!(medium.max_chunks, 4);

        let low = mesh_upload_budget_for_fps(60.0);
        assert_eq!(low.max_bytes, UPLOAD_BUDGET_LOW_BYTES);
        assert_eq!(low.max_chunks, 2);

        let min = mesh_upload_budget_for_fps(40.0);
        assert_eq!(min.max_bytes, UPLOAD_BUDGET_MIN_BYTES);
        assert_eq!(min.max_chunks, 1);
    }

    #[test]
    fn starvation_rule_allows_first_chunk_over_byte_budget() {
        let budget = MeshUploadBudget {
            max_bytes: 2 * 1024 * 1024,
            max_chunks: 4,
        };
        assert!(should_upload_next_chunk(
            0,
            0,
            3 * 1024 * 1024,
            budget,
        ));
        assert!(!should_upload_next_chunk(
            1,
            3 * 1024 * 1024,
            512 * 1024,
            budget,
        ));
    }
}

fn simple_rng_next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

pub fn run() {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();
    println!("Veldspar client starting...");

    let event_loop = match EventLoop::new() {
        Ok(loop_handle) => loop_handle,
        Err(err) => {
            eprintln!("Failed to create event loop: {err}");
            return;
        }
    };

    let mut app = ClientApp::default();
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("Event loop exited with error: {err}");
    }
}
