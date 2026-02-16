use std::mem;

use bytemuck::{Pod, Zeroable};

const MAX_QUADS: usize = 2_048;
const MAX_VERTICES: usize = MAX_QUADS * 4;
const MAX_INDICES: usize = MAX_QUADS * 6;

const ICON_COUNT: usize = 10;
const ICON_SIZE_PX: f32 = 16.0;
const ICON_GAP_PX: f32 = 4.0;
const HUD_BOTTOM_OFFSET_PX: f32 = 78.0;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UiVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub struct HealthHudRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl HealthHudRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Health HUD Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/ui_simple.wgsl"
                ))
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Health HUD Pipeline Layout"),
            bind_group_layouts: &[],
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
        ];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Health HUD Pipeline"),
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
            label: Some("Health HUD Vertex Buffer"),
            size: (MAX_VERTICES * mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Health HUD Index Buffer"),
            size: (MAX_INDICES * mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count: 0,
        }
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
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
        if !visible {
            self.index_count = 0;
            return;
        }

        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let ui_scale = gui_scale.clamp(1.0, 3.0);
        let icon_size = ICON_SIZE_PX * ui_scale;
        let icon_gap = ICON_GAP_PX * ui_scale;
        let icon_border = (2.0 * ui_scale).max(1.0);
        let icon_fill_inset = (3.0 * ui_scale).max(1.0);
        let hud_margin = 18.0 * ui_scale;
        let mut vertices = Vec::with_capacity(512);

        let icon_strip_w = ICON_COUNT as f32 * icon_size + (ICON_COUNT as f32 - 1.0) * icon_gap;
        let base_y = screen_h - HUD_BOTTOM_OFFSET_PX * ui_scale;
        let health_x = hud_margin;
        let hunger_x = screen_w - icon_strip_w - hud_margin;

        let flashing = damage_flash_timer > 0.0;

        for i in 0..ICON_COUNT {
            let x = health_x + i as f32 * (icon_size + icon_gap);
            let hp_threshold = (i + 1) as f32 * 2.0;
            let filled = health >= hp_threshold;

            draw_icon_frame(
                &mut vertices,
                x,
                base_y,
                icon_size,
                icon_border,
                screen_w,
                screen_h,
            );
            let fill_color = if filled {
                if flashing {
                    [1.0, 0.34, 0.34, 1.0]
                } else {
                    [0.88, 0.13, 0.13, 0.96]
                }
            } else {
                [0.28, 0.08, 0.08, 0.8]
            };
            create_quad_px(
                &mut vertices,
                x + icon_fill_inset,
                base_y + icon_fill_inset,
                icon_size - icon_fill_inset * 2.0,
                icon_size - icon_fill_inset * 2.0,
                screen_w,
                screen_h,
                fill_color,
            );
        }

        for i in 0..ICON_COUNT {
            let x = hunger_x + i as f32 * (icon_size + icon_gap);
            let hunger_threshold = (i + 1) as f32 * 2.0;
            let filled = hunger >= hunger_threshold;

            draw_icon_frame(
                &mut vertices,
                x,
                base_y,
                icon_size,
                icon_border,
                screen_w,
                screen_h,
            );
            let fill_color = if filled {
                [0.87, 0.55, 0.2, 0.96]
            } else {
                [0.24, 0.22, 0.2, 0.78]
            };
            create_quad_px(
                &mut vertices,
                x + icon_fill_inset,
                base_y + icon_fill_inset,
                icon_size - icon_fill_inset * 2.0,
                icon_size - icon_fill_inset * 2.0,
                screen_w,
                screen_h,
                fill_color,
            );
        }

        // XP progress bar (green bar below health/hunger row)
        {
            let bar_w = icon_strip_w * 2.0 + 36.0;
            let bar_h = 6.0 * ui_scale;
            let bar_x = health_x;
            let bar_y = base_y + icon_size + 4.0 * ui_scale;
            create_quad_px(
                &mut vertices,
                bar_x,
                bar_y,
                bar_w,
                bar_h,
                screen_w,
                screen_h,
                [0.1, 0.1, 0.1, 0.8],
            );
            let filled_w = bar_w * xp_progress.clamp(0.0, 1.0);
            if filled_w > 0.0 {
                create_quad_px(
                    &mut vertices,
                    bar_x,
                    bar_y,
                    filled_w,
                    bar_h,
                    screen_w,
                    screen_h,
                    [0.3, 0.9, 0.1, 0.95],
                );
            }
            if xp_level > 0 {
                let digit_size = 4.0 * ui_scale;
                let level_x = bar_x + bar_w * 0.5 - digit_size * 0.5;
                let level_y = bar_y - 2.0 * ui_scale;
                create_quad_px(
                    &mut vertices,
                    level_x,
                    level_y,
                    digit_size,
                    digit_size,
                    screen_w,
                    screen_h,
                    [0.3, 0.9, 0.1, 1.0],
                );
            }
        }

        // Air bubbles (shown when air_supply < max_air_supply)
        if air_supply < max_air_supply {
            let bubble_count: usize = 10;
            let bubble_size: f32 = 10.0 * ui_scale;
            let bubble_gap: f32 = 3.0 * ui_scale;
            let bubble_border = (1.0 * ui_scale).max(1.0);
            let bubble_fill_inset = (2.0 * ui_scale).max(1.0);
            let bubble_strip_w =
                bubble_count as f32 * bubble_size + (bubble_count as f32 - 1.0) * bubble_gap;
            let bubble_x = hunger_x + icon_strip_w - bubble_strip_w;
            let bubble_y = base_y - bubble_size - 4.0 * ui_scale;
            for i in 0..bubble_count {
                let x = bubble_x + i as f32 * (bubble_size + bubble_gap);
                let threshold = (i + 1) as f32 / bubble_count as f32 * max_air_supply;
                let filled = air_supply >= threshold;
                let color = if filled {
                    [0.4, 0.7, 1.0, 0.9]
                } else {
                    [0.15, 0.2, 0.3, 0.5]
                };
                create_quad_px(
                    &mut vertices,
                    x,
                    bubble_y,
                    bubble_size,
                    bubble_border,
                    screen_w,
                    screen_h,
                    [0.2, 0.3, 0.5, 0.7],
                );
                create_quad_px(
                    &mut vertices,
                    x,
                    bubble_y + bubble_size - bubble_border,
                    bubble_size,
                    bubble_border,
                    screen_w,
                    screen_h,
                    [0.2, 0.3, 0.5, 0.7],
                );
                create_quad_px(
                    &mut vertices,
                    x,
                    bubble_y,
                    bubble_border,
                    bubble_size,
                    screen_w,
                    screen_h,
                    [0.2, 0.3, 0.5, 0.7],
                );
                create_quad_px(
                    &mut vertices,
                    x + bubble_size - bubble_border,
                    bubble_y,
                    bubble_border,
                    bubble_size,
                    screen_w,
                    screen_h,
                    [0.2, 0.3, 0.5, 0.7],
                );
                create_quad_px(
                    &mut vertices,
                    x + bubble_fill_inset,
                    bubble_y + bubble_fill_inset,
                    bubble_size - bubble_fill_inset * 2.0,
                    bubble_size - bubble_fill_inset * 2.0,
                    screen_w,
                    screen_h,
                    color,
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
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn draw_icon_frame(
    vertices: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    icon_size: f32,
    border_width: f32,
    screen_w: f32,
    screen_h: f32,
) {
    let border = [0.06, 0.06, 0.08, 0.92];
    create_quad_px(vertices, x, y, icon_size, border_width, screen_w, screen_h, border);
    create_quad_px(
        vertices,
        x,
        y + icon_size - border_width,
        icon_size,
        border_width,
        screen_w,
        screen_h,
        border,
    );
    create_quad_px(vertices, x, y, border_width, icon_size, screen_w, screen_h, border);
    create_quad_px(
        vertices,
        x + icon_size - border_width,
        y,
        border_width,
        icon_size,
        screen_w,
        screen_h,
        border,
    );
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
        },
        UiVertex {
            position: [x1, y1],
            color,
        },
        UiVertex {
            position: [x1, y0],
            color,
        },
        UiVertex {
            position: [x0, y0],
            color,
        },
    ]);
}

fn screen_to_ndc(x_px: f32, y_px: f32, screen_w: f32, screen_h: f32) -> (f32, f32) {
    ((x_px / screen_w) * 2.0 - 1.0, 1.0 - (y_px / screen_h) * 2.0)
}
