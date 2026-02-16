use std::mem;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const BUTTON_WIDTH_PX: f32 = 400.0;
const BUTTON_HEIGHT_PX: f32 = 60.0;
const BUTTON_SPACING_PX: f32 = 20.0;
const TITLE_TOP_MARGIN_PX: f32 = 100.0;
const IP_FIELD_WIDTH_PX: f32 = 400.0;
const IP_FIELD_HEIGHT_PX: f32 = 40.0;
const IP_FIELD_MARGIN_PX: f32 = 20.0;

const WORLD_PANEL_TOP_PX: f32 = 156.0;
const WORLD_PANEL_PADDING_PX: f32 = 14.0;
const WORLD_ROW_SPACING_PX: f32 = 10.0;
const WORLD_MIN_ROW_HEIGHT_PX: f32 = 52.0;
const WORLD_ACTION_BUTTON_HEIGHT_PX: f32 = 50.0;

const MODAL_FIELD_HEIGHT_PX: f32 = 44.0;
const PAUSE_PANEL_WIDTH_PX: f32 = 520.0;
const PAUSE_PANEL_HEIGHT_PX: f32 = 360.0;
const PAUSE_BUTTON_HEIGHT_PX: f32 = 58.0;
const PAUSE_BUTTON_SPACING_PX: f32 = 16.0;
const SETTINGS_PANEL_WIDTH_PX: f32 = 980.0;
const SETTINGS_PANEL_HEIGHT_PX: f32 = 700.0;
const SETTINGS_ROW_HEIGHT_PX: f32 = 54.0;
const SETTINGS_ROW_SPACING_PX: f32 = 10.0;
const SETTINGS_SLIDER_HEIGHT_PX: f32 = 18.0;
const SETTINGS_BACK_BUTTON_HEIGHT_PX: f32 = 48.0;

const BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.9];
const TITLE_BG_COLOR: [f32; 4] = [0.08, 0.16, 0.19, 0.92];
const BUTTON_NORMAL_COLOR: [f32; 4] = [0.12, 0.22, 0.27, 0.92];
const BUTTON_SELECTED_COLOR: [f32; 4] = [0.2, 0.44, 0.5, 1.0];
const BUTTON_DISABLED_COLOR: [f32; 4] = [0.11, 0.16, 0.2, 0.78];
const IP_FIELD_COLOR: [f32; 4] = [0.1, 0.19, 0.24, 0.94];

const WORLD_PANEL_COLOR: [f32; 4] = [0.05, 0.11, 0.15, 0.95];
const WORLD_ENTRY_COLOR: [f32; 4] = [0.11, 0.2, 0.24, 0.9];
const WORLD_ENTRY_SELECTED_COLOR: [f32; 4] = [0.2, 0.43, 0.49, 0.98];
const WORLD_DETAILS_TEXT_COLOR: [f32; 4] = [0.76, 0.9, 0.9, 1.0];
const WORLD_OVERLAY_COLOR: [f32; 4] = [0.01, 0.02, 0.03, 0.78];
const MODAL_COLOR: [f32; 4] = [0.08, 0.13, 0.18, 0.97];
const MODAL_FIELD_COLOR: [f32; 4] = [0.12, 0.2, 0.25, 0.97];
const MODAL_FIELD_ACTIVE_COLOR: [f32; 4] = [0.2, 0.42, 0.47, 0.99];
const DELETE_CONFIRM_BUTTON_COLOR: [f32; 4] = [0.66, 0.22, 0.22, 0.96];
const SLIDER_TRACK_COLOR: [f32; 4] = [0.1, 0.17, 0.2, 0.97];
const SLIDER_FILL_COLOR: [f32; 4] = [0.24, 0.64, 0.67, 1.0];
const SLIDER_KNOB_COLOR: [f32; 4] = [0.94, 0.97, 1.0, 1.0];
const TOGGLE_ON_COLOR: [f32; 4] = [0.2, 0.58, 0.32, 0.96];
const TOGGLE_OFF_COLOR: [f32; 4] = [0.38, 0.22, 0.22, 0.96];

const TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

const TITLE_PIXEL_SCALE: f32 = 8.0;
const BUTTON_TEXT_PIXEL_SCALE: f32 = 4.0;
const IP_TEXT_PIXEL_SCALE: f32 = 4.0;
const WORLD_TITLE_PIXEL_SCALE: f32 = 4.2;
const WORLD_NAME_PIXEL_SCALE: f32 = 3.0;
const WORLD_META_PIXEL_SCALE: f32 = 2.0;
const ACTION_BUTTON_PIXEL_SCALE: f32 = 2.5;
const MODAL_TITLE_PIXEL_SCALE: f32 = 3.8;
const MODAL_LABEL_PIXEL_SCALE: f32 = 2.3;
const MODAL_INPUT_PIXEL_SCALE: f32 = 2.6;
const SETTINGS_LABEL_PIXEL_SCALE: f32 = 2.5;
const SETTINGS_VALUE_PIXEL_SCALE: f32 = 2.3;

const MAX_QUADS: usize = 16000;
const MAX_VERTICES: usize = MAX_QUADS * 4;
const MAX_INDICES: usize = MAX_QUADS * 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldCreateInputField {
    Name,
    Seed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldSelectHitTarget {
    WorldEntry(usize),
    CreateNewWorld,
    PlaySelected,
    DeleteSelected,
    Back,
    CreateNameField,
    CreateSeedField,
    CreatePlayModeToggle,
    CreateConfirm,
    CreateCancel,
    DeleteConfirm,
    DeleteCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseMenuHitTarget {
    Resume,
    Settings,
    SaveAndQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSliderKind {
    RenderDistance,
    SurfaceBelow,
    FlightBelow,
    StreamAbove,
    LodDistance,
    MouseSensitivity,
    Fov,
    GuiScale,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsHitTarget {
    Slider(SettingsSliderKind, f32),
    ShowFpsToggle,
    Back,
}

#[derive(Debug, Clone, Copy)]
pub struct WorldListEntryView<'a> {
    pub name: &'a str,
    pub seed: u64,
    pub size_label: &'a str,
    pub last_opened_label: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct WorldSelectView<'a> {
    pub worlds: &'a [WorldListEntryView<'a>],
    pub selected_world: Option<usize>,
    pub create_form_open: bool,
    pub create_name_input: &'a str,
    pub create_seed_input: &'a str,
    pub create_play_mode_label: &'a str,
    pub create_play_mode_is_creative: bool,
    pub active_input_field: WorldCreateInputField,
    pub delete_confirmation_open: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsMenuView {
    pub render_distance: i32,
    pub stream_surface_below: i32,
    pub stream_flight_below: i32,
    pub stream_above: i32,
    pub lod1_distance: i32,
    pub mouse_sensitivity: f32,
    pub fov: f32,
    pub gui_scale: f32,
    pub show_fps: bool,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UiVertex {
    position: [f32; 2],
    color: [f32; 4],
    effect: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct MainMenuUniform {
    resolution: [f32; 2],
    time: f32,
    _pad: f32,
}

#[derive(Debug, Clone, Copy)]
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

    fn center_x(self) -> f32 {
        self.x + self.w * 0.5
    }

    fn center_y(self) -> f32 {
        self.y + self.h * 0.5
    }
}

struct WorldSelectLayout {
    panel: RectPx,
    world_rows: Vec<(usize, RectPx)>,
    create_button: RectPx,
    play_button: RectPx,
    delete_button: RectPx,
    back_button: RectPx,
}

struct CreateWorldModalLayout {
    panel: RectPx,
    name_field: RectPx,
    seed_field: RectPx,
    play_mode_toggle: RectPx,
    create_button: RectPx,
    cancel_button: RectPx,
}

struct DeleteWorldModalLayout {
    panel: RectPx,
    delete_button: RectPx,
    cancel_button: RectPx,
}

struct PauseMenuLayout {
    panel: RectPx,
    resume_button: RectPx,
    settings_button: RectPx,
    save_quit_button: RectPx,
}

struct SettingsMenuLayout {
    panel: RectPx,
    render_distance_slider: RectPx,
    stream_surface_below_slider: RectPx,
    stream_flight_below_slider: RectPx,
    stream_above_slider: RectPx,
    lod_distance_slider: RectPx,
    mouse_sensitivity_slider: RectPx,
    fov_slider: RectPx,
    gui_scale_slider: RectPx,
    show_fps_toggle: RectPx,
    back_button: RectPx,
    label_x: f32,
    value_x: f32,
}

pub struct MainMenuRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    menu_uniform_buffer: wgpu::Buffer,
    menu_bind_group: wgpu::BindGroup,
    start_time: Instant,
    num_indices: u32,
}

impl MainMenuRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Main Menu Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/main_menu.wgsl"
                ))
                .into(),
            ),
        });

        let menu_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Main Menu Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Menu Pipeline Layout"),
            bind_group_layouts: &[&menu_bind_group_layout],
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
                format: wgpu::VertexFormat::Float32,
            },
        ];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Main Menu Pipeline"),
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
                    format,
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
            label: Some("Main Menu Vertex Buffer"),
            size: (MAX_VERTICES * mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Main Menu Index Buffer"),
            size: (MAX_INDICES * mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let menu_uniform = MainMenuUniform {
            resolution: [width.max(1) as f32, height.max(1) as f32],
            time: 0.0,
            _pad: 0.0,
        };
        let menu_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Main Menu Uniform Buffer"),
            contents: bytemuck::bytes_of(&menu_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let menu_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Main Menu Bind Group"),
            layout: &menu_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: menu_uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            menu_uniform_buffer,
            menu_bind_group,
            start_time: Instant::now(),
            num_indices: 0,
        }
    }

    fn update_uniform(&mut self, queue: &wgpu::Queue, width: u32, height: u32) -> (f32, f32) {
        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let menu_uniform = MainMenuUniform {
            resolution: [screen_w, screen_h],
            time: self.start_time.elapsed().as_secs_f32(),
            _pad: 0.0,
        };
        queue.write_buffer(
            &self.menu_uniform_buffer,
            0,
            bytemuck::bytes_of(&menu_uniform),
        );
        (screen_w, screen_h)
    }

    fn write_geometry(&mut self, queue: &wgpu::Queue, mut vertices: Vec<UiVertex>) {
        let mut quad_count = vertices.len() / 4;
        if quad_count > MAX_QUADS {
            quad_count = MAX_QUADS;
            vertices.truncate(MAX_VERTICES);
        }

        let mut indices: Vec<u16> = Vec::with_capacity(quad_count * 6);
        for i in 0..quad_count {
            let base = (i * 4) as u16;
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        self.num_indices = indices.len() as u32;

        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        if !indices.is_empty() {
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));
        }
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        selected_item: u8,
        show_ip_field: bool,
        server_ip: &str,
    ) {
        let (screen_w, screen_h) = self.update_uniform(queue, width, height);

        let mut vertices = Vec::with_capacity(MAX_VERTICES.min(4096));

        create_quad_with_effect(&mut vertices, -1.0, 1.0, 2.0, 2.0, BACKGROUND_COLOR, 1.0);

        let title_w = ndc_from_px_x(620.0, screen_w);
        let title_h = ndc_from_px_y(108.0, screen_h);
        let title_top = 1.0 - ndc_from_px_y(TITLE_TOP_MARGIN_PX - 24.0, screen_h);
        create_quad(
            &mut vertices,
            -title_w / 2.0,
            title_top,
            title_w,
            title_h,
            TITLE_BG_COLOR,
        );

        let title_center_y = title_top - title_h / 2.0;
        render_text(
            &mut vertices,
            "VELDSPAR",
            0.0,
            title_center_y,
            TITLE_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        render_text(
            &mut vertices,
            "CRAFT  SURVIVE  EXPLORE",
            0.0,
            title_center_y - ndc_from_px_y(34.0, screen_h),
            2.2,
            screen_w,
            screen_h,
            [0.78, 0.9, 0.88, 1.0],
        );

        let button_w = ndc_from_px_x(BUTTON_WIDTH_PX, screen_w);
        let button_h = ndc_from_px_y(BUTTON_HEIGHT_PX, screen_h);
        let button_spacing = ndc_from_px_y(BUTTON_SPACING_PX, screen_h);
        let total_h = button_h * 3.0 + button_spacing * 2.0;
        let start_y = total_h / 2.0;

        let panel_pad = ndc_from_px_x(22.0, screen_w);
        let panel_h = total_h + ndc_from_px_y(42.0, screen_h);
        create_quad(
            &mut vertices,
            -button_w / 2.0 - panel_pad,
            start_y + ndc_from_px_y(18.0, screen_h),
            button_w + panel_pad * 2.0,
            panel_h,
            [0.05, 0.1, 0.13, 0.76],
        );

        let labels = ["PLAY WORLD", "JOIN SERVER", "EXIT GAME"];
        for i in 0..3u8 {
            let button_top = start_y - (i as f32) * (button_h + button_spacing);
            let color = if i == selected_item {
                BUTTON_SELECTED_COLOR
            } else {
                BUTTON_NORMAL_COLOR
            };

            create_quad(
                &mut vertices,
                -button_w / 2.0,
                button_top,
                button_w,
                button_h,
                color,
            );

            if i == selected_item {
                let stripe_w = ndc_from_px_x(8.0, screen_w);
                create_quad(
                    &mut vertices,
                    -button_w / 2.0,
                    button_top,
                    stripe_w,
                    button_h,
                    [0.88, 0.97, 0.88, 0.95],
                );
            }

            let button_center_y = button_top - button_h / 2.0;
            render_text(
                &mut vertices,
                labels[i as usize],
                0.0,
                button_center_y,
                BUTTON_TEXT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
        }

        if show_ip_field {
            let ip_w = ndc_from_px_x(IP_FIELD_WIDTH_PX, screen_w);
            let ip_h = ndc_from_px_y(IP_FIELD_HEIGHT_PX, screen_h);
            let ip_margin = ndc_from_px_y(IP_FIELD_MARGIN_PX, screen_h);
            let ip_top = start_y - (button_h + button_spacing) - button_h - ip_margin;

            create_quad(
                &mut vertices,
                -ip_w / 2.0,
                ip_top,
                ip_w,
                ip_h,
                IP_FIELD_COLOR,
            );

            let ip_center_y = ip_top - ip_h / 2.0;
            render_text(
                &mut vertices,
                server_ip,
                0.0,
                ip_center_y,
                IP_TEXT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
        }

        render_text(
            &mut vertices,
            "F4: FLY MODE  |  C: CREATIVE/SURVIVAL",
            0.0,
            -1.0 + ndc_from_px_y(24.0, screen_h),
            1.8,
            screen_w,
            screen_h,
            [0.71, 0.86, 0.84, 1.0],
        );

        self.write_geometry(queue, vertices);
    }

    pub fn update_world_select(
        &mut self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        view: &WorldSelectView<'_>,
    ) {
        let (screen_w, screen_h) = self.update_uniform(queue, width, height);
        let selected_world = view.selected_world.filter(|idx| *idx < view.worlds.len());
        let layout = build_world_select_layout(screen_w, screen_h, view.worlds.len(), selected_world);

        let mut vertices = Vec::with_capacity(MAX_VERTICES.min(12000));

        create_quad_with_effect(&mut vertices, -1.0, 1.0, 2.0, 2.0, BACKGROUND_COLOR, 1.0);

        let title_w = ndc_from_px_x(640.0, screen_w);
        let title_h = ndc_from_px_y(72.0, screen_h);
        let title_top = 1.0 - ndc_from_px_y(TITLE_TOP_MARGIN_PX, screen_h);
        create_quad(
            &mut vertices,
            -title_w / 2.0,
            title_top,
            title_w,
            title_h,
            TITLE_BG_COLOR,
        );

        let title_center_y = title_top - title_h / 2.0;
        render_text(
            &mut vertices,
            "WORLD SELECTOR",
            0.0,
            title_center_y,
            WORLD_TITLE_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        let worlds_label = format!("{} WORLDS", view.worlds.len());
        render_text(
            &mut vertices,
            &worlds_label,
            0.0,
            title_center_y - ndc_from_px_y(26.0, screen_h),
            2.2,
            screen_w,
            screen_h,
            [0.72, 0.88, 0.9, 1.0],
        );

        create_quad_px(
            &mut vertices,
            layout.panel.x,
            layout.panel.y,
            layout.panel.w,
            layout.panel.h,
            screen_w,
            screen_h,
            WORLD_PANEL_COLOR,
        );
        render_text_left_px(
            &mut vertices,
            "LOCAL WORLDS",
            layout.panel.x + WORLD_PANEL_PADDING_PX,
            layout.panel.y + 16.0,
            MODAL_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        let action_panel = RectPx {
            x: layout.create_button.x - 10.0,
            y: layout.panel.y,
            w: layout.create_button.w + 20.0,
            h: layout.panel.h,
        };
        create_quad_px(
            &mut vertices,
            action_panel.x,
            action_panel.y,
            action_panel.w,
            action_panel.h,
            screen_w,
            screen_h,
            [0.07, 0.13, 0.17, 0.94],
        );
        render_text_center_px(
            &mut vertices,
            "ACTIONS",
            action_panel.center_x(),
            action_panel.y + 26.0,
            MODAL_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        if layout.world_rows.is_empty() {
            render_text_center_px(
                &mut vertices,
                "NO WORLDS FOUND",
                layout.panel.center_x(),
                layout.panel.center_y(),
                WORLD_NAME_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
        } else {
            for (world_idx, row_rect) in &layout.world_rows {
                let world = view.worlds[*world_idx];
                let is_selected = selected_world == Some(*world_idx);
                let row_color = if is_selected {
                    WORLD_ENTRY_SELECTED_COLOR
                } else {
                    WORLD_ENTRY_COLOR
                };

                create_quad_px(
                    &mut vertices,
                    row_rect.x,
                    row_rect.y,
                    row_rect.w,
                    row_rect.h,
                    screen_w,
                    screen_h,
                    row_color,
                );
                if is_selected {
                    create_quad_px(
                        &mut vertices,
                        row_rect.x,
                        row_rect.y,
                        5.0,
                        row_rect.h,
                        screen_w,
                        screen_h,
                        [0.84, 0.96, 0.88, 0.95],
                    );
                }

                let icon_size = (row_rect.h - 16.0).clamp(26.0, 44.0);
                let icon_y = row_rect.y + (row_rect.h - icon_size) * 0.5;
                create_quad_px(
                    &mut vertices,
                    row_rect.x + 10.0,
                    icon_y,
                    icon_size,
                    icon_size,
                    screen_w,
                    screen_h,
                    seed_preview_color(world.seed),
                );

                let title_text = truncate_text(world.name, 26);
                render_text_left_px(
                    &mut vertices,
                    &title_text,
                    row_rect.x + icon_size + 24.0,
                    row_rect.y + 9.0,
                    WORLD_NAME_PIXEL_SCALE,
                    screen_w,
                    screen_h,
                    TEXT_COLOR,
                );

                let details = format!(
                    "SEED {}   SIZE {}   LAST {}",
                    world.seed,
                    world.size_label,
                    world.last_opened_label
                );
                let details = truncate_text(&details, 68);
                render_text_left_px(
                    &mut vertices,
                    &details,
                    row_rect.x + icon_size + 24.0,
                    row_rect.y + row_rect.h - 19.0,
                    WORLD_META_PIXEL_SCALE,
                    screen_w,
                    screen_h,
                    WORLD_DETAILS_TEXT_COLOR,
                );
            }
        }

        if let Some(selected) = selected_world {
            let selected_world_name = truncate_text(view.worlds[selected].name, 22);
            render_text_center_px(
                &mut vertices,
                "SELECTED",
                action_panel.center_x(),
                action_panel.y + 60.0,
                WORLD_META_PIXEL_SCALE,
                screen_w,
                screen_h,
                WORLD_DETAILS_TEXT_COLOR,
            );
            render_text_center_px(
                &mut vertices,
                &selected_world_name,
                action_panel.center_x(),
                action_panel.y + 82.0,
                MODAL_INPUT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
        }

        let has_selection = selected_world.is_some();
        draw_button(
            &mut vertices,
            layout.create_button,
            "NEW WORLD",
            BUTTON_NORMAL_COLOR,
            ACTION_BUTTON_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_button(
            &mut vertices,
            layout.play_button,
            "ENTER",
            if has_selection {
                BUTTON_SELECTED_COLOR
            } else {
                BUTTON_DISABLED_COLOR
            },
            ACTION_BUTTON_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_button(
            &mut vertices,
            layout.delete_button,
            "DELETE",
            if has_selection {
                BUTTON_NORMAL_COLOR
            } else {
                BUTTON_DISABLED_COLOR
            },
            ACTION_BUTTON_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_button(
            &mut vertices,
            layout.back_button,
            "BACK TO MENU",
            BUTTON_NORMAL_COLOR,
            ACTION_BUTTON_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );

        if view.create_form_open {
            let modal = build_create_world_modal_layout(screen_w, screen_h);
            create_quad_px(
                &mut vertices,
                0.0,
                0.0,
                screen_w,
                screen_h,
                screen_w,
                screen_h,
                WORLD_OVERLAY_COLOR,
            );
            create_quad_px(
                &mut vertices,
                modal.panel.x,
                modal.panel.y,
                modal.panel.w,
                modal.panel.h,
                screen_w,
                screen_h,
                MODAL_COLOR,
            );

            render_text_center_px(
                &mut vertices,
                "CREATE NEW WORLD",
                modal.panel.center_x(),
                modal.panel.y + 36.0,
                MODAL_TITLE_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );

            render_text_left_px(
                &mut vertices,
                "WORLD NAME",
                modal.name_field.x,
                modal.name_field.y - 18.0,
                MODAL_LABEL_PIXEL_SCALE,
                screen_w,
                screen_h,
                WORLD_DETAILS_TEXT_COLOR,
            );
            render_text_left_px(
                &mut vertices,
                "SEED",
                modal.seed_field.x,
                modal.seed_field.y - 18.0,
                MODAL_LABEL_PIXEL_SCALE,
                screen_w,
                screen_h,
                WORLD_DETAILS_TEXT_COLOR,
            );
            render_text_left_px(
                &mut vertices,
                "PLAY MODE",
                modal.play_mode_toggle.x,
                modal.play_mode_toggle.y - 18.0,
                MODAL_LABEL_PIXEL_SCALE,
                screen_w,
                screen_h,
                WORLD_DETAILS_TEXT_COLOR,
            );

            create_quad_px(
                &mut vertices,
                modal.name_field.x,
                modal.name_field.y,
                modal.name_field.w,
                modal.name_field.h,
                screen_w,
                screen_h,
                if view.active_input_field == WorldCreateInputField::Name {
                    MODAL_FIELD_ACTIVE_COLOR
                } else {
                    MODAL_FIELD_COLOR
                },
            );
            create_quad_px(
                &mut vertices,
                modal.seed_field.x,
                modal.seed_field.y,
                modal.seed_field.w,
                modal.seed_field.h,
                screen_w,
                screen_h,
                if view.active_input_field == WorldCreateInputField::Seed {
                    MODAL_FIELD_ACTIVE_COLOR
                } else {
                    MODAL_FIELD_COLOR
                },
            );

            let mut name_input = if view.create_name_input.trim().is_empty() {
                "NEW WORLD".to_string()
            } else {
                truncate_text(view.create_name_input, 32)
            };
            let mut seed_input = if view.create_seed_input.trim().is_empty() {
                "RANDOM".to_string()
            } else {
                truncate_text(view.create_seed_input, 32)
            };

            if view.active_input_field == WorldCreateInputField::Name {
                name_input.push('_');
            } else {
                seed_input.push('_');
            }

            render_text_left_px(
                &mut vertices,
                &name_input,
                modal.name_field.x + 10.0,
                modal.name_field.y + 11.0,
                MODAL_INPUT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
            render_text_left_px(
                &mut vertices,
                &seed_input,
                modal.seed_field.x + 10.0,
                modal.seed_field.y + 11.0,
                MODAL_INPUT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );

            draw_button(
                &mut vertices,
                modal.play_mode_toggle,
                view.create_play_mode_label,
                if view.create_play_mode_is_creative {
                    TOGGLE_ON_COLOR
                } else {
                    BUTTON_NORMAL_COLOR
                },
                BUTTON_TEXT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );

            draw_button(
                &mut vertices,
                modal.create_button,
                "CREATE",
                BUTTON_SELECTED_COLOR,
                BUTTON_TEXT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
            draw_button(
                &mut vertices,
                modal.cancel_button,
                "CANCEL",
                BUTTON_NORMAL_COLOR,
                BUTTON_TEXT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
        }

        if view.delete_confirmation_open {
            let modal = build_delete_world_modal_layout(screen_w, screen_h);
            create_quad_px(
                &mut vertices,
                0.0,
                0.0,
                screen_w,
                screen_h,
                screen_w,
                screen_h,
                WORLD_OVERLAY_COLOR,
            );
            create_quad_px(
                &mut vertices,
                modal.panel.x,
                modal.panel.y,
                modal.panel.w,
                modal.panel.h,
                screen_w,
                screen_h,
                MODAL_COLOR,
            );
            render_text_center_px(
                &mut vertices,
                "DELETE SELECTED WORLD?",
                modal.panel.center_x(),
                modal.panel.y + 50.0,
                MODAL_TITLE_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );

            if let Some(selected) = selected_world {
                let selected_name = truncate_text(view.worlds[selected].name, 28);
                render_text_center_px(
                    &mut vertices,
                    &selected_name,
                    modal.panel.center_x(),
                    modal.panel.y + 88.0,
                    MODAL_LABEL_PIXEL_SCALE,
                    screen_w,
                    screen_h,
                    WORLD_DETAILS_TEXT_COLOR,
                );
            }

            draw_button(
                &mut vertices,
                modal.delete_button,
                "DELETE",
                DELETE_CONFIRM_BUTTON_COLOR,
                BUTTON_TEXT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
            draw_button(
                &mut vertices,
                modal.cancel_button,
                "CANCEL",
                BUTTON_NORMAL_COLOR,
                BUTTON_TEXT_PIXEL_SCALE,
                screen_w,
                screen_h,
                TEXT_COLOR,
            );
        }

        self.write_geometry(queue, vertices);
    }

    pub fn update_pause(&mut self, queue: &wgpu::Queue, width: u32, height: u32, selected_item: u8) {
        let (screen_w, screen_h) = self.update_uniform(queue, width, height);
        let layout = build_pause_menu_layout(screen_w, screen_h);
        let mut vertices = Vec::with_capacity(MAX_VERTICES.min(4096));

        create_quad_px(
            &mut vertices,
            0.0,
            0.0,
            screen_w,
            screen_h,
            screen_w,
            screen_h,
            WORLD_OVERLAY_COLOR,
        );
        create_quad_px(
            &mut vertices,
            layout.panel.x,
            layout.panel.y,
            layout.panel.w,
            layout.panel.h,
            screen_w,
            screen_h,
            MODAL_COLOR,
        );

        render_text_center_px(
            &mut vertices,
            "PAUSED",
            layout.panel.center_x(),
            layout.panel.y + 52.0,
            MODAL_TITLE_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );

        draw_button(
            &mut vertices,
            layout.resume_button,
            "RESUME",
            if selected_item == 0 {
                BUTTON_SELECTED_COLOR
            } else {
                BUTTON_NORMAL_COLOR
            },
            BUTTON_TEXT_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_button(
            &mut vertices,
            layout.settings_button,
            "SETTINGS",
            if selected_item == 1 {
                BUTTON_SELECTED_COLOR
            } else {
                BUTTON_NORMAL_COLOR
            },
            BUTTON_TEXT_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_button(
            &mut vertices,
            layout.save_quit_button,
            "SAVE AND QUIT TO MENU",
            if selected_item == 2 {
                BUTTON_SELECTED_COLOR
            } else {
                BUTTON_NORMAL_COLOR
            },
            ACTION_BUTTON_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );

        self.write_geometry(queue, vertices);
    }

    pub fn update_settings(
        &mut self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        selected_item: u8,
        view: &SettingsMenuView,
    ) {
        let (screen_w, screen_h) = self.update_uniform(queue, width, height);
        let layout = build_settings_menu_layout(screen_w, screen_h);
        let mut vertices = Vec::with_capacity(MAX_VERTICES.min(8192));

        create_quad_px(
            &mut vertices,
            0.0,
            0.0,
            screen_w,
            screen_h,
            screen_w,
            screen_h,
            WORLD_OVERLAY_COLOR,
        );
        create_quad_px(
            &mut vertices,
            layout.panel.x,
            layout.panel.y,
            layout.panel.w,
            layout.panel.h,
            screen_w,
            screen_h,
            MODAL_COLOR,
        );

        render_text_center_px(
            &mut vertices,
            "SETTINGS",
            layout.panel.center_x(),
            layout.panel.y + 46.0,
            MODAL_TITLE_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );

        let render_distance_t = ((view.render_distance - 4) as f32 / 20.0).clamp(0.0, 1.0);
        let stream_surface_t = (view.stream_surface_below as f32 / 3.0).clamp(0.0, 1.0);
        let stream_flight_t = ((view.stream_flight_below - 2) as f32 / 22.0).clamp(0.0, 1.0);
        let stream_above_t = ((view.stream_above - 1) as f32 / 7.0).clamp(0.0, 1.0);
        let lod_distance_t = ((view.lod1_distance - 4) as f32 / 10.0).clamp(0.0, 1.0);
        let sensitivity_t = ((view.mouse_sensitivity - 0.5) / 4.5).clamp(0.0, 1.0);
        let fov_t = ((view.fov - 60.0) / 60.0).clamp(0.0, 1.0);
        let gui_scale_t = ((view.gui_scale - 1.0) / 2.0).clamp(0.0, 1.0);

        render_text_left_px(
            &mut vertices,
            "RENDER DISTANCE",
            layout.label_x,
            layout.render_distance_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.render_distance_slider,
            render_distance_t,
            selected_item == 0,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{} CHUNKS", view.render_distance),
            layout.value_x,
            layout.render_distance_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "SURFACE DEPTH",
            layout.label_x,
            layout.stream_surface_below_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.stream_surface_below_slider,
            stream_surface_t,
            selected_item == 1,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{} CH", view.stream_surface_below),
            layout.value_x,
            layout.stream_surface_below_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "FLIGHT DEPTH",
            layout.label_x,
            layout.stream_flight_below_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.stream_flight_below_slider,
            stream_flight_t,
            selected_item == 2,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{} CH", view.stream_flight_below),
            layout.value_x,
            layout.stream_flight_below_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "UPPER CHUNKS",
            layout.label_x,
            layout.stream_above_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.stream_above_slider,
            stream_above_t,
            selected_item == 3,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{} CH", view.stream_above),
            layout.value_x,
            layout.stream_above_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "LOD SWITCH DISTANCE",
            layout.label_x,
            layout.lod_distance_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.lod_distance_slider,
            lod_distance_t,
            selected_item == 4,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{} CH", view.lod1_distance),
            layout.value_x,
            layout.lod_distance_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "MOUSE SENSITIVITY",
            layout.label_x,
            layout.mouse_sensitivity_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.mouse_sensitivity_slider,
            sensitivity_t,
            selected_item == 5,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{:.2}", view.mouse_sensitivity),
            layout.value_x,
            layout.mouse_sensitivity_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "FIELD OF VIEW",
            layout.label_x,
            layout.fov_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.fov_slider,
            fov_t,
            selected_item == 6,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{:.0} DEG", view.fov),
            layout.value_x,
            layout.fov_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "GUI SCALE",
            layout.label_x,
            layout.gui_scale_slider.y - 22.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_slider(
            &mut vertices,
            layout.gui_scale_slider,
            gui_scale_t,
            selected_item == 7,
            screen_w,
            screen_h,
        );
        render_text_left_px(
            &mut vertices,
            &format!("{:.1}X", view.gui_scale),
            layout.value_x,
            layout.gui_scale_slider.y - 4.0,
            SETTINGS_VALUE_PIXEL_SCALE,
            screen_w,
            screen_h,
            WORLD_DETAILS_TEXT_COLOR,
        );

        render_text_left_px(
            &mut vertices,
            "SHOW FPS",
            layout.label_x,
            layout.show_fps_toggle.y + 7.0,
            SETTINGS_LABEL_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );
        draw_button(
            &mut vertices,
            layout.show_fps_toggle,
            if view.show_fps { "ON" } else { "OFF" },
            if view.show_fps {
                TOGGLE_ON_COLOR
            } else {
                TOGGLE_OFF_COLOR
            },
            BUTTON_TEXT_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );

        if selected_item == 8 {
            create_quad_px(
                &mut vertices,
                layout.show_fps_toggle.x - 3.0,
                layout.show_fps_toggle.y - 3.0,
                layout.show_fps_toggle.w + 6.0,
                layout.show_fps_toggle.h + 6.0,
                screen_w,
                screen_h,
                [0.55, 0.67, 0.93, 0.3],
            );
        }

        draw_button(
            &mut vertices,
            layout.back_button,
            "BACK",
            if selected_item == 9 {
                BUTTON_SELECTED_COLOR
            } else {
                BUTTON_NORMAL_COLOR
            },
            BUTTON_TEXT_PIXEL_SCALE,
            screen_w,
            screen_h,
            TEXT_COLOR,
        );

        self.write_geometry(queue, vertices);
    }

    pub fn hit_test_pause(
        cursor_x: f32,
        cursor_y: f32,
        width: u32,
        height: u32,
    ) -> Option<PauseMenuHitTarget> {
        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let layout = build_pause_menu_layout(screen_w, screen_h);
        if layout.resume_button.contains(cursor_x, cursor_y) {
            return Some(PauseMenuHitTarget::Resume);
        }
        if layout.settings_button.contains(cursor_x, cursor_y) {
            return Some(PauseMenuHitTarget::Settings);
        }
        if layout.save_quit_button.contains(cursor_x, cursor_y) {
            return Some(PauseMenuHitTarget::SaveAndQuit);
        }
        None
    }

    pub fn hit_test_settings(
        cursor_x: f32,
        cursor_y: f32,
        width: u32,
        height: u32,
    ) -> Option<SettingsHitTarget> {
        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let layout = build_settings_menu_layout(screen_w, screen_h);

        if layout.render_distance_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::RenderDistance,
                slider_value_at(layout.render_distance_slider, cursor_x),
            ));
        }
        if layout.stream_surface_below_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::SurfaceBelow,
                slider_value_at(layout.stream_surface_below_slider, cursor_x),
            ));
        }
        if layout.stream_flight_below_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::FlightBelow,
                slider_value_at(layout.stream_flight_below_slider, cursor_x),
            ));
        }
        if layout.stream_above_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::StreamAbove,
                slider_value_at(layout.stream_above_slider, cursor_x),
            ));
        }
        if layout.lod_distance_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::LodDistance,
                slider_value_at(layout.lod_distance_slider, cursor_x),
            ));
        }
        if layout.mouse_sensitivity_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::MouseSensitivity,
                slider_value_at(layout.mouse_sensitivity_slider, cursor_x),
            ));
        }
        if layout.fov_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::Fov,
                slider_value_at(layout.fov_slider, cursor_x),
            ));
        }
        if layout.gui_scale_slider.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Slider(
                SettingsSliderKind::GuiScale,
                slider_value_at(layout.gui_scale_slider, cursor_x),
            ));
        }
        if layout.show_fps_toggle.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::ShowFpsToggle);
        }
        if layout.back_button.contains(cursor_x, cursor_y) {
            return Some(SettingsHitTarget::Back);
        }
        None
    }

    pub fn settings_slider_fraction(
        slider_kind: SettingsSliderKind,
        cursor_x: f32,
        width: u32,
        height: u32,
    ) -> f32 {
        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let layout = build_settings_menu_layout(screen_w, screen_h);
        let slider = match slider_kind {
            SettingsSliderKind::RenderDistance => layout.render_distance_slider,
            SettingsSliderKind::SurfaceBelow => layout.stream_surface_below_slider,
            SettingsSliderKind::FlightBelow => layout.stream_flight_below_slider,
            SettingsSliderKind::StreamAbove => layout.stream_above_slider,
            SettingsSliderKind::LodDistance => layout.lod_distance_slider,
            SettingsSliderKind::MouseSensitivity => layout.mouse_sensitivity_slider,
            SettingsSliderKind::Fov => layout.fov_slider,
            SettingsSliderKind::GuiScale => layout.gui_scale_slider,
        };
        slider_value_at(slider, cursor_x)
    }

    pub fn settings_target_to_selection(target: SettingsHitTarget) -> u8 {
        match target {
            SettingsHitTarget::Slider(SettingsSliderKind::RenderDistance, _) => 0,
            SettingsHitTarget::Slider(SettingsSliderKind::SurfaceBelow, _) => 1,
            SettingsHitTarget::Slider(SettingsSliderKind::FlightBelow, _) => 2,
            SettingsHitTarget::Slider(SettingsSliderKind::StreamAbove, _) => 3,
            SettingsHitTarget::Slider(SettingsSliderKind::LodDistance, _) => 4,
            SettingsHitTarget::Slider(SettingsSliderKind::MouseSensitivity, _) => 5,
            SettingsHitTarget::Slider(SettingsSliderKind::Fov, _) => 6,
            SettingsHitTarget::Slider(SettingsSliderKind::GuiScale, _) => 7,
            SettingsHitTarget::ShowFpsToggle => 8,
            SettingsHitTarget::Back => 9,
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.num_indices == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.menu_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
    }

    pub fn hit_test(cursor_x: f32, cursor_y: f32, width: u32, height: u32) -> Option<u8> {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;

        let nx = (cursor_x / w) * 2.0 - 1.0;
        let ny = 1.0 - (cursor_y / h) * 2.0;

        let button_w = ndc_from_px_x(BUTTON_WIDTH_PX, w);
        let button_h = ndc_from_px_y(BUTTON_HEIGHT_PX, h);
        let button_spacing = ndc_from_px_y(BUTTON_SPACING_PX, h);
        let total_h = button_h * 3.0 + button_spacing * 2.0;
        let start_y = total_h / 2.0;

        let left = -button_w / 2.0;
        let right = button_w / 2.0;
        if nx < left || nx > right {
            return None;
        }

        for i in 0..3u8 {
            let top = start_y - (i as f32) * (button_h + button_spacing);
            let bottom = top - button_h;
            if ny <= top && ny >= bottom {
                return Some(i);
            }
        }

        None
    }

    pub fn hit_test_world_select(
        cursor_x: f32,
        cursor_y: f32,
        width: u32,
        height: u32,
        world_count: usize,
        selected_world: Option<usize>,
        create_form_open: bool,
        delete_confirmation_open: bool,
    ) -> Option<WorldSelectHitTarget> {
        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;

        if delete_confirmation_open {
            let modal = build_delete_world_modal_layout(screen_w, screen_h);
            if modal.delete_button.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::DeleteConfirm);
            }
            if modal.cancel_button.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::DeleteCancel);
            }
            return None;
        }

        if create_form_open {
            let modal = build_create_world_modal_layout(screen_w, screen_h);
            if modal.name_field.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::CreateNameField);
            }
            if modal.seed_field.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::CreateSeedField);
            }
            if modal.play_mode_toggle.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::CreatePlayModeToggle);
            }
            if modal.create_button.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::CreateConfirm);
            }
            if modal.cancel_button.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::CreateCancel);
            }
            return None;
        }

        let selected_world = selected_world.filter(|idx| *idx < world_count);
        let layout = build_world_select_layout(screen_w, screen_h, world_count, selected_world);

        for (world_idx, row_rect) in layout.world_rows {
            if row_rect.contains(cursor_x, cursor_y) {
                return Some(WorldSelectHitTarget::WorldEntry(world_idx));
            }
        }

        if layout.create_button.contains(cursor_x, cursor_y) {
            return Some(WorldSelectHitTarget::CreateNewWorld);
        }
        if layout.play_button.contains(cursor_x, cursor_y) {
            return Some(WorldSelectHitTarget::PlaySelected);
        }
        if layout.delete_button.contains(cursor_x, cursor_y) {
            return Some(WorldSelectHitTarget::DeleteSelected);
        }
        if layout.back_button.contains(cursor_x, cursor_y) {
            return Some(WorldSelectHitTarget::Back);
        }

        None
    }
}

fn build_world_select_layout(
    screen_w: f32,
    screen_h: f32,
    world_count: usize,
    selected_world: Option<usize>,
) -> WorldSelectLayout {
    let panel_top = WORLD_PANEL_TOP_PX.min((screen_h - 190.0).max(92.0));
    let content_h = (screen_h - panel_top - 34.0).clamp(220.0, 560.0);

    let action_w = (screen_w * 0.24).clamp(220.0, 310.0);
    let gap = 20.0;
    let panel_w = (screen_w - 120.0 - action_w - gap).clamp(420.0, 920.0);
    let total_w = panel_w + gap + action_w;
    let panel_left = (screen_w - total_w) * 0.5;
    let action_left = panel_left + panel_w + gap;

    let panel = RectPx {
        x: panel_left,
        y: panel_top,
        w: panel_w,
        h: content_h,
    };

    let row_top = panel_top + 56.0;
    let rows_h = (content_h - 56.0 - WORLD_PANEL_PADDING_PX).max(WORLD_MIN_ROW_HEIGHT_PX);
    let max_visible = (((rows_h + WORLD_ROW_SPACING_PX)
        / (WORLD_MIN_ROW_HEIGHT_PX + WORLD_ROW_SPACING_PX))
        .floor() as usize)
        .max(1);
    let visible_count = world_count.min(max_visible);
    let (start_world, end_world) = visible_world_range(world_count, visible_count, selected_world);

    let row_h = if visible_count == 0 {
        WORLD_MIN_ROW_HEIGHT_PX
    } else {
        (rows_h - WORLD_ROW_SPACING_PX * (visible_count.saturating_sub(1) as f32)) / visible_count as f32
    };

    let mut world_rows = Vec::with_capacity(visible_count);
    for (slot, world_idx) in (start_world..end_world).enumerate() {
        let y = row_top + slot as f32 * (row_h + WORLD_ROW_SPACING_PX);
        world_rows.push((
            world_idx,
            RectPx {
                x: panel_left + WORLD_PANEL_PADDING_PX,
                y,
                w: panel_w - WORLD_PANEL_PADDING_PX * 2.0,
                h: row_h,
            },
        ));
    }

    let button_w = action_w - 28.0;
    let button_x = action_left + 14.0;
    let top_buttons_start = panel_top + 96.0;
    let top_button_gap = 12.0;

    let create_button = RectPx {
        x: button_x,
        y: top_buttons_start,
        w: button_w,
        h: WORLD_ACTION_BUTTON_HEIGHT_PX,
    };
    let play_button = RectPx {
        x: button_x,
        y: create_button.y + WORLD_ACTION_BUTTON_HEIGHT_PX + top_button_gap,
        w: button_w,
        h: WORLD_ACTION_BUTTON_HEIGHT_PX,
    };
    let delete_button = RectPx {
        x: button_x,
        y: panel_top + content_h - WORLD_ACTION_BUTTON_HEIGHT_PX * 2.0 - 24.0,
        w: button_w,
        h: WORLD_ACTION_BUTTON_HEIGHT_PX,
    };
    let back_button = RectPx {
        x: button_x,
        y: panel_top + content_h - WORLD_ACTION_BUTTON_HEIGHT_PX - 12.0,
        w: button_w,
        h: WORLD_ACTION_BUTTON_HEIGHT_PX,
    };

    WorldSelectLayout {
        panel,
        world_rows,
        create_button,
        play_button,
        delete_button,
        back_button,
    }
}

fn build_create_world_modal_layout(screen_w: f32, screen_h: f32) -> CreateWorldModalLayout {
    let panel_w = (screen_w - 80.0).clamp(380.0, 620.0);
    let panel_h = (screen_h - 80.0).clamp(330.0, 400.0);
    let panel_x = (screen_w - panel_w) * 0.5;
    let panel_y = (screen_h - panel_h) * 0.5;
    let panel = RectPx {
        x: panel_x,
        y: panel_y,
        w: panel_w,
        h: panel_h,
    };

    let field_w = panel_w - 80.0;
    let field_x = panel_x + (panel_w - field_w) * 0.5;
    let name_field = RectPx {
        x: field_x,
        y: panel_y + 84.0,
        w: field_w,
        h: MODAL_FIELD_HEIGHT_PX,
    };
    let seed_field = RectPx {
        x: field_x,
        y: name_field.y + MODAL_FIELD_HEIGHT_PX + 20.0,
        w: field_w,
        h: MODAL_FIELD_HEIGHT_PX,
    };

    let play_mode_toggle = RectPx {
        x: field_x,
        y: seed_field.y + MODAL_FIELD_HEIGHT_PX + 20.0,
        w: field_w,
        h: 40.0,
    };

    let button_w = (field_w - 16.0) * 0.5;
    let button_y = panel_y + panel_h - 66.0;
    let create_button = RectPx {
        x: field_x,
        y: button_y,
        w: button_w,
        h: 44.0,
    };
    let cancel_button = RectPx {
        x: field_x + button_w + 16.0,
        y: button_y,
        w: button_w,
        h: 44.0,
    };

    CreateWorldModalLayout {
        panel,
        name_field,
        seed_field,
        play_mode_toggle,
        create_button,
        cancel_button,
    }
}

fn build_delete_world_modal_layout(screen_w: f32, screen_h: f32) -> DeleteWorldModalLayout {
    let panel_w = (screen_w - 120.0).clamp(360.0, 520.0);
    let panel_h = 200.0;
    let panel_x = (screen_w - panel_w) * 0.5;
    let panel_y = (screen_h - panel_h) * 0.5;
    let panel = RectPx {
        x: panel_x,
        y: panel_y,
        w: panel_w,
        h: panel_h,
    };

    let button_w = (panel_w - 100.0) * 0.5;
    let button_y = panel_y + panel_h - 58.0;
    let delete_button = RectPx {
        x: panel_x + 40.0,
        y: button_y,
        w: button_w,
        h: 40.0,
    };
    let cancel_button = RectPx {
        x: delete_button.x + button_w + 20.0,
        y: button_y,
        w: button_w,
        h: 40.0,
    };

    DeleteWorldModalLayout {
        panel,
        delete_button,
        cancel_button,
    }
}

fn build_pause_menu_layout(screen_w: f32, screen_h: f32) -> PauseMenuLayout {
    let max_w = (screen_w - 24.0).max(220.0);
    let max_h = (screen_h - 24.0).max(180.0);
    let panel_w = PAUSE_PANEL_WIDTH_PX.min(max_w).max(280.0).min(max_w);
    let panel_h = PAUSE_PANEL_HEIGHT_PX.min(max_h).max(220.0).min(max_h);
    let panel = RectPx {
        x: (screen_w - panel_w) * 0.5,
        y: (screen_h - panel_h) * 0.5,
        w: panel_w,
        h: panel_h,
    };

    let button_w = panel_w - 96.0;
    let button_x = panel.x + (panel_w - button_w) * 0.5;
    let first_button_y = panel.y + 98.0;

    let resume_button = RectPx {
        x: button_x,
        y: first_button_y,
        w: button_w,
        h: PAUSE_BUTTON_HEIGHT_PX,
    };
    let settings_button = RectPx {
        x: button_x,
        y: first_button_y + PAUSE_BUTTON_HEIGHT_PX + PAUSE_BUTTON_SPACING_PX,
        w: button_w,
        h: PAUSE_BUTTON_HEIGHT_PX,
    };
    let save_quit_button = RectPx {
        x: button_x,
        y: settings_button.y + PAUSE_BUTTON_HEIGHT_PX + PAUSE_BUTTON_SPACING_PX,
        w: button_w,
        h: PAUSE_BUTTON_HEIGHT_PX,
    };

    PauseMenuLayout {
        panel,
        resume_button,
        settings_button,
        save_quit_button,
    }
}

fn build_settings_menu_layout(screen_w: f32, screen_h: f32) -> SettingsMenuLayout {
    let max_w = (screen_w - 24.0).max(320.0);
    let max_h = (screen_h - 24.0).max(260.0);
    let panel_w = SETTINGS_PANEL_WIDTH_PX
        .min(max_w)
        .max(420.0)
        .min(max_w);
    let panel_h = SETTINGS_PANEL_HEIGHT_PX
        .min(max_h)
        .max(320.0)
        .min(max_h);
    let panel = RectPx {
        x: (screen_w - panel_w) * 0.5,
        y: (screen_h - panel_h) * 0.5,
        w: panel_w,
        h: panel_h,
    };

    let row_start_y = panel.y + 88.0;
    let slider_x = panel.x + panel.w * 0.44;
    let slider_w = (panel.w * 0.38).max(160.0);
    let value_x = slider_x + slider_w + 26.0;

    let render_distance_slider = RectPx {
        x: slider_x,
        y: row_start_y + (SETTINGS_ROW_HEIGHT_PX - SETTINGS_SLIDER_HEIGHT_PX) * 0.5,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };
    let stream_surface_below_slider = RectPx {
        x: slider_x,
        y: render_distance_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };
    let stream_flight_below_slider = RectPx {
        x: slider_x,
        y: stream_surface_below_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };
    let stream_above_slider = RectPx {
        x: slider_x,
        y: stream_flight_below_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };
    let lod_distance_slider = RectPx {
        x: slider_x,
        y: stream_above_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };
    let mouse_sensitivity_slider = RectPx {
        x: slider_x,
        y: lod_distance_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };
    let fov_slider = RectPx {
        x: slider_x,
        y: mouse_sensitivity_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };
    let gui_scale_slider = RectPx {
        x: slider_x,
        y: fov_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX,
        w: slider_w,
        h: SETTINGS_SLIDER_HEIGHT_PX,
    };

    let show_fps_toggle = RectPx {
        x: slider_x,
        y: gui_scale_slider.y + SETTINGS_ROW_HEIGHT_PX + SETTINGS_ROW_SPACING_PX - 10.0,
        w: 118.0,
        h: 38.0,
    };

    let back_button = RectPx {
        x: panel.x + panel.w - 184.0,
        y: panel.y + panel.h - SETTINGS_BACK_BUTTON_HEIGHT_PX - 24.0,
        w: 148.0,
        h: SETTINGS_BACK_BUTTON_HEIGHT_PX,
    };

    SettingsMenuLayout {
        panel,
        render_distance_slider,
        stream_surface_below_slider,
        stream_flight_below_slider,
        stream_above_slider,
        lod_distance_slider,
        mouse_sensitivity_slider,
        fov_slider,
        gui_scale_slider,
        show_fps_toggle,
        back_button,
        label_x: panel.x + 46.0,
        value_x,
    }
}

fn visible_world_range(
    total_worlds: usize,
    visible_worlds: usize,
    selected_world: Option<usize>,
) -> (usize, usize) {
    if total_worlds == 0 || visible_worlds == 0 || total_worlds <= visible_worlds {
        return (0, total_worlds);
    }

    let mut start = selected_world.unwrap_or(0).saturating_sub(visible_worlds / 2);
    if start + visible_worlds > total_worlds {
        start = total_worlds - visible_worlds;
    }

    (start, start + visible_worlds)
}

fn seed_preview_color(seed: u64) -> [f32; 4] {
    let mut value = seed ^ 0x9E37_79B9_7F4A_7C15;
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;

    let r = 0.22 + ((value & 0xFF) as f32 / 255.0) * 0.65;
    let g = 0.24 + (((value >> 8) & 0xFF) as f32 / 255.0) * 0.62;
    let b = 0.3 + (((value >> 16) & 0xFF) as f32 / 255.0) * 0.58;
    [r.min(1.0), g.min(1.0), b.min(1.0), 1.0]
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    if max_chars <= 3 {
        return text.chars().take(max_chars).collect();
    }

    let mut out: String = text.chars().take(max_chars - 3).collect();
    out.push_str("...");
    out
}

fn draw_button(
    vertices: &mut Vec<UiVertex>,
    rect: RectPx,
    label: &str,
    color: [f32; 4],
    text_scale: f32,
    screen_w: f32,
    screen_h: f32,
    text_color: [f32; 4],
) {
    create_quad_px(
        vertices, rect.x, rect.y, rect.w, rect.h, screen_w, screen_h, color,
    );
    render_text_center_px(
        vertices,
        label,
        rect.center_x(),
        rect.center_y(),
        text_scale,
        screen_w,
        screen_h,
        text_color,
    );
}

fn draw_slider(
    vertices: &mut Vec<UiVertex>,
    rect: RectPx,
    normalized_value: f32,
    selected: bool,
    screen_w: f32,
    screen_h: f32,
) {
    let normalized_value = normalized_value.clamp(0.0, 1.0);
    let track_color = if selected {
        [SLIDER_TRACK_COLOR[0] + 0.06, SLIDER_TRACK_COLOR[1] + 0.06, SLIDER_TRACK_COLOR[2] + 0.08, 1.0]
    } else {
        SLIDER_TRACK_COLOR
    };
    create_quad_px(vertices, rect.x, rect.y, rect.w, rect.h, screen_w, screen_h, track_color);

    let fill_w = (rect.w * normalized_value).max(2.0);
    create_quad_px(
        vertices,
        rect.x,
        rect.y,
        fill_w,
        rect.h,
        screen_w,
        screen_h,
        SLIDER_FILL_COLOR,
    );

    let knob_size = rect.h + 10.0;
    let knob_center_x = rect.x + rect.w * normalized_value;
    create_quad_px(
        vertices,
        knob_center_x - knob_size * 0.5,
        rect.y - (knob_size - rect.h) * 0.5,
        knob_size,
        knob_size,
        screen_w,
        screen_h,
        if selected {
            [0.98, 1.0, 1.0, 1.0]
        } else {
            SLIDER_KNOB_COLOR
        },
    );
}

fn slider_value_at(rect: RectPx, cursor_x: f32) -> f32 {
    ((cursor_x - rect.x) / rect.w.max(1.0)).clamp(0.0, 1.0)
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
    let (x_ndc, y_ndc) = screen_to_ndc(x_px, y_px, screen_w, screen_h);
    create_quad(
        vertices,
        x_ndc,
        y_ndc,
        ndc_from_px_x(w_px, screen_w),
        ndc_from_px_y(h_px, screen_h),
        color,
    );
}

fn render_text_center_px(
    vertices: &mut Vec<UiVertex>,
    text: &str,
    center_x_px: f32,
    center_y_px: f32,
    pixel_scale: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let center_x_ndc = (center_x_px / screen_w) * 2.0 - 1.0;
    let center_y_ndc = 1.0 - (center_y_px / screen_h) * 2.0;
    render_text(
        vertices,
        text,
        center_x_ndc,
        center_y_ndc,
        pixel_scale,
        screen_w,
        screen_h,
        color,
    );
}

fn render_text_left_px(
    vertices: &mut Vec<UiVertex>,
    text: &str,
    x_px: f32,
    y_px: f32,
    pixel_scale: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    render_text_origin_px(vertices, text, x_px, y_px, pixel_scale, screen_w, screen_h, color);
}

fn render_text_origin_px(
    vertices: &mut Vec<UiVertex>,
    text: &str,
    origin_x_px: f32,
    origin_y_px: f32,
    pixel_scale: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let pixel_w_ndc = ndc_from_px_x(pixel_scale, screen_w);
    let pixel_h_ndc = ndc_from_px_y(pixel_scale, screen_h);

    let char_stride = 6.0;
    for (char_index, ch) in text.chars().enumerate() {
        let ch = ch.to_ascii_uppercase();
        let Some(rows) = glyph(ch) else {
            continue;
        };

        let char_x_px = origin_x_px + char_index as f32 * char_stride * pixel_scale;

        for (row_idx, row_bits) in rows.iter().enumerate() {
            for col in 0..5 {
                if (row_bits & (0x10 >> col)) == 0 {
                    continue;
                }

                let x_px = char_x_px + col as f32 * pixel_scale;
                let y_px = origin_y_px + row_idx as f32 * pixel_scale;
                let (x_ndc, y_ndc) = screen_to_ndc(x_px, y_px, screen_w, screen_h);
                create_quad(vertices, x_ndc, y_ndc, pixel_w_ndc, pixel_h_ndc, color);
            }
        }
    }
}

fn create_quad(vertices: &mut Vec<UiVertex>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    create_quad_with_effect(vertices, x, y, w, h, color, 0.0);
}

fn create_quad_with_effect(
    vertices: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
    effect: f32,
) {
    vertices.extend_from_slice(&[
        UiVertex {
            position: [x, y - h],
            color,
            effect,
        },
        UiVertex {
            position: [x + w, y - h],
            color,
            effect,
        },
        UiVertex {
            position: [x + w, y],
            color,
            effect,
        },
        UiVertex {
            position: [x, y],
            color,
            effect,
        },
    ]);
}

fn screen_to_ndc(x_px: f32, y_px: f32, screen_w: f32, screen_h: f32) -> (f32, f32) {
    (
        (x_px / screen_w) * 2.0 - 1.0,
        1.0 - (y_px / screen_h) * 2.0,
    )
}

fn ndc_from_px_x(px: f32, screen_w: f32) -> f32 {
    (px / screen_w) * 2.0
}

fn ndc_from_px_y(px: f32, screen_h: f32) -> f32 {
    (px / screen_h) * 2.0
}

fn render_text(
    vertices: &mut Vec<UiVertex>,
    text: &str,
    center_x_ndc: f32,
    center_y_ndc: f32,
    pixel_scale: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let glyph_count = text.chars().count();
    if glyph_count == 0 {
        return;
    }

    let char_stride = 6.0;
    let text_w_font_px = glyph_count as f32 * char_stride - 1.0;
    let text_h_font_px = 7.0;

    let text_w_px = text_w_font_px * pixel_scale;
    let text_h_px = text_h_font_px * pixel_scale;

    let center_x_px = (center_x_ndc + 1.0) * 0.5 * screen_w;
    let center_y_px = (1.0 - center_y_ndc) * 0.5 * screen_h;

    let origin_x_px = center_x_px - text_w_px * 0.5;
    let origin_y_px = center_y_px - text_h_px * 0.5;

    render_text_origin_px(
        vertices,
        text,
        origin_x_px,
        origin_y_px,
        pixel_scale,
        screen_w,
        screen_h,
        color,
    );
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
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '/' => [0x01, 0x02, 0x04, 0x08, 0x10, 0x00, 0x00],
        _ => return None,
    })
}
