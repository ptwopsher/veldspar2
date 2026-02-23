use std::collections::HashSet;
use std::mem;
use std::sync::{OnceLock, RwLock};

use bytemuck::{Pod, Zeroable};
use veldspar_shared::block::{BlockId, BlockRegistry};
use veldspar_shared::inventory::{
    max_stack_for_item, Inventory, ItemId, ItemStack, FIRST_NON_BLOCK_ITEM_ID,
};

use crate::renderer::atlas::AtlasMapping;

const MAX_QUADS: usize = 48_000;
const MAX_VERTICES: usize = MAX_QUADS * 4;
const MAX_INDICES: usize = MAX_QUADS * 6;

const SLOT_BORDER_PX: f32 = 2.0;
const HOTBAR_MARGIN_BOTTOM_PX: f32 = 18.0;
const HOTBAR_SLOT_SIZE_PX: f32 = 42.0;
const HOTBAR_SLOT_GAP_PX: f32 = 4.0;
const MAX_CRAFTING_INPUT_SLOTS: usize = 9;
pub const CHEST_SLOT_COUNT: usize = 27;
const CREATIVE_COLUMNS: usize = 9;
const CREATIVE_VISIBLE_ROWS: usize = 5;
const CREATIVE_VISIBLE_SLOTS: usize = CREATIVE_COLUMNS * CREATIVE_VISIBLE_ROWS;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UiVertex {
    position: [f32; 2],
    color: [f32; 4],
    tex_coord: [f32; 2],
    tile_origin: [f32; 2],
    use_texture: f32,
}

#[derive(Copy, Clone, Debug, Default)]
struct RectPx {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl RectPx {
    fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Copy, Clone)]
struct InventoryLayout {
    panel: RectPx,
    slots: [RectPx; Inventory::TOTAL_SIZE],
    crafting_inputs: [RectPx; MAX_CRAFTING_INPUT_SLOTS],
    crafting_input_count: usize,
    crafting_output: RectPx,
}

#[derive(Copy, Clone)]
struct ChestLayout {
    panel: RectPx,
    chest_slots: [RectPx; CHEST_SLOT_COUNT],
    player_slots: [RectPx; Inventory::TOTAL_SIZE],
}

#[derive(Clone, Debug, Default)]
struct CreativeUiState {
    enabled: bool,
    search: String,
    scroll: usize,
    catalog: Vec<ItemId>,
}

#[derive(Copy, Clone)]
struct CreativeLayout {
    panel: RectPx,
    search_bar: RectPx,
    clear_button: RectPx,
    catalog_slots: [RectPx; CREATIVE_VISIBLE_SLOTS],
    hotbar_slots: [RectPx; Inventory::HOTBAR_SIZE],
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CraftingUiMode {
    Inventory2x2,
    CraftingTable3x3,
}

impl CraftingUiMode {
    fn input_slot_count(self) -> usize {
        match self {
            Self::Inventory2x2 => 4,
            Self::CraftingTable3x3 => 9,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OpenInventoryHitTarget {
    InventorySlot(usize),
    ChestSlot(usize),
    CraftingInput(usize),
    CraftingOutput,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CreativeHitTarget {
    CatalogSlot(usize),
    HotbarSlot(usize),
    ClearButton,
    SearchBar,
}

fn creative_ui_state_storage() -> &'static RwLock<CreativeUiState> {
    static STORAGE: OnceLock<RwLock<CreativeUiState>> = OnceLock::new();
    STORAGE.get_or_init(|| RwLock::new(CreativeUiState::default()))
}

fn read_creative_ui_state() -> CreativeUiState {
    match creative_ui_state_storage().read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

pub fn set_creative_ui_state(enabled: bool, search: &str, scroll: usize, catalog: &[ItemId]) {
    let mut state = match creative_ui_state_storage().write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    state.enabled = enabled;
    state.search.clear();
    state.search.push_str(search);
    state.scroll = scroll;
    state.catalog.clear();
    state.catalog.extend_from_slice(catalog);
}

fn normalized_gui_scale(gui_scale: f32) -> f32 {
    gui_scale.clamp(1.0, 3.0)
}

fn scale_rect(rect: RectPx, scale: f32) -> RectPx {
    RectPx {
        x: rect.x * scale,
        y: rect.y * scale,
        w: rect.w * scale,
        h: rect.h * scale,
    }
}

fn scale_inventory_layout(layout: &mut InventoryLayout, scale: f32) {
    layout.panel = scale_rect(layout.panel, scale);
    for rect in &mut layout.slots {
        *rect = scale_rect(*rect, scale);
    }
    for rect in &mut layout.crafting_inputs {
        *rect = scale_rect(*rect, scale);
    }
    layout.crafting_output = scale_rect(layout.crafting_output, scale);
}

fn scale_chest_layout(layout: &mut ChestLayout, scale: f32) {
    layout.panel = scale_rect(layout.panel, scale);
    for rect in &mut layout.chest_slots {
        *rect = scale_rect(*rect, scale);
    }
    for rect in &mut layout.player_slots {
        *rect = scale_rect(*rect, scale);
    }
}

fn scale_creative_layout(layout: &mut CreativeLayout, scale: f32) {
    layout.panel = scale_rect(layout.panel, scale);
    layout.search_bar = scale_rect(layout.search_bar, scale);
    layout.clear_button = scale_rect(layout.clear_button, scale);
    for rect in &mut layout.catalog_slots {
        *rect = scale_rect(*rect, scale);
    }
    for rect in &mut layout.hotbar_slots {
        *rect = scale_rect(*rect, scale);
    }
}

pub fn creative_max_scroll(catalog_len: usize) -> usize {
    let total_rows = (catalog_len + CREATIVE_COLUMNS - 1) / CREATIVE_COLUMNS;
    total_rows.saturating_sub(CREATIVE_VISIBLE_ROWS)
}

fn creative_block_catalog_key(name: &str) -> &str {
    if name.starts_with("door_") {
        return "wooden_door";
    }
    if name.starts_with("bed_") {
        return "bed";
    }
    if let Some((prefix, _)) = name.rsplit_once("_stage_") {
        return prefix;
    }
    if let Some(base) = name.strip_suffix("_north") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_east") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_south") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_west") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_open") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_closed") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_top") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_bottom") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_on") {
        return base;
    }
    if let Some(base) = name.strip_suffix("_off") {
        return base;
    }
    name
}

pub fn build_creative_catalog(registry: &BlockRegistry) -> Vec<ItemId> {
    let mut items = Vec::new();
    let mut seen_block_keys = HashSet::<String>::new();

    for raw_id in 1..registry.len() {
        let block_id = BlockId(raw_id as u16);
        let props = registry.get_properties(block_id);
        let name = props.name.as_str();
        if name == "air" || name.starts_with("flowing_") {
            continue;
        }
        let key = creative_block_catalog_key(name);
        if seen_block_keys.insert(key.to_string()) {
            items.push(ItemId::from(block_id));
        }
    }

    for id in FIRST_NON_BLOCK_ITEM_ID..=ItemId::BONE_MEAL.0 {
        items.push(ItemId(id));
    }
    items.push(ItemId::PORTAL_GUN);

    items
}

pub struct InventoryRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    atlas_bind_group: wgpu::BindGroup,
    atlas_mapping: AtlasMapping,
}

impl InventoryRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        atlas_bind_group_layout: &wgpu::BindGroupLayout,
        atlas_bind_group: wgpu::BindGroup,
        atlas_mapping: AtlasMapping,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Inventory UI Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/ui.wgsl"
                ))
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Inventory UI Pipeline Layout"),
            bind_group_layouts: &[atlas_bind_group_layout],
            push_constant_ranges: &[],
        });

        let attributes = &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32,
            },
        ];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Inventory UI Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<UiVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Inventory UI Vertex Buffer"),
            size: (MAX_VERTICES * mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Inventory UI Index Buffer"),
            size: (MAX_INDICES * mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count: 0,
            atlas_bind_group,
            atlas_mapping,
        }
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
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
        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let ui_scale = normalized_gui_scale(gui_scale);
        let mut vertices = Vec::with_capacity(8_192);
        let creative_state = read_creative_ui_state();

        if inventory_open {
            if let Some(chest_inventory) = chest_inventory {
                let layout = build_chest_layout_scaled(screen_w, screen_h, ui_scale);

                create_quad_px(
                    &mut vertices,
                    0.0,
                    0.0,
                    screen_w,
                    screen_h,
                    screen_w,
                    screen_h,
                    [0.0, 0.0, 0.0, 0.45],
                );

                create_quad_px(
                    &mut vertices,
                    layout.panel.x,
                    layout.panel.y,
                    layout.panel.w,
                    layout.panel.h,
                    screen_w,
                    screen_h,
                    [0.08, 0.09, 0.12, 0.92],
                );

                render_text_center_px(
                    &mut vertices,
                    "CHEST",
                    layout.panel.x + layout.panel.w * 0.5,
                    layout.panel.y + 16.0,
                    2.8,
                    screen_w,
                    screen_h,
                    [0.94, 0.96, 1.0, 1.0],
                );
                render_text_left_px(
                    &mut vertices,
                    "CHEST",
                    layout.chest_slots[0].x,
                    layout.chest_slots[0].y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );
                render_text_left_px(
                    &mut vertices,
                    "MAIN",
                    layout.player_slots[9].x,
                    layout.player_slots[9].y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );
                render_text_left_px(
                    &mut vertices,
                    "HOTBAR",
                    layout.player_slots[0].x,
                    layout.player_slots[0].y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );

                for slot in 0..CHEST_SLOT_COUNT {
                    draw_slot(
                        &mut vertices,
                        layout.chest_slots[slot],
                        chest_inventory.get(slot).copied(),
                        registry,
                        &self.atlas_mapping,
                        false,
                        true,
                        screen_w,
                        screen_h,
                    );
                }

                for slot in 0..Inventory::TOTAL_SIZE {
                    draw_slot(
                        &mut vertices,
                        layout.player_slots[slot],
                        inventory.get(slot).copied(),
                        registry,
                        &self.atlas_mapping,
                        slot < Inventory::HOTBAR_SIZE && slot == selected_hotbar_slot,
                        true,
                        screen_w,
                        screen_h,
                    );
                }

                if let (Some(held), Some((cx, cy))) = (cursor_stack, cursor_position) {
                    let slot_size = layout.player_slots[0].w;
                    let held_rect = RectPx {
                        x: cx as f32 + 14.0,
                        y: cy as f32 + 14.0,
                        w: slot_size,
                        h: slot_size,
                    };
                    draw_slot(
                        &mut vertices,
                        held_rect,
                        Some(held),
                        registry,
                        &self.atlas_mapping,
                        true,
                        true,
                        screen_w,
                        screen_h,
                    );
                }
            } else if creative_state.enabled {
                let layout = build_creative_layout_scaled(screen_w, screen_h, ui_scale);
                let scroll = creative_state
                    .scroll
                    .min(creative_max_scroll(creative_state.catalog.len()));
                let hover_target = cursor_position.and_then(|(cx, cy)| {
                    hit_test_creative(
                        cx as f32,
                        cy as f32,
                        screen_w,
                        screen_h,
                        ui_scale,
                        creative_state.catalog.len(),
                        scroll,
                    )
                });

                create_quad_px(
                    &mut vertices,
                    0.0,
                    0.0,
                    screen_w,
                    screen_h,
                    screen_w,
                    screen_h,
                    [0.0, 0.0, 0.0, 0.45],
                );

                create_quad_px(
                    &mut vertices,
                    layout.panel.x,
                    layout.panel.y,
                    layout.panel.w,
                    layout.panel.h,
                    screen_w,
                    screen_h,
                    [0.07, 0.09, 0.12, 0.94],
                );

                render_text_center_px(
                    &mut vertices,
                    "CREATIVE INVENTORY",
                    layout.panel.x + layout.panel.w * 0.5,
                    layout.panel.y + 14.0,
                    2.5,
                    screen_w,
                    screen_h,
                    [0.94, 0.96, 1.0, 1.0],
                );

                create_quad_px(
                    &mut vertices,
                    layout.search_bar.x,
                    layout.search_bar.y,
                    layout.search_bar.w,
                    layout.search_bar.h,
                    screen_w,
                    screen_h,
                    [0.16, 0.19, 0.24, 0.95],
                );
                let search_border = [0.38, 0.42, 0.5, 0.95];
                create_quad_px(
                    &mut vertices,
                    layout.search_bar.x,
                    layout.search_bar.y,
                    layout.search_bar.w,
                    SLOT_BORDER_PX,
                    screen_w,
                    screen_h,
                    search_border,
                );
                create_quad_px(
                    &mut vertices,
                    layout.search_bar.x,
                    layout.search_bar.y + layout.search_bar.h - SLOT_BORDER_PX,
                    layout.search_bar.w,
                    SLOT_BORDER_PX,
                    screen_w,
                    screen_h,
                    search_border,
                );
                create_quad_px(
                    &mut vertices,
                    layout.search_bar.x,
                    layout.search_bar.y,
                    SLOT_BORDER_PX,
                    layout.search_bar.h,
                    screen_w,
                    screen_h,
                    search_border,
                );
                create_quad_px(
                    &mut vertices,
                    layout.search_bar.x + layout.search_bar.w - SLOT_BORDER_PX,
                    layout.search_bar.y,
                    SLOT_BORDER_PX,
                    layout.search_bar.h,
                    screen_w,
                    screen_h,
                    search_border,
                );

                let query_empty = creative_state.search.trim().is_empty();
                let search_text = if query_empty {
                    "SEARCH...".to_string()
                } else {
                    creative_state.search.to_ascii_uppercase()
                };
                let search_color = if query_empty {
                    [0.66, 0.7, 0.78, 0.95]
                } else {
                    [0.95, 0.97, 1.0, 1.0]
                };
                let search_text_x = layout.search_bar.x + 10.0;
                let search_text_y = layout.search_bar.y + (layout.search_bar.h - 13.0) * 0.5;
                render_text_left_px(
                    &mut vertices,
                    &search_text,
                    search_text_x,
                    search_text_y,
                    1.7,
                    screen_w,
                    screen_h,
                    search_color,
                );
                let now_ms =
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
                if (now_ms / 500) % 2 == 0 {
                    let cursor_base = if query_empty {
                        0.0
                    } else {
                        text_pixel_width(&creative_state.search.to_ascii_uppercase(), 1.7)
                    };
                    create_quad_px(
                        &mut vertices,
                        search_text_x + cursor_base + 2.0,
                        search_text_y - 1.0,
                        2.0,
                        13.0,
                        screen_w,
                        screen_h,
                        [0.96, 0.97, 1.0, 0.95],
                    );
                }

                create_quad_px(
                    &mut vertices,
                    layout.clear_button.x,
                    layout.clear_button.y,
                    layout.clear_button.w,
                    layout.clear_button.h,
                    screen_w,
                    screen_h,
                    [0.75, 0.18, 0.18, 0.95],
                );
                render_text_center_px(
                    &mut vertices,
                    "X",
                    layout.clear_button.x + layout.clear_button.w * 0.5,
                    layout.clear_button.y + (layout.clear_button.h - 12.0) * 0.5,
                    2.0,
                    screen_w,
                    screen_h,
                    [1.0, 1.0, 1.0, 1.0],
                );

                for visible_idx in 0..CREATIVE_VISIBLE_SLOTS {
                    let catalog_idx = scroll * CREATIVE_COLUMNS + visible_idx;
                    let stack = creative_state.catalog.get(catalog_idx).copied().map(|item| {
                        ItemStack::new(item, max_stack_for_item(item))
                    });
                    let selected = matches!(
                        hover_target,
                        Some(CreativeHitTarget::CatalogSlot(hover_idx)) if hover_idx == catalog_idx
                    );
                    draw_slot(
                        &mut vertices,
                        layout.catalog_slots[visible_idx],
                        stack,
                        registry,
                        &self.atlas_mapping,
                        selected,
                        false,
                        screen_w,
                        screen_h,
                    );
                }

                render_text_left_px(
                    &mut vertices,
                    "HOTBAR",
                    layout.hotbar_slots[0].x,
                    layout.hotbar_slots[0].y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );
                for slot in 0..Inventory::HOTBAR_SIZE {
                    let selected = matches!(
                        hover_target,
                        Some(CreativeHitTarget::HotbarSlot(hover_slot)) if hover_slot == slot
                    ) || slot == selected_hotbar_slot;
                    draw_slot(
                        &mut vertices,
                        layout.hotbar_slots[slot],
                        inventory.hotbar_slot(slot).copied(),
                        registry,
                        &self.atlas_mapping,
                        selected,
                        true,
                        screen_w,
                        screen_h,
                    );
                }

                if let (Some(target), Some((cx, cy))) = (hover_target, cursor_position) {
                    let hovered_item = match target {
                        CreativeHitTarget::CatalogSlot(idx) => {
                            creative_state.catalog.get(idx).copied()
                        }
                        CreativeHitTarget::HotbarSlot(slot) => {
                            inventory.hotbar_slot(slot).map(|stack| stack.item)
                        }
                        CreativeHitTarget::ClearButton | CreativeHitTarget::SearchBar => None,
                    };
                    if let Some(item) = hovered_item {
                        let label = item_display_name(item, registry);
                        let label_w = text_pixel_width(&label, 1.5) + 10.0;
                        let label_h = 15.0;
                        let tooltip_x = (cx as f32 + 18.0).min(screen_w - label_w - 8.0);
                        let tooltip_y = (cy as f32 + 18.0).min(screen_h - label_h - 8.0);
                        create_quad_px(
                            &mut vertices,
                            tooltip_x,
                            tooltip_y,
                            label_w,
                            label_h,
                            screen_w,
                            screen_h,
                            [0.04, 0.04, 0.06, 0.95],
                        );
                        render_text_left_px(
                            &mut vertices,
                            &label,
                            tooltip_x + 5.0,
                            tooltip_y + 4.0,
                            1.5,
                            screen_w,
                            screen_h,
                            [0.97, 0.97, 0.99, 1.0],
                        );
                    }
                }

                if let (Some(held), Some((cx, cy))) = (cursor_stack, cursor_position) {
                    let slot_size = layout.hotbar_slots[0].w;
                    let held_rect = RectPx {
                        x: cx as f32 + 14.0,
                        y: cy as f32 + 14.0,
                        w: slot_size,
                        h: slot_size,
                    };
                    draw_slot(
                        &mut vertices,
                        held_rect,
                        Some(held),
                        registry,
                        &self.atlas_mapping,
                        true,
                        true,
                        screen_w,
                        screen_h,
                    );
                }
            } else {
                let layout = build_inventory_layout_scaled(screen_w, screen_h, crafting_mode, ui_scale);

                create_quad_px(
                    &mut vertices,
                    0.0,
                    0.0,
                    screen_w,
                    screen_h,
                    screen_w,
                    screen_h,
                    [0.0, 0.0, 0.0, 0.45],
                );

                create_quad_px(
                    &mut vertices,
                    layout.panel.x,
                    layout.panel.y,
                    layout.panel.w,
                    layout.panel.h,
                    screen_w,
                    screen_h,
                    [0.08, 0.09, 0.12, 0.92],
                );

                render_text_center_px(
                    &mut vertices,
                    match crafting_mode {
                        CraftingUiMode::Inventory2x2 => "INVENTORY",
                        CraftingUiMode::CraftingTable3x3 => "CRAFTING TABLE",
                    },
                    layout.panel.x + layout.panel.w * 0.5,
                    layout.panel.y + 16.0,
                    2.8,
                    screen_w,
                    screen_h,
                    [0.94, 0.96, 1.0, 1.0],
                );
                render_text_left_px(
                    &mut vertices,
                    "MAIN",
                    layout.slots[9].x,
                    layout.slots[9].y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );
                render_text_left_px(
                    &mut vertices,
                    "HOTBAR",
                    layout.slots[0].x,
                    layout.slots[0].y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );
                render_text_left_px(
                    &mut vertices,
                    "CRAFT",
                    layout.crafting_inputs[0].x,
                    layout.crafting_inputs[0].y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );
                render_text_left_px(
                    &mut vertices,
                    "RESULT",
                    layout.crafting_output.x,
                    layout.crafting_output.y - 18.0,
                    1.8,
                    screen_w,
                    screen_h,
                    [0.8, 0.85, 0.92, 0.95],
                );

                for slot in 0..Inventory::TOTAL_SIZE {
                    draw_slot(
                        &mut vertices,
                        layout.slots[slot],
                        inventory.get(slot).copied(),
                        registry,
                        &self.atlas_mapping,
                        slot < Inventory::HOTBAR_SIZE && slot == selected_hotbar_slot,
                        true,
                        screen_w,
                        screen_h,
                    );
                }

                for input_idx in 0..layout.crafting_input_count {
                    draw_slot(
                        &mut vertices,
                        layout.crafting_inputs[input_idx],
                        crafting_inputs.get(input_idx).copied().flatten(),
                        registry,
                        &self.atlas_mapping,
                        false,
                        false,
                        screen_w,
                        screen_h,
                    );
                }

                draw_slot(
                    &mut vertices,
                    layout.crafting_output,
                    crafting_output,
                    registry,
                    &self.atlas_mapping,
                    crafting_output.is_some(),
                    true,
                    screen_w,
                    screen_h,
                );

                if let (Some(held), Some((cx, cy))) = (cursor_stack, cursor_position) {
                    let slot_size = layout.slots[0].w;
                    let held_rect = RectPx {
                        x: cx as f32 + 14.0,
                        y: cy as f32 + 14.0,
                        w: slot_size,
                        h: slot_size,
                    };
                    draw_slot(
                        &mut vertices,
                        held_rect,
                        Some(held),
                        registry,
                        &self.atlas_mapping,
                        true,
                        true,
                        screen_w,
                        screen_h,
                    );
                }
            }
        } else {
            let slots = build_hotbar_slot_rects_scaled(screen_w, screen_h, ui_scale);
            for slot in 0..Inventory::HOTBAR_SIZE {
                draw_slot(
                    &mut vertices,
                    slots[slot],
                    inventory.hotbar_slot(slot).copied(),
                    registry,
                    &self.atlas_mapping,
                    slot == selected_hotbar_slot,
                    false,
                    screen_w,
                    screen_h,
                );
            }
        }

        let mut quad_count = vertices.len() / 4;
        if quad_count > MAX_QUADS {
            quad_count = MAX_QUADS;
            vertices.truncate(MAX_VERTICES);
        }

        let mut indices = Vec::with_capacity(quad_count * 6);
        for i in 0..quad_count {
            let base = (i * 4) as u16;
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        self.index_count = indices.len() as u32;
        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        if !indices.is_empty() {
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        if self.index_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.atlas_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

pub fn hit_test_open_target(
    cursor_x: f32,
    cursor_y: f32,
    width: u32,
    height: u32,
    gui_scale: f32,
    mode: CraftingUiMode,
    chest_open: bool,
) -> Option<OpenInventoryHitTarget> {
    if chest_open {
        let layout = build_chest_layout_scaled(
            width.max(1) as f32,
            height.max(1) as f32,
            normalized_gui_scale(gui_scale),
        );
        for (idx, rect) in layout.chest_slots.iter().enumerate() {
            if rect.contains(cursor_x, cursor_y) {
                return Some(OpenInventoryHitTarget::ChestSlot(idx));
            }
        }
        for (idx, rect) in layout.player_slots.iter().enumerate() {
            if rect.contains(cursor_x, cursor_y) {
                return Some(OpenInventoryHitTarget::InventorySlot(idx));
            }
        }
        return None;
    }

    let layout = build_inventory_layout_scaled(
        width.max(1) as f32,
        height.max(1) as f32,
        mode,
        normalized_gui_scale(gui_scale),
    );
    for (idx, rect) in layout.slots.iter().enumerate() {
        if rect.contains(cursor_x, cursor_y) {
            return Some(OpenInventoryHitTarget::InventorySlot(idx));
        }
    }
    for input_idx in 0..layout.crafting_input_count {
        if layout.crafting_inputs[input_idx].contains(cursor_x, cursor_y) {
            return Some(OpenInventoryHitTarget::CraftingInput(input_idx));
        }
    }
    if layout.crafting_output.contains(cursor_x, cursor_y) {
        return Some(OpenInventoryHitTarget::CraftingOutput);
    }
    None
}

pub fn hit_test_creative(
    cursor_x: f32,
    cursor_y: f32,
    width: f32,
    height: f32,
    gui_scale: f32,
    catalog_len: usize,
    scroll: usize,
) -> Option<CreativeHitTarget> {
    let layout = build_creative_layout_scaled(
        width.max(1.0),
        height.max(1.0),
        normalized_gui_scale(gui_scale),
    );

    if layout.clear_button.contains(cursor_x, cursor_y) {
        return Some(CreativeHitTarget::ClearButton);
    }
    if layout.search_bar.contains(cursor_x, cursor_y) {
        return Some(CreativeHitTarget::SearchBar);
    }

    for (slot, rect) in layout.hotbar_slots.iter().enumerate() {
        if rect.contains(cursor_x, cursor_y) {
            return Some(CreativeHitTarget::HotbarSlot(slot));
        }
    }

    let clamped_scroll = scroll.min(creative_max_scroll(catalog_len));
    for (visible_idx, rect) in layout.catalog_slots.iter().enumerate() {
        if !rect.contains(cursor_x, cursor_y) {
            continue;
        }
        let catalog_idx = clamped_scroll * CREATIVE_COLUMNS + visible_idx;
        if catalog_idx < catalog_len {
            return Some(CreativeHitTarget::CatalogSlot(catalog_idx));
        }
        return None;
    }

    None
}

pub fn is_creative_panel_hit(
    cursor_x: f32,
    cursor_y: f32,
    width: f32,
    height: f32,
    gui_scale: f32,
) -> bool {
    let layout = build_creative_layout_scaled(
        width.max(1.0),
        height.max(1.0),
        normalized_gui_scale(gui_scale),
    );
    layout.panel.contains(cursor_x, cursor_y)
}

pub fn is_open_inventory_panel_hit(
    cursor_x: f32,
    cursor_y: f32,
    width: u32,
    height: u32,
    gui_scale: f32,
    mode: CraftingUiMode,
    chest_open: bool,
) -> bool {
    let ui_scale = normalized_gui_scale(gui_scale);
    if chest_open {
        let layout = build_chest_layout_scaled(width.max(1) as f32, height.max(1) as f32, ui_scale);
        return layout.panel.contains(cursor_x, cursor_y);
    }

    if read_creative_ui_state().enabled {
        let layout = build_creative_layout_scaled(width.max(1) as f32, height.max(1) as f32, ui_scale);
        return layout.panel.contains(cursor_x, cursor_y);
    }

    let layout = build_inventory_layout_scaled(width.max(1) as f32, height.max(1) as f32, mode, ui_scale);
    layout.panel.contains(cursor_x, cursor_y)
}

fn build_inventory_layout(screen_w: f32, screen_h: f32, mode: CraftingUiMode) -> InventoryLayout {
    let slot_size = (screen_w.min(screen_h) * 0.07).clamp(46.0, 62.0);
    let slot_gap = (slot_size * 0.14).round().max(4.0);

    let inv_grid_w = slot_size * 9.0 + slot_gap * 8.0;
    let inv_grid_h = slot_size * 4.0 + slot_gap * 3.0;
    let craft_side = match mode {
        CraftingUiMode::Inventory2x2 => 2,
        CraftingUiMode::CraftingTable3x3 => 3,
    };
    let craft_grid_w = slot_size * craft_side as f32 + slot_gap * (craft_side as f32 - 1.0);
    let craft_grid_h = craft_grid_w;
    let craft_section_w = craft_grid_w + slot_gap * 2.0 + slot_size;
    let panel_w = inv_grid_w + craft_section_w + 72.0;
    let panel_h = (inv_grid_h + 94.0).max(craft_grid_h + 110.0);

    let panel = RectPx {
        x: (screen_w - panel_w) * 0.5,
        y: (screen_h - panel_h) * 0.5,
        w: panel_w,
        h: panel_h,
    };

    let inv_grid_x = panel.x + 24.0;
    let inv_grid_y = panel.y + panel.h - inv_grid_h - 24.0;
    let craft_x = panel.x + panel.w - craft_section_w - 24.0;
    let craft_y = panel.y + 54.0;

    let mut slots = [RectPx::default(); Inventory::TOTAL_SIZE];
    for row in 0..4 {
        for col in 0..9 {
            let slot_idx = if row < 3 {
                Inventory::HOTBAR_SIZE + row * 9 + col
            } else {
                col
            };
            slots[slot_idx] = RectPx {
                x: inv_grid_x + col as f32 * (slot_size + slot_gap),
                y: inv_grid_y + row as f32 * (slot_size + slot_gap),
                w: slot_size,
                h: slot_size,
            };
        }
    }

    let mut crafting_inputs = [RectPx::default(); MAX_CRAFTING_INPUT_SLOTS];
    let crafting_input_count = mode.input_slot_count();
    let craft_cols = craft_side;
    for idx in 0..crafting_input_count {
        let row = idx / craft_cols;
        let col = idx % craft_cols;
        crafting_inputs[idx] = RectPx {
            x: craft_x + col as f32 * (slot_size + slot_gap),
            y: craft_y + row as f32 * (slot_size + slot_gap),
            w: slot_size,
            h: slot_size,
        };
    }

    let crafting_output = RectPx {
        x: craft_x + craft_grid_w + slot_gap * 2.0,
        y: craft_y + (craft_grid_h - slot_size) * 0.5,
        w: slot_size,
        h: slot_size,
    };

    InventoryLayout {
        panel,
        slots,
        crafting_inputs,
        crafting_input_count,
        crafting_output,
    }
}

fn build_inventory_layout_scaled(
    screen_w: f32,
    screen_h: f32,
    mode: CraftingUiMode,
    gui_scale: f32,
) -> InventoryLayout {
    let scale = normalized_gui_scale(gui_scale);
    let mut layout = build_inventory_layout(screen_w / scale, screen_h / scale, mode);
    scale_inventory_layout(&mut layout, scale);
    layout
}

fn build_chest_layout(screen_w: f32, screen_h: f32) -> ChestLayout {
    let slot_size = (screen_w.min(screen_h) * 0.07).clamp(46.0, 62.0);
    let slot_gap = (slot_size * 0.14).round().max(4.0);

    let grid_w = slot_size * 9.0 + slot_gap * 8.0;
    let chest_grid_h = slot_size * 3.0 + slot_gap * 2.0;
    let player_grid_h = slot_size * 4.0 + slot_gap * 3.0;
    let panel_w = grid_w + 48.0;
    let panel_h = chest_grid_h + player_grid_h + 128.0;

    let panel = RectPx {
        x: (screen_w - panel_w) * 0.5,
        y: (screen_h - panel_h) * 0.5,
        w: panel_w,
        h: panel_h,
    };

    let chest_x = panel.x + 24.0;
    let chest_y = panel.y + 54.0;
    let player_x = panel.x + 24.0;
    let player_y = panel.y + panel.h - player_grid_h - 24.0;

    let mut chest_slots = [RectPx::default(); CHEST_SLOT_COUNT];
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = row * 9 + col;
            chest_slots[slot_idx] = RectPx {
                x: chest_x + col as f32 * (slot_size + slot_gap),
                y: chest_y + row as f32 * (slot_size + slot_gap),
                w: slot_size,
                h: slot_size,
            };
        }
    }

    let mut player_slots = [RectPx::default(); Inventory::TOTAL_SIZE];
    for row in 0..4 {
        for col in 0..9 {
            let slot_idx = if row < 3 {
                Inventory::HOTBAR_SIZE + row * 9 + col
            } else {
                col
            };
            player_slots[slot_idx] = RectPx {
                x: player_x + col as f32 * (slot_size + slot_gap),
                y: player_y + row as f32 * (slot_size + slot_gap),
                w: slot_size,
                h: slot_size,
            };
        }
    }

    ChestLayout {
        panel,
        chest_slots,
        player_slots,
    }
}

fn build_chest_layout_scaled(screen_w: f32, screen_h: f32, gui_scale: f32) -> ChestLayout {
    let scale = normalized_gui_scale(gui_scale);
    let mut layout = build_chest_layout(screen_w / scale, screen_h / scale);
    scale_chest_layout(&mut layout, scale);
    layout
}

fn build_creative_layout(screen_w: f32, screen_h: f32) -> CreativeLayout {
    let slot_size = (screen_w.min(screen_h) * 0.064).clamp(44.0, 58.0);
    let slot_gap = (slot_size * 0.14).round().max(4.0);
    let grid_w = slot_size * CREATIVE_COLUMNS as f32 + slot_gap * (CREATIVE_COLUMNS as f32 - 1.0);
    let grid_h = slot_size * CREATIVE_VISIBLE_ROWS as f32
        + slot_gap * (CREATIVE_VISIBLE_ROWS as f32 - 1.0);
    let search_h = (slot_size * 0.76).clamp(34.0, 44.0);
    let clear_button_size = search_h;
    let panel_w = grid_w + 56.0;
    let panel_h = 36.0 + search_h + 16.0 + grid_h + 28.0 + slot_size + 20.0;

    let panel = RectPx {
        x: (screen_w - panel_w) * 0.5,
        y: (screen_h - panel_h) * 0.5,
        w: panel_w,
        h: panel_h,
    };

    let search_bar = RectPx {
        x: panel.x + 22.0,
        y: panel.y + 36.0,
        w: panel.w - 22.0 * 2.0 - clear_button_size - 12.0,
        h: search_h,
    };

    let clear_button = RectPx {
        x: search_bar.x + search_bar.w + 12.0,
        y: search_bar.y,
        w: clear_button_size,
        h: clear_button_size,
    };

    let catalog_x = panel.x + 22.0;
    let catalog_y = search_bar.y + search_bar.h + 16.0;
    let mut catalog_slots = [RectPx::default(); CREATIVE_VISIBLE_SLOTS];
    for row in 0..CREATIVE_VISIBLE_ROWS {
        for col in 0..CREATIVE_COLUMNS {
            let idx = row * CREATIVE_COLUMNS + col;
            catalog_slots[idx] = RectPx {
                x: catalog_x + col as f32 * (slot_size + slot_gap),
                y: catalog_y + row as f32 * (slot_size + slot_gap),
                w: slot_size,
                h: slot_size,
            };
        }
    }

    let hotbar_y = panel.y + panel.h - slot_size - 20.0;
    let mut hotbar_slots = [RectPx::default(); Inventory::HOTBAR_SIZE];
    for col in 0..Inventory::HOTBAR_SIZE {
        hotbar_slots[col] = RectPx {
            x: catalog_x + col as f32 * (slot_size + slot_gap),
            y: hotbar_y,
            w: slot_size,
            h: slot_size,
        };
    }

    CreativeLayout {
        panel,
        search_bar,
        clear_button,
        catalog_slots,
        hotbar_slots,
    }
}

fn build_creative_layout_scaled(screen_w: f32, screen_h: f32, gui_scale: f32) -> CreativeLayout {
    let scale = normalized_gui_scale(gui_scale);
    let mut layout = build_creative_layout(screen_w / scale, screen_h / scale);
    scale_creative_layout(&mut layout, scale);
    layout
}

fn build_hotbar_slot_rects(screen_w: f32, screen_h: f32) -> [RectPx; Inventory::HOTBAR_SIZE] {
    let total_w = HOTBAR_SLOT_SIZE_PX * 9.0 + HOTBAR_SLOT_GAP_PX * 8.0;
    let start_x = (screen_w - total_w) * 0.5;
    let y = screen_h - HOTBAR_MARGIN_BOTTOM_PX - HOTBAR_SLOT_SIZE_PX;

    let mut slots = [RectPx::default(); Inventory::HOTBAR_SIZE];
    for (i, rect) in slots.iter_mut().enumerate() {
        *rect = RectPx {
            x: start_x + i as f32 * (HOTBAR_SLOT_SIZE_PX + HOTBAR_SLOT_GAP_PX),
            y,
            w: HOTBAR_SLOT_SIZE_PX,
            h: HOTBAR_SLOT_SIZE_PX,
        };
    }
    slots
}

fn build_hotbar_slot_rects_scaled(
    screen_w: f32,
    screen_h: f32,
    gui_scale: f32,
) -> [RectPx; Inventory::HOTBAR_SIZE] {
    let scale = normalized_gui_scale(gui_scale);
    let mut slots = build_hotbar_slot_rects(screen_w / scale, screen_h / scale);
    for rect in &mut slots {
        *rect = scale_rect(*rect, scale);
    }
    slots
}

fn draw_slot(
    vertices: &mut Vec<UiVertex>,
    rect: RectPx,
    stack: Option<ItemStack>,
    registry: Option<&BlockRegistry>,
    atlas_mapping: &AtlasMapping,
    selected: bool,
    show_name: bool,
    screen_w: f32,
    screen_h: f32,
) {
    create_quad_px(
        vertices,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        screen_w,
        screen_h,
        [0.15, 0.16, 0.2, 0.82],
    );

    let border = if selected {
        [0.95, 0.82, 0.24, 1.0]
    } else {
        [0.32, 0.34, 0.41, 0.94]
    };
    create_quad_px(
        vertices,
        rect.x,
        rect.y,
        rect.w,
        SLOT_BORDER_PX,
        screen_w,
        screen_h,
        border,
    );
    create_quad_px(
        vertices,
        rect.x,
        rect.y + rect.h - SLOT_BORDER_PX,
        rect.w,
        SLOT_BORDER_PX,
        screen_w,
        screen_h,
        border,
    );
    create_quad_px(
        vertices,
        rect.x,
        rect.y,
        SLOT_BORDER_PX,
        rect.h,
        screen_w,
        screen_h,
        border,
    );
    create_quad_px(
        vertices,
        rect.x + rect.w - SLOT_BORDER_PX,
        rect.y,
        SLOT_BORDER_PX,
        rect.h,
        screen_w,
        screen_h,
        border,
    );

    let Some(stack) = stack else {
        return;
    };

    let icon_size = (rect.w - 12.0).max(14.0);
    let icon_x = rect.x + (rect.w - icon_size) * 0.5;
    let icon_y = rect.y + (rect.h - icon_size) * 0.5;

    // Try to render as 3D isometric textured block
    let rendered_3d = if let Some(block) = stack.item.as_block_id() {
        if let Some(tile_origin) = atlas_mapping.offset(block) {
            let tint = block_tint_for_item(stack, registry);
            draw_isometric_block(vertices, icon_x, icon_y, icon_size, screen_w, screen_h, tile_origin, tint);
            true
        } else {
            false
        }
    } else {
        false
    };

    if !rendered_3d {
        if let Some(tile_origin) = atlas_mapping.offset_for_item(stack.item) {
            draw_item_sprite(
                vertices,
                icon_x,
                icon_y,
                icon_size,
                screen_w,
                screen_h,
                tile_origin,
            );
        } else {
            create_quad_px(
                vertices,
                icon_x,
                icon_y,
                icon_size,
                icon_size,
                screen_w,
                screen_h,
                block_color_from_item(stack, registry),
            );
        }
    }

    if show_name {
        let name = truncate_text_chars(&item_name(stack, registry), 9);
        render_text_left_px(
            vertices,
            &name,
            rect.x + 4.0,
            rect.y + 4.0,
            1.2,
            screen_w,
            screen_h,
            [0.96, 0.96, 0.97, 0.96],
        );
    }

    if stack.count > 1 {
        let count = stack.count.to_string();
        let w = text_pixel_width(&count, 1.5);
        render_text_left_px(
            vertices,
            &count,
            rect.x + rect.w - w - 3.0,
            rect.y + rect.h - 13.0,
            1.5,
            screen_w,
            screen_h,
            [0.98, 0.98, 0.99, 1.0],
        );
    }
}

fn draw_isometric_block(
    vertices: &mut Vec<UiVertex>,
    ix: f32,
    iy: f32,
    icon_size: f32,
    screen_w: f32,
    screen_h: f32,
    tile_origin: [f32; 2],
    tint: [f32; 3],
) {
    let s = icon_size * 0.85;
    let cx = ix + icon_size * 0.5;
    let cy = iy + icon_size * 0.5;
    let hw = s * 0.5;
    let hh = s * 0.25;

    // 7 vertices of the isometric cube
    let top = [cx, cy - hh * 2.0];
    let left = [cx - hw, cy - hh];
    let right = [cx + hw, cy - hh];
    let near = [cx, cy];
    let bottom_left = [cx - hw, cy + hh];
    let bottom_right = [cx + hw, cy + hh];
    let bottom = [cx, cy + hh * 2.0];

    // Top face (brightest)
    let b = 1.0;
    push_textured_iso_face(
        vertices,
        [left, near, right, top],
        screen_w,
        screen_h,
        [tint[0] * b, tint[1] * b, tint[2] * b, 1.0],
        tile_origin,
    );

    // Left face (medium shade)
    let b = 0.65;
    push_textured_iso_face(
        vertices,
        [bottom_left, bottom, near, left],
        screen_w,
        screen_h,
        [tint[0] * b, tint[1] * b, tint[2] * b, 1.0],
        tile_origin,
    );

    // Right face (darkest)
    let b = 0.45;
    push_textured_iso_face(
        vertices,
        [bottom, bottom_right, right, near],
        screen_w,
        screen_h,
        [tint[0] * b, tint[1] * b, tint[2] * b, 1.0],
        tile_origin,
    );
}

fn draw_item_sprite(
    vertices: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    size: f32,
    screen_w: f32,
    screen_h: f32,
    tile_origin: [f32; 2],
) {
    let corners = [[x, y + size], [x + size, y + size], [x + size, y], [x, y]];
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    for i in 0..4 {
        let (nx, ny) = screen_to_ndc(corners[i][0], corners[i][1], screen_w, screen_h);
        vertices.push(UiVertex {
            position: [nx, ny],
            color: [1.0, 1.0, 1.0, 1.0],
            tex_coord: uvs[i],
            tile_origin,
            use_texture: 1.0,
        });
    }
}

fn push_textured_iso_face(
    vertices: &mut Vec<UiVertex>,
    corners: [[f32; 2]; 4],
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
    tile_origin: [f32; 2],
) {
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    for i in 0..4 {
        let (nx, ny) = screen_to_ndc(corners[i][0], corners[i][1], screen_w, screen_h);
        vertices.push(UiVertex {
            position: [nx, ny],
            color,
            tex_coord: uvs[i],
            tile_origin,
            use_texture: 1.0,
        });
    }
}

fn block_tint_for_item(stack: ItemStack, registry: Option<&BlockRegistry>) -> [f32; 3] {
    let Some(block) = stack.item.as_block_id() else {
        return [1.0, 1.0, 1.0];
    };
    let name = registry
        .map(|r| r.get_properties(block).name.as_str())
        .unwrap_or("unknown");
    match name {
        "verdant_turf" | "canopy_leaves" | "tall_grass" | "wildflower" | "sapling"
        | "sugar_cane" => [0.48, 0.74, 0.32],
        _ => [1.0, 1.0, 1.0],
    }
}

fn block_color_from_item(stack: ItemStack, registry: Option<&BlockRegistry>) -> [f32; 4] {
    let Some(block) = stack.item.as_block_id() else {
        return match stack.item {
            ItemId::STICK => [0.76, 0.61, 0.39, 1.0],
            ItemId::WOODEN_PICKAXE => [0.7, 0.51, 0.31, 1.0],
            ItemId::WOODEN_SWORD => [0.74, 0.56, 0.33, 1.0],
            _ => [0.62, 0.62, 0.62, 1.0],
        };
    };
    let name = registry
        .map(|r| r.get_properties(block).name.as_str())
        .unwrap_or("unknown");

    if name == "still_water" || name.starts_with("flowing_water_") {
        return [0.3, 0.48, 0.8, 1.0];
    }
    if name == "lava_source" || name.starts_with("flowing_lava_") {
        return [0.95, 0.36, 0.08, 1.0];
    }

    let rgb = match name {
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
        "netherite_block" | "polished_blackstone" => [0.3, 0.3, 0.34],
        "dripstone_block" => [0.57, 0.45, 0.32],
        "crafting_table" => [0.61, 0.45, 0.27],
        "furnace" => [0.43, 0.44, 0.46],
        "chest" => [0.66, 0.49, 0.3],
        _ => [0.62, 0.62, 0.62],
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

pub fn item_display_name(item: ItemId, registry: Option<&BlockRegistry>) -> String {
    if let Some(display_name) = item.display_name() {
        return display_name.to_ascii_uppercase();
    }

    match item {
        ItemId::STICK => "STICK".to_string(),
        ItemId::WOODEN_PICKAXE => "WOODEN PICKAXE".to_string(),
        ItemId::WOODEN_SWORD => "WOODEN SWORD".to_string(),
        ItemId::STONE_PICKAXE => "STONE PICKAXE".to_string(),
        ItemId::STONE_SWORD => "STONE SWORD".to_string(),
        ItemId::STONE_SHOVEL => "STONE SHOVEL".to_string(),
        ItemId::STONE_AXE => "STONE AXE".to_string(),
        ItemId::IRON_PICKAXE => "IRON PICKAXE".to_string(),
        ItemId::IRON_SWORD => "IRON SWORD".to_string(),
        ItemId::IRON_SHOVEL => "IRON SHOVEL".to_string(),
        ItemId::IRON_AXE => "IRON AXE".to_string(),
        ItemId::DIAMOND_PICKAXE => "DIAMOND PICKAXE".to_string(),
        ItemId::DIAMOND_SWORD => "DIAMOND SWORD".to_string(),
        ItemId::DIAMOND_SHOVEL => "DIAMOND SHOVEL".to_string(),
        ItemId::DIAMOND_AXE => "DIAMOND AXE".to_string(),
        ItemId::WOODEN_SHOVEL => "WOODEN SHOVEL".to_string(),
        ItemId::WOODEN_AXE => "WOODEN AXE".to_string(),
        ItemId::WOODEN_HOE => "WOODEN HOE".to_string(),
        ItemId::STONE_HOE => "STONE HOE".to_string(),
        ItemId::IRON_HOE => "IRON HOE".to_string(),
        ItemId::DIAMOND_HOE => "DIAMOND HOE".to_string(),
        ItemId::IRON_INGOT => "IRON INGOT".to_string(),
        ItemId::GOLD_INGOT => "GOLD INGOT".to_string(),
        ItemId::DIAMOND_GEM => "DIAMOND GEM".to_string(),
        ItemId::COAL => "COAL".to_string(),
        ItemId::WHEAT_ITEM => "WHEAT".to_string(),
        ItemId::WHEAT_SEEDS => "WHEAT SEEDS".to_string(),
        ItemId::BREAD => "BREAD".to_string(),
        ItemId::COPPER_INGOT => "COPPER INGOT".to_string(),
        ItemId::IRON_HELMET => "IRON HELMET".to_string(),
        ItemId::IRON_CHESTPLATE => "IRON CHESTPLATE".to_string(),
        ItemId::IRON_LEGGINGS => "IRON LEGGINGS".to_string(),
        ItemId::IRON_BOOTS => "IRON BOOTS".to_string(),
        ItemId::DIAMOND_HELMET => "DIAMOND HELMET".to_string(),
        ItemId::DIAMOND_CHESTPLATE => "DIAMOND CHESTPLATE".to_string(),
        ItemId::DIAMOND_LEGGINGS => "DIAMOND LEGGINGS".to_string(),
        ItemId::DIAMOND_BOOTS => "DIAMOND BOOTS".to_string(),
        ItemId::BONE_MEAL => "BONE MEAL".to_string(),
        other => {
            let Some(block) = other.as_block_id() else {
                return format!("ITEM {}", item.0);
            };
            let raw = registry
                .map(|r| r.get_properties(block).name.as_str())
                .unwrap_or("unknown");
            raw.replace('_', " ").to_ascii_uppercase()
        }
    }
}

fn item_name(stack: ItemStack, registry: Option<&BlockRegistry>) -> String {
    item_display_name(stack.item, registry)
}

fn truncate_text_chars(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

fn text_pixel_width(text: &str, pixel_scale: f32) -> f32 {
    if text.is_empty() {
        0.0
    } else {
        text.chars().count() as f32 * 6.0 * pixel_scale - pixel_scale
    }
}

fn create_quad_px(
    vertices: &mut Vec<UiVertex>,
    x_px: f32,
    y_px: f32,
    w_px: f32,
    h_px: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let (x0, y0) = screen_to_ndc(x_px, y_px, screen_w, screen_h);
    let (x1, y1) = screen_to_ndc(x_px + w_px, y_px + h_px, screen_w, screen_h);

    vertices.extend_from_slice(&[
        UiVertex {
            position: [x0, y1],
            color,
            tex_coord: [0.0, 0.0],
            tile_origin: [0.0, 0.0],
            use_texture: 0.0,
        },
        UiVertex {
            position: [x1, y1],
            color,
            tex_coord: [0.0, 0.0],
            tile_origin: [0.0, 0.0],
            use_texture: 0.0,
        },
        UiVertex {
            position: [x1, y0],
            color,
            tex_coord: [0.0, 0.0],
            tile_origin: [0.0, 0.0],
            use_texture: 0.0,
        },
        UiVertex {
            position: [x0, y0],
            color,
            tex_coord: [0.0, 0.0],
            tile_origin: [0.0, 0.0],
            use_texture: 0.0,
        },
    ]);
}

fn render_text_left_px(
    vertices: &mut Vec<UiVertex>,
    text: &str,
    origin_x_px: f32,
    origin_y_px: f32,
    pixel_scale: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let mut x_px = origin_x_px;
    let char_stride = 6.0 * pixel_scale;

    for ch in text.chars() {
        let ch = ch.to_ascii_uppercase();
        let Some(rows) = glyph(ch) else {
            x_px += char_stride;
            continue;
        };

        for (row_idx, row_bits) in rows.iter().enumerate() {
            for col in 0..5 {
                if (row_bits & (0x10 >> col)) == 0 {
                    continue;
                }
                create_quad_px(
                    vertices,
                    x_px + col as f32 * pixel_scale,
                    origin_y_px + row_idx as f32 * pixel_scale,
                    pixel_scale,
                    pixel_scale,
                    screen_w,
                    screen_h,
                    color,
                );
            }
        }
        x_px += char_stride;
    }
}

fn render_text_center_px(
    vertices: &mut Vec<UiVertex>,
    text: &str,
    center_x_px: f32,
    origin_y_px: f32,
    pixel_scale: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let width = text_pixel_width(text, pixel_scale);
    render_text_left_px(
        vertices,
        text,
        center_x_px - width * 0.5,
        origin_y_px,
        pixel_scale,
        screen_w,
        screen_h,
        color,
    );
}

fn screen_to_ndc(x_px: f32, y_px: f32, screen_w: f32, screen_h: f32) -> (f32, f32) {
    ((x_px / screen_w) * 2.0 - 1.0, 1.0 - (y_px / screen_h) * 2.0)
}

fn glyph(ch: char) -> Option<[u8; 7]> {
    Some(match ch {
        'A' => [0x04, 0x0A, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0E, 0x11, 0x10, 0x0E, 0x01, 0x11, 0x0E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        '3' => [0x0E, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        _ => return None,
    })
}
