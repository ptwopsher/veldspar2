pub mod atlas;
pub mod chunk_renderer;
pub mod clouds;
pub mod item_drop_renderer;
pub mod mesh;

pub mod particles;
pub mod pipeline;
pub mod player_renderer;
pub mod portal_renderer;
pub mod sky;
pub mod ui;
pub mod water_pipeline;

use std::cell::Cell;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use rustc_hash::FxHashMap;
use wgpu::util::DeviceExt;
use veldspar_shared::block::BlockRegistry;
use veldspar_shared::coords::ChunkPos;
use veldspar_shared::inventory::{Inventory, ItemId, ItemStack};
use winit::window::Window;

use crate::camera::Camera;
use crate::renderer::atlas::{build_atlas, AtlasBuildError, AtlasMapping};
use crate::renderer::chunk_renderer::{
    collect_visible_transparent_chunks,
    extract_frustum_planes,
    render_chunks_with_camera,
    render_visible_transparent_chunks,
    update_mesh_buffers,
    upload_mesh,
    ChunkPassStats,
    ChunkRenderData,
    FrustumPlanes,
    MeshUploadStats,
};
use crate::renderer::clouds::CloudRenderer;
use crate::renderer::item_drop_renderer::{ItemDropRenderData, ItemDropRenderer};
use crate::renderer::mesh::{ChunkMesh, ChunkMeshes};
use crate::renderer::particles::ParticleRenderer;
use crate::renderer::pipeline::ChunkPipeline;
use crate::renderer::player_renderer::{MobRenderInfo, PlayerRenderer, RemotePlayer};
use crate::renderer::portal_renderer::PortalRenderer;
use crate::renderer::sky::{sky_horizon_color, SkyRenderer};
use crate::renderer::water_pipeline::WaterPipeline;
use crate::ui::block_highlight::BlockHighlightRenderer;
use crate::ui::break_indicator::BreakIndicatorRenderer;
use crate::ui::crosshair::CrosshairRenderer;
use crate::ui::health_hud::HealthHudRenderer;
use crate::ui::inventory::{CraftingUiMode, InventoryRenderer};
use crate::ui::main_menu::{MainMenuRenderer, SettingsMenuView, WorldSelectView};
use crate::ui::text_overlay::TextOverlayRenderer;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const WEATHER_DARKEN_STRENGTH: f32 = 0.3;
const MAIN_PASS_FRUSTUM_CULLING: bool = false;

pub use crate::renderer::portal_renderer::{PortalRenderInfo, PortalRenderPortal};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    fog_color: [f32; 4],
    fog_start: f32,
    fog_end: f32,
    time_of_day: f32,
    underwater: f32,
    render_time_seconds: f32,
    _padding: [f32; 3],
}

impl CameraUniform {
    fn from_view_projection(
        view_proj: glam::Mat4,
        camera_pos: glam::Vec3,
        fog_start: f32,
        fog_end: f32,
        time_of_day: f32,
        underwater: bool,
        render_time_seconds: f32,
        weather_dim: f32,
    ) -> Self {
        let mut fog_color = sky_horizon_color(time_of_day);
        let weather_factor = 1.0 - WEATHER_DARKEN_STRENGTH * weather_dim.clamp(0.0, 1.0);
        fog_color[0] *= weather_factor;
        fog_color[1] *= weather_factor;
        fog_color[2] *= weather_factor;

        Self {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
            fog_color,
            fog_start,
            fog_end,
            time_of_day,
            underwater: if underwater { 1.0 } else { 0.0 },
            render_time_seconds,
            _padding: [0.0; 3],
        }
    }

    fn from_camera(
        camera: &Camera,
        fog_start: f32,
        fog_end: f32,
        time_of_day: f32,
        underwater: bool,
        render_time_seconds: f32,
        weather_dim: f32,
    ) -> Self {
        Self::from_view_projection(
            camera.view_projection_matrix(),
            camera.position,
            fog_start,
            fog_end,
            time_of_day,
            underwater,
            render_time_seconds,
            weather_dim,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderFrameStats {
    pub opaque_draw_calls: u32,
    pub portal_draw_calls: u32,
    pub portal_view_passes: u32,
    pub water_draw_calls: u32,
    pub rendered_chunks: u32,
    pub rendered_indices: u64,
    pub rendered_vertices: u64,
}

#[derive(Debug)]
struct DepthTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl DepthTexture {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Veldspar Depth Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
        }
    }
}

#[derive(Debug, Clone)]
struct CachedCameraState {
    camera: Camera,
    fog_start: f32,
    fog_end: f32,
    time_of_day: f32,
    underwater: bool,
    render_time_seconds: f32,
    weather_dim: f32,
}

impl Default for CachedCameraState {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            fog_start: 0.0,
            fog_end: 1.0,
            time_of_day: 0.5,
            underwater: false,
            render_time_seconds: 0.0,
            weather_dim: 0.0,
        }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    depth_texture: DepthTexture,
    chunk_pipeline: ChunkPipeline,
    water_pipeline: WaterPipeline,
    portal_renderer: PortalRenderer,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    portal_camera_uniform_buffers: [wgpu::Buffer; 2],
    portal_camera_bind_groups: [wgpu::BindGroup; 2],
    atlas_bind_group: wgpu::BindGroup,
    _atlas_texture: wgpu::Texture,
    atlas_mapping: AtlasMapping,
    chunk_meshes: FxHashMap<ChunkPos, ChunkRenderData>,
    water_meshes: FxHashMap<ChunkPos, ChunkRenderData>,
    visible_transparent: Vec<(ChunkPos, f32)>,
    portal_visible_transparent: [Vec<(ChunkPos, f32)>; 2],
    portal_render_info: PortalRenderInfo,
    sky_renderer: SkyRenderer,
    cloud_renderer: CloudRenderer,
    particle_renderer: ParticleRenderer,
    item_drop_renderer: ItemDropRenderer,
    crosshair_renderer: CrosshairRenderer,
    block_highlight: BlockHighlightRenderer,
    break_indicator: BreakIndicatorRenderer,
    inventory_renderer: InventoryRenderer,
    health_hud_renderer: HealthHudRenderer,
    text_overlay_renderer: TextOverlayRenderer,
    player_renderer: PlayerRenderer,
    main_menu_renderer: MainMenuRenderer,
    camera_state: CachedCameraState,
    view_proj: Cell<glam::Mat4>,
    camera_pos: Cell<glam::Vec3>,
    crosshair_visible: bool,
    last_frame_stats: RenderFrameStats,
}

#[derive(Debug)]
pub enum RendererInitError {
    CreateSurface(wgpu::CreateSurfaceError),
    RequestAdapter(wgpu::RequestAdapterError),
    RequestDevice(wgpu::RequestDeviceError),
    UnsupportedSurface,
    BuildAtlas(AtlasBuildError),
}

impl fmt::Display for RendererInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateSurface(err) => write!(f, "failed to create surface: {err}"),
            Self::RequestAdapter(err) => write!(f, "failed to request adapter: {err}"),
            Self::RequestDevice(err) => write!(f, "failed to request device: {err}"),
            Self::UnsupportedSurface => write!(f, "adapter does not support this surface"),
            Self::BuildAtlas(err) => write!(f, "failed to build texture atlas: {err}"),
        }
    }
}

impl std::error::Error for RendererInitError {}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Result<Self, RendererInitError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(RendererInitError::CreateSurface)?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(RendererInitError::RequestAdapter)?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Veldspar Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(RendererInitError::RequestDevice)?;

        let initial_size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, initial_size.width.max(1), initial_size.height.max(1))
            .ok_or(RendererInitError::UnsupportedSurface)?;

        surface.configure(&device, &surface_config);

        let chunk_pipeline = ChunkPipeline::new(&device, surface_config.format, DEPTH_FORMAT);
        let water_pipeline = WaterPipeline::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
            &chunk_pipeline.texture_bind_group_layout,
            &chunk_pipeline.chunk_params_bind_group_layout,
        );
        let mut portal_renderer = PortalRenderer::new(
            &device,
            surface_config.format,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let atlas_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Excalibur_V1/assets/minecraft/textures/block");
        let (atlas_texture, atlas_view, atlas_sampler, atlas_mapping) =
            build_atlas(&device, &queue, &atlas_dir).map_err(RendererInitError::BuildAtlas)?;
        let atlas_bind_group = create_texture_bind_group(
            &device,
            &chunk_pipeline.texture_bind_group_layout,
            &atlas_view,
            &atlas_sampler,
            "Chunk Atlas Bind Group",
        );

        let initial_camera_uniform = CameraUniform {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0; 4],
            fog_color: sky_horizon_color(0.5),
            fog_start: 0.0,
            fog_end: 1.0,
            time_of_day: 0.5,
            underwater: 0.0,
            render_time_seconds: 0.0,
            _padding: [0.0; 3],
        };
        let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&initial_camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chunk Camera Bind Group"),
            layout: &chunk_pipeline.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });
        let portal_camera_uniform_buffers = std::array::from_fn(|index| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(match index {
                    0 => "Portal Camera Uniform Buffer Orange",
                    _ => "Portal Camera Uniform Buffer Blue",
                }),
                contents: bytemuck::bytes_of(&initial_camera_uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        });
        let portal_camera_bind_groups = std::array::from_fn(|index| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(match index {
                    0 => "Portal Camera Bind Group Orange",
                    _ => "Portal Camera Bind Group Blue",
                }),
                layout: &chunk_pipeline.camera_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: portal_camera_uniform_buffers[index].as_entire_binding(),
                }],
            })
        });
        portal_renderer.resize(&device, surface_config.width, surface_config.height);
        let depth_texture = DepthTexture::new(&device, surface_config.width, surface_config.height);
        let sky_renderer = SkyRenderer::new(&device, surface_config.format);
        let cloud_renderer = CloudRenderer::new(&device, surface_config.format);
        let particle_renderer = ParticleRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let item_drop_renderer = ItemDropRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
            &chunk_pipeline.texture_bind_group_layout,
            create_texture_bind_group(
                &device,
                &chunk_pipeline.texture_bind_group_layout,
                &atlas_view,
                &atlas_sampler,
                "Item Drop Atlas Bind Group",
            ),
        );
        let crosshair_renderer = CrosshairRenderer::new(
            &device,
            surface_config.format,
            surface_config.width,
            surface_config.height,
        );
        let block_highlight = BlockHighlightRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let break_indicator = BreakIndicatorRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let inventory_renderer = InventoryRenderer::new(
            &device,
            surface_config.format,
            &chunk_pipeline.texture_bind_group_layout,
            create_texture_bind_group(
                &device,
                &chunk_pipeline.texture_bind_group_layout,
                &atlas_view,
                &atlas_sampler,
                "Inventory Atlas Bind Group",
            ),
            atlas_mapping.clone(),
        );
        let health_hud_renderer = HealthHudRenderer::new(&device, surface_config.format);
        let text_overlay_renderer = TextOverlayRenderer::new(&device, surface_config.format);
        let player_renderer = PlayerRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let main_menu_renderer = MainMenuRenderer::new(
            &device,
            surface_config.format,
            surface_config.width,
            surface_config.height,
        );

        player_renderer.init_buffers(&queue);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            depth_texture,
            chunk_pipeline,
            water_pipeline,
            portal_renderer,
            camera_uniform_buffer,
            camera_bind_group,
            portal_camera_uniform_buffers,
            portal_camera_bind_groups,
            atlas_bind_group,
            _atlas_texture: atlas_texture,
            atlas_mapping,
            chunk_meshes: FxHashMap::default(),
            water_meshes: FxHashMap::default(),
            visible_transparent: Vec::new(),
            portal_visible_transparent: std::array::from_fn(|_| Vec::new()),
            portal_render_info: PortalRenderInfo::default(),
            sky_renderer,
            cloud_renderer,
            particle_renderer,
            item_drop_renderer,
            crosshair_renderer,
            block_highlight,
            break_indicator,
            inventory_renderer,
            health_hud_renderer,
            text_overlay_renderer,
            player_renderer,
            main_menu_renderer,
            camera_state: CachedCameraState::default(),
            view_proj: Cell::new(glam::Mat4::IDENTITY),
            camera_pos: Cell::new(glam::Vec3::ZERO),
            crosshair_visible: true,
            last_frame_stats: RenderFrameStats::default(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.depth_texture = DepthTexture::new(&self.device, width, height);
        self.portal_renderer.resize(&self.device, width, height);
        self.crosshair_renderer.resize(&self.queue, width, height);
    }

    pub fn atlas_mapping(&self) -> &AtlasMapping {
        &self.atlas_mapping
    }

    pub fn last_frame_stats(&self) -> RenderFrameStats {
        self.last_frame_stats
    }

    pub fn set_portal_render_info(&mut self, portal_render_info: PortalRenderInfo) {
        self.portal_render_info = portal_render_info;
    }

    pub fn reserve_chunk_mesh_capacity(&mut self, loaded_chunks: usize) {
        let target = loaded_chunks.max(64);
        if self.chunk_meshes.capacity() < target {
            self.chunk_meshes.reserve(target - self.chunk_meshes.capacity());
        }
        if self.water_meshes.capacity() < target {
            self.water_meshes.reserve(target - self.water_meshes.capacity());
        }
        if self.visible_transparent.capacity() < target {
            self.visible_transparent
                .reserve(target - self.visible_transparent.capacity());
        }
        for visible in &mut self.portal_visible_transparent {
            if visible.capacity() < target {
                visible.reserve(target - visible.capacity());
            }
        }
    }

    pub fn update_camera_uniform(
        &mut self,
        camera: &Camera,
        fog_start: f32,
        fog_end: f32,
        time_of_day: f32,
        underwater: bool,
        render_time_seconds: f32,
        weather_dim: f32,
    ) {
        let uniform = CameraUniform::from_camera(
            camera,
            fog_start,
            fog_end,
            time_of_day,
            underwater,
            render_time_seconds,
            weather_dim,
        );
        self.queue
            .write_buffer(&self.camera_uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        // Store view_proj and camera_pos for frustum culling
        self.view_proj.set(camera.view_projection_matrix());
        self.camera_pos.set(camera.position);
        self.camera_state = CachedCameraState {
            camera: camera.clone(),
            fog_start,
            fog_end,
            time_of_day,
            underwater,
            render_time_seconds,
            weather_dim,
        };
    }

    pub fn update_sky(&self, camera: &Camera, time_of_day: f32, weather_dim: f32) {
        self.sky_renderer
            .update(&self.queue, camera, time_of_day, weather_dim);
    }

    pub fn update_clouds(&self, camera: &Camera, time: f32) {
        self.cloud_renderer.update(&self.queue, camera, time);
    }

    pub fn update_particles(&mut self, dt: f32, camera: &Camera) {
        let forward = camera.forward_direction();
        let mut right = forward.cross(glam::Vec3::Y).normalize_or_zero();
        if right.length_squared() < 1e-6 {
            right = glam::Vec3::X;
        }
        let up = right.cross(forward).normalize_or_zero();
        self.particle_renderer.update(&self.queue, dt, right, up);
    }

    pub fn spawn_break_particles(&mut self, block_pos: glam::IVec3, color: [f32; 3]) {
        self.particle_renderer.spawn_break_particles(block_pos, color);
    }

    pub fn spawn_walk_particles(&mut self, position: glam::Vec3, color: [f32; 3]) {
        self.particle_renderer.spawn_walk_particles(position, color);
    }

    pub fn spawn_place_particles(&mut self, block_pos: glam::IVec3, color: [f32; 3]) {
        self.particle_renderer.spawn_place_particles(block_pos, color);
    }

    pub fn spawn_rain_particles(&mut self, spawn_positions: &[glam::Vec3]) {
        self.particle_renderer.spawn_rain_particles(spawn_positions);
    }

    pub fn spawn_snow_particles(&mut self, spawn_positions: &[glam::Vec3]) {
        self.particle_renderer.spawn_snow_particles(spawn_positions);
    }

    pub fn upload_chunk_mesh(&mut self, mesh: &ChunkMesh, chunk_pos: ChunkPos) {
        if mesh.is_empty() {
            return;
        }

        let (data, _) = upload_mesh(
            &self.device,
            &self.queue,
            mesh,
            chunk_pos,
            &self.chunk_pipeline.chunk_params_bind_group_layout,
            0.0,
        );
        self.chunk_meshes.insert(chunk_pos, data);
    }

    pub fn clear_chunk_meshes(&mut self) {
        self.chunk_meshes.clear();
        self.water_meshes.clear();
        self.visible_transparent.clear();
        for visible in &mut self.portal_visible_transparent {
            visible.clear();
        }
        self.particle_renderer.clear();
        self.item_drop_renderer.clear();
    }

    pub fn replace_chunk_mesh(
        &mut self,
        meshes: &ChunkMeshes,
        chunk_pos: ChunkPos,
        spawn_time_seconds: f32,
    ) -> MeshUploadStats {
        let mut stats = MeshUploadStats::default();

        accumulate_upload_stats(
            &mut stats,
            replace_render_mesh(
                &self.device,
                &self.queue,
                &self.chunk_pipeline.chunk_params_bind_group_layout,
                &mut self.chunk_meshes,
                chunk_pos,
                &meshes.opaque,
                spawn_time_seconds,
            ),
        );
        accumulate_upload_stats(
            &mut stats,
            replace_render_mesh(
                &self.device,
                &self.queue,
                &self.chunk_pipeline.chunk_params_bind_group_layout,
                &mut self.water_meshes,
                chunk_pos,
                &meshes.water,
                spawn_time_seconds,
            ),
        );

        stats
    }

    pub fn remove_chunk_mesh(&mut self, chunk_pos: ChunkPos) {
        self.chunk_meshes.remove(&chunk_pos);
        self.water_meshes.remove(&chunk_pos);
    }

    pub fn update_highlight(&mut self, block_pos: Option<glam::IVec3>) {
        self.block_highlight.update(&self.queue, block_pos);
    }

    pub fn update_break_indicator(
        &mut self,
        local_block_pos: Option<glam::IVec3>,
        local_progress: f32,
        remote_overlays: &[(glam::IVec3, f32)],
    ) {
        self.break_indicator
            .update(&self.queue, local_block_pos, local_progress, remote_overlays);
    }

    pub fn update_inventory_ui(
        &mut self,
        gui_scale: f32,
        inventory: &Inventory,
        selected_hotbar_slot: usize,
        inventory_open: bool,
        crafting_mode: CraftingUiMode,
        crafting_inputs: &[Option<ItemStack>],
        crafting_output: Option<ItemStack>,
        chest_inventory: Option<&Inventory>,
        cursor_stack: Option<ItemStack>,
        cursor_position: Option<(f64, f64)>,
        registry: Option<&BlockRegistry>,
    ) {
        self.inventory_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            gui_scale,
            inventory,
            selected_hotbar_slot,
            inventory_open,
            crafting_mode,
            crafting_inputs,
            crafting_output,
            chest_inventory,
            cursor_stack,
            cursor_position,
            registry,
        );
    }

    pub fn update_health_hud(
        &mut self,
        gui_scale: f32,
        health: f32,
        hunger: f32,
        damage_flash_timer: f32,
        visible: bool,
        air_supply: f32,
        max_air_supply: f32,
        xp_progress: f32,
        xp_level: u32,
    ) {
        self.health_hud_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            gui_scale,
            health,
            hunger,
            damage_flash_timer,
            visible,
            air_supply,
            max_air_supply,
            xp_progress,
            xp_level,
        );
    }

    pub fn set_crosshair_visible(&mut self, visible: bool) {
        self.crosshair_visible = visible;
    }

    pub fn update_overlay_lines(
        &mut self,
        lines: &[String],
        chat_lines: &[(String, f32)],
        chat_input_line: Option<&str>,
    ) {
        self.text_overlay_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            lines,
            chat_lines,
            chat_input_line,
        );
    }

    pub fn update_players(&mut self, players: &[RemotePlayer]) {
        self.player_renderer.update_players(&self.queue, players);
    }

    pub fn update_mob_data(&mut self, mobs: &[MobRenderInfo]) {
        self.player_renderer
            .update_mobs(&self.device, &self.queue, mobs);
    }

    pub fn update_item_drops(&mut self, drops: &[ItemDropRenderData]) {
        self.item_drop_renderer.update(&self.queue, drops);
    }

    pub fn update_first_person_hand(
        &mut self,
        camera_pos: glam::Vec3,
        camera_forward: glam::Vec3,
        attack_animation: f32,
    ) {
        self.player_renderer.update_first_person_hand(
            &self.queue,
            camera_pos,
            camera_forward,
            attack_animation,
        );
    }

    pub fn update_first_person_hand_item(&mut self, selected_item: ItemId) {
        self.player_renderer
            .update_hand_item(&self.device, selected_item, &self.atlas_mapping);
    }

    pub fn update_main_menu(&mut self, selected_item: u8, show_ip_field: bool, server_ip: &str) {
        self.main_menu_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            selected_item,
            show_ip_field,
            server_ip,
        );
    }

    pub fn update_world_select_menu(&mut self, view: &WorldSelectView<'_>) {
        self.main_menu_renderer.update_world_select(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            view,
        );
    }

    pub fn update_pause_menu(&mut self, selected_item: u8) {
        self.main_menu_renderer.update_pause(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            selected_item,
        );
    }

    pub fn update_settings_menu(&mut self, selected_item: u8, view: &SettingsMenuView) {
        self.main_menu_renderer.update_settings(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            selected_item,
            view,
        );
    }

    pub fn render_main_menu_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Menu Command Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Menu Sky Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.sky_renderer.render(&mut render_pass);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Menu UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.main_menu_renderer.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    fn render_portal_views(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        main_frustum: &FrustumPlanes,
    ) -> u32 {
        let camera_state = self.camera_state.clone();
        let queue = &self.queue;
        let sky_renderer = &self.sky_renderer;
        let cloud_renderer = &self.cloud_renderer;
        let chunk_pipeline = self.chunk_pipeline.pipeline();
        let water_pipeline = self.water_pipeline.pipeline();
        let atlas_bind_group = &self.atlas_bind_group;
        let chunk_meshes = &self.chunk_meshes;
        let water_meshes = &self.water_meshes;
        let portal_camera_uniform_buffers = &self.portal_camera_uniform_buffers;
        let portal_camera_bind_groups = &self.portal_camera_bind_groups;
        let portal_visible_transparent = &mut self.portal_visible_transparent;

        let rendered_passes = self.portal_renderer.render_portal_views(
            &self.portal_render_info,
            &camera_state.camera,
            camera_state.camera.position,
            main_frustum,
            |source_index, portal_camera, color_view, depth_view| {
                let uniform = CameraUniform::from_view_projection(
                    portal_camera.view_proj,
                    portal_camera.position,
                    camera_state.fog_start,
                    camera_state.fog_end,
                    camera_state.time_of_day,
                    camera_state.underwater,
                    camera_state.render_time_seconds,
                    camera_state.weather_dim,
                );
                queue.write_buffer(
                    &portal_camera_uniform_buffers[source_index],
                    0,
                    bytemuck::bytes_of(&uniform),
                );
                let portal_camera_bind_group = &portal_camera_bind_groups[source_index];
                let portal_frustum = extract_frustum_planes(portal_camera.view_proj);

                collect_visible_transparent_chunks(
                    water_meshes,
                    &portal_frustum,
                    portal_camera.position,
                    &mut portal_visible_transparent[source_index],
                    true,
                );

                let clear_color = sky_horizon_color(camera_state.time_of_day);
                sky_renderer.update(
                    queue,
                    &portal_camera.camera,
                    camera_state.time_of_day,
                    camera_state.weather_dim,
                );
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Portal RTT Sky Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: clear_color[0] as f64,
                                    g: clear_color[1] as f64,
                                    b: clear_color[2] as f64,
                                    a: clear_color[3] as f64,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    sky_renderer.render(&mut render_pass);
                }

                cloud_renderer.update(queue, &portal_camera.camera, camera_state.time_of_day);
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Portal RTT Cloud Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    cloud_renderer.render(&mut render_pass);
                }

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Portal RTT Opaque Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    render_pass.set_pipeline(chunk_pipeline);
                    render_pass.set_bind_group(1, atlas_bind_group, &[]);
                    let _ = render_chunks_with_camera(
                        &mut render_pass,
                        chunk_meshes,
                        &portal_frustum,
                        true,
                        portal_camera_bind_group,
                    );
                }

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Portal RTT Water Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    render_pass.set_pipeline(water_pipeline);
                    render_pass.set_bind_group(0, portal_camera_bind_group, &[]);
                    render_pass.set_bind_group(1, atlas_bind_group, &[]);
                    let _ = render_visible_transparent_chunks(
                        &mut render_pass,
                        water_meshes,
                        &portal_visible_transparent[source_index],
                    );
                }
            },
        );

        self.sky_renderer.update(
            &self.queue,
            &self.camera_state.camera,
            self.camera_state.time_of_day,
            self.camera_state.weather_dim,
        );
        self.cloud_renderer.update(
            &self.queue,
            &self.camera_state.camera,
            self.camera_state.time_of_day,
        );

        rendered_passes
    }

    pub fn render_frame(&mut self, draw_menu_overlay: bool) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let frustum_planes = extract_frustum_planes(self.view_proj.get());
        let mut frame_stats = RenderFrameStats::default();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Veldspar Command Encoder"),
            });

        frame_stats.portal_view_passes = self.render_portal_views(&mut encoder, &frustum_planes);

        collect_visible_transparent_chunks(
            &self.water_meshes,
            &frustum_planes,
            self.camera_pos.get(),
            &mut self.visible_transparent,
            MAIN_PASS_FRUSTUM_CULLING,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Sky Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.529,
                            g: 0.808,
                            b: 0.922,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.sky_renderer.render(&mut render_pass);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Cloud Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.cloud_renderer.render(&mut render_pass);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Opaque Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(self.chunk_pipeline.pipeline());
            render_pass.set_bind_group(1, &self.atlas_bind_group, &[]);
            let chunk_stats = render_chunks_with_camera(
                &mut render_pass,
                &self.chunk_meshes,
                &frustum_planes,
                MAIN_PASS_FRUSTUM_CULLING,
                &self.camera_bind_group,
            );
            accumulate_chunk_stats(&mut frame_stats, chunk_stats, true);
            frame_stats.portal_draw_calls += self.portal_renderer.render_portal_frames(
                &self.queue,
                &mut render_pass,
                &self.camera_bind_group,
                &self.portal_render_info,
            );

            self.block_highlight.render(&mut render_pass, &self.camera_bind_group);
            self.break_indicator
                .render(&mut render_pass, &self.camera_bind_group);
            self.item_drop_renderer
                .render(&mut render_pass, &self.camera_bind_group);
            self.player_renderer.render(&mut render_pass, &self.camera_bind_group);
            self.player_renderer
                .render_mobs(&mut render_pass, &self.camera_bind_group);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Portal Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            frame_stats.portal_draw_calls += self.portal_renderer.render_portal_surfaces(
                &self.queue,
                &mut render_pass,
                &self.camera_bind_group,
                &self.portal_render_info,
            );
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Water Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(self.water_pipeline.pipeline());
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.atlas_bind_group, &[]);
            let water_stats = render_visible_transparent_chunks(
                &mut render_pass,
                &self.water_meshes,
                &self.visible_transparent,
            );
            accumulate_chunk_stats(&mut frame_stats, water_stats, false);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Particle Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.particle_renderer
                .render(&mut render_pass, &self.camera_bind_group);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar First Person Hand Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.player_renderer
                .render_first_person_hand(&mut render_pass, &self.camera_bind_group);
            self.player_renderer.render_first_person_hand_block(
                &mut render_pass,
                &self.camera_bind_group,
                &self.atlas_bind_group,
                self.item_drop_renderer.pipeline(),
            );
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if self.crosshair_visible {
                self.crosshair_renderer.render(&mut render_pass);
            }
            self.health_hud_renderer.render(&mut render_pass);
            self.inventory_renderer.render(&mut render_pass);
            self.text_overlay_renderer.render(&mut render_pass);
        }

        if draw_menu_overlay {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Menu Overlay Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.main_menu_renderer.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        self.last_frame_stats = frame_stats;
        Ok(())
    }
}

fn accumulate_chunk_stats(
    frame_stats: &mut RenderFrameStats,
    pass_stats: ChunkPassStats,
    opaque: bool,
) {
    if opaque {
        frame_stats.opaque_draw_calls += pass_stats.draw_calls;
    } else {
        frame_stats.water_draw_calls += pass_stats.draw_calls;
    }
    frame_stats.rendered_chunks += pass_stats.rendered_chunks;
    frame_stats.rendered_indices += pass_stats.rendered_indices;
    frame_stats.rendered_vertices += pass_stats.rendered_vertices;
}

fn accumulate_upload_stats(total: &mut MeshUploadStats, delta: MeshUploadStats) {
    total.uploaded_bytes += delta.uploaded_bytes;
    total.buffer_reallocations += delta.buffer_reallocations;
}

fn replace_render_mesh(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    chunk_params_layout: &wgpu::BindGroupLayout,
    meshes: &mut FxHashMap<ChunkPos, ChunkRenderData>,
    chunk_pos: ChunkPos,
    mesh: &ChunkMesh,
    spawn_time_seconds: f32,
) -> MeshUploadStats {
    if mesh.is_empty() {
        meshes.remove(&chunk_pos);
        return MeshUploadStats::default();
    }

    if let Some(existing) = meshes.get_mut(&chunk_pos) {
        return update_mesh_buffers(device, queue, existing, mesh);
    }

    let (data, stats) = upload_mesh(
        device,
        queue,
        mesh,
        chunk_pos,
        chunk_params_layout,
        spawn_time_seconds,
    );
    meshes.insert(chunk_pos, data);
    stats
}

fn create_texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
