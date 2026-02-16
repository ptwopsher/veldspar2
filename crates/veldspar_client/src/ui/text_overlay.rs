use std::mem;

use bytemuck::{Pod, Zeroable};

const MAX_QUADS: usize = 12_000;
const MAX_VERTICES: usize = MAX_QUADS * 4;
const MAX_INDICES: usize = MAX_QUADS * 6;

const PANEL_MARGIN_PX: f32 = 10.0;
const PANEL_PADDING_PX: f32 = 8.0;
const FONT_PIXEL_SCALE: f32 = 2.0;
const LINE_GAP_PX: f32 = 4.0;

const PANEL_BG_COLOR: [f32; 4] = [0.03, 0.03, 0.04, 0.7];
const TEXT_COLOR: [f32; 4] = [0.95, 0.95, 0.95, 1.0];

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UiVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub struct TextOverlayRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl TextOverlayRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/ui_simple.wgsl"
                ))
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Overlay Pipeline Layout"),
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
            label: Some("Text Overlay Pipeline"),
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
            label: Some("Text Overlay Vertex Buffer"),
            size: (MAX_VERTICES * mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Overlay Index Buffer"),
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
        lines: &[String],
        chat_lines: &[(String, f32)],
        chat_input_line: Option<&str>,
    ) {
        if lines.is_empty() && chat_lines.is_empty() && chat_input_line.is_none() {
            self.index_count = 0;
            return;
        }

        let screen_w = width.max(1) as f32;
        let screen_h = height.max(1) as f32;
        let mut vertices = Vec::with_capacity(4096);
        let char_stride_px = 6.0 * FONT_PIXEL_SCALE;
        let line_height_px = 7.0 * FONT_PIXEL_SCALE + LINE_GAP_PX;

        if !lines.is_empty() {
            let longest_line_len = lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0);
            let text_width_px = if longest_line_len == 0 {
                0.0
            } else {
                longest_line_len as f32 * char_stride_px - FONT_PIXEL_SCALE
            };
            let text_height_px = lines.len() as f32 * line_height_px - LINE_GAP_PX;

            let panel_width_px = text_width_px + PANEL_PADDING_PX * 2.0;
            let panel_height_px = text_height_px + PANEL_PADDING_PX * 2.0;
            create_quad_px(
                &mut vertices,
                PANEL_MARGIN_PX,
                PANEL_MARGIN_PX,
                panel_width_px,
                panel_height_px,
                screen_w,
                screen_h,
                PANEL_BG_COLOR,
            );

            let text_x = PANEL_MARGIN_PX + PANEL_PADDING_PX;
            let mut text_y = PANEL_MARGIN_PX + PANEL_PADDING_PX;
            for line in lines {
                render_text_px(
                    &mut vertices,
                    line,
                    text_x,
                    text_y,
                    FONT_PIXEL_SCALE,
                    screen_w,
                    screen_h,
                    TEXT_COLOR,
                );
                text_y += line_height_px;
            }
        }

        if !chat_lines.is_empty() || chat_input_line.is_some() {
            let mut longest_line_len = chat_lines
                .iter()
                .map(|(line, _)| line.chars().count())
                .max()
                .unwrap_or(0);
            if let Some(input_line) = chat_input_line {
                longest_line_len = longest_line_len.max(input_line.chars().count());
            }

            let chat_line_count = chat_lines.len() + usize::from(chat_input_line.is_some());
            let text_width_px = if longest_line_len == 0 {
                0.0
            } else {
                longest_line_len as f32 * char_stride_px - FONT_PIXEL_SCALE
            };
            let text_height_px = if chat_line_count == 0 {
                0.0
            } else {
                chat_line_count as f32 * line_height_px - LINE_GAP_PX
            };

            let panel_width_px = text_width_px + PANEL_PADDING_PX * 2.0;
            let panel_height_px = text_height_px + PANEL_PADDING_PX * 2.0;
            let panel_y_px = (screen_h - PANEL_MARGIN_PX - panel_height_px).max(PANEL_MARGIN_PX);

            let mut panel_color = PANEL_BG_COLOR;
            let max_chat_alpha = chat_lines
                .iter()
                .map(|(_, alpha)| *alpha)
                .fold(0.0_f32, f32::max);
            let panel_alpha_scale = if chat_input_line.is_some() {
                1.0
            } else {
                max_chat_alpha.clamp(0.15, 1.0)
            };
            panel_color[3] *= panel_alpha_scale;

            create_quad_px(
                &mut vertices,
                PANEL_MARGIN_PX,
                panel_y_px,
                panel_width_px,
                panel_height_px,
                screen_w,
                screen_h,
                panel_color,
            );

            let text_x = PANEL_MARGIN_PX + PANEL_PADDING_PX;
            let mut text_y = panel_y_px + PANEL_PADDING_PX;
            for (line, alpha) in chat_lines {
                let mut color = TEXT_COLOR;
                color[3] *= alpha.clamp(0.0, 1.0);
                render_text_px(
                    &mut vertices,
                    line,
                    text_x,
                    text_y,
                    FONT_PIXEL_SCALE,
                    screen_w,
                    screen_h,
                    color,
                );
                text_y += line_height_px;
            }
            if let Some(input_line) = chat_input_line {
                render_text_px(
                    &mut vertices,
                    input_line,
                    text_x,
                    text_y,
                    FONT_PIXEL_SCALE,
                    screen_w,
                    screen_h,
                    TEXT_COLOR,
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

fn render_text_px(
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
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x08],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '>' => [0x10, 0x08, 0x04, 0x02, 0x04, 0x08, 0x10],
        '<' => [0x01, 0x02, 0x04, 0x08, 0x04, 0x02, 0x01],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '\'' => [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '`' => [0x08, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
        _ => return None,
    })
}
