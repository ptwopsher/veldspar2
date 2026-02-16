use std::mem;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const SLOT_SIZE: f32 = 40.0;
const SLOT_SPACING: f32 = 4.0;
const HOTBAR_Y_OFFSET: f32 = 20.0;
const BORDER_WIDTH: f32 = 2.0;
const BLOCK_ICON_SIZE: f32 = 32.0;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct HotbarVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ScreenUniform {
    dimensions: [f32; 2],
    _padding: [f32; 2],
}

pub struct HotbarRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    screen_uniform_buffer: wgpu::Buffer,
    screen_bind_group: wgpu::BindGroup,
    vertex_count: u32,
    index_count: u32,
}

impl HotbarRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hotbar Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/hotbar.wgsl"
                ))
                .into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Hotbar Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Hotbar Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
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
            label: Some("Hotbar Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<HotbarVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
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

        let screen_uniform = ScreenUniform {
            dimensions: [width.max(1) as f32, height.max(1) as f32],
            _padding: [0.0, 0.0],
        };

        let screen_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hotbar Screen Uniform Buffer"),
            contents: bytemuck::bytes_of(&screen_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hotbar Screen Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_uniform_buffer.as_entire_binding(),
            }],
        });

        // Build initial geometry with slot 0 selected
        let (vertices, indices) = build_hotbar_geometry(width, height, 0);
        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hotbar Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hotbar Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            screen_uniform_buffer,
            screen_bind_group,
            vertex_count,
            index_count,
        }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        let screen_uniform = ScreenUniform {
            dimensions: [width.max(1) as f32, height.max(1) as f32],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.screen_uniform_buffer, 0, bytemuck::bytes_of(&screen_uniform));
    }

    pub fn update(&mut self, queue: &wgpu::Queue, width: u32, height: u32, selected_slot: u8) {
        let (vertices, indices) = build_hotbar_geometry(width, height, selected_slot);
        self.vertex_count = vertices.len() as u32;
        self.index_count = indices.len() as u32;

        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        if self.index_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.screen_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn build_hotbar_geometry(width: u32, height: u32, selected_slot: u8) -> (Vec<HotbarVertex>, Vec<u16>) {
    let width = width.max(1) as f32;
    let height = height.max(1) as f32;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Calculate total hotbar width: 9 slots * 40px + 8 gaps * 4px = 392px
    let total_width = 9.0 * SLOT_SIZE + 8.0 * SLOT_SPACING;
    let start_x = (width - total_width) / 2.0;
    let start_y = height - HOTBAR_Y_OFFSET - SLOT_SIZE;

    // Block colors for each slot (matching the BlockId -> slot mapping)
    let block_colors: [[f32; 4]; 9] = [
        [0.55, 0.45, 0.35, 1.0], // granite
        [0.45, 0.3, 0.15, 1.0],  // loam
        [0.2, 0.6, 0.15, 1.0],   // verdant_turf
        [0.85, 0.8, 0.55, 1.0],  // dune_sand
        [0.4, 0.25, 0.1, 1.0],   // timber_log
        [0.7, 0.55, 0.3, 1.0],   // hewn_plank
        [0.5, 0.5, 0.5, 1.0],    // rubblestone
        [0.6, 0.3, 0.2, 1.0],    // kiln_brick
        [0.95, 0.97, 1.0, 1.0],  // snowcap
    ];

    let bg_color = [0.2, 0.2, 0.2, 0.7];
    let border_color = [1.0, 1.0, 1.0, 0.9];

    for slot in 0..9 {
        let x = start_x + slot as f32 * (SLOT_SIZE + SLOT_SPACING);
        let y = start_y;

        let base_vertex = vertices.len() as u16;

        // Background quad
        add_quad(
            &mut vertices,
            &mut indices,
            base_vertex,
            x,
            y,
            SLOT_SIZE,
            SLOT_SIZE,
            bg_color,
        );

        // Block icon (centered in slot)
        let icon_offset = (SLOT_SIZE - BLOCK_ICON_SIZE) / 2.0;
        let icon_x = x + icon_offset;
        let icon_y = y + icon_offset;
        let icon_base_vertex = vertices.len() as u16;

        add_quad(
            &mut vertices,
            &mut indices,
            icon_base_vertex,
            icon_x,
            icon_y,
            BLOCK_ICON_SIZE,
            BLOCK_ICON_SIZE,
            block_colors[slot as usize],
        );

        // Selection border (if this is the selected slot)
        if slot == selected_slot {
            let border_base = vertices.len() as u16;

            // Top border
            add_quad(
                &mut vertices,
                &mut indices,
                border_base,
                x,
                y,
                SLOT_SIZE,
                BORDER_WIDTH,
                border_color,
            );

            // Bottom border
            let bottom_base = vertices.len() as u16;
            add_quad(
                &mut vertices,
                &mut indices,
                bottom_base,
                x,
                y + SLOT_SIZE - BORDER_WIDTH,
                SLOT_SIZE,
                BORDER_WIDTH,
                border_color,
            );

            // Left border
            let left_base = vertices.len() as u16;
            add_quad(
                &mut vertices,
                &mut indices,
                left_base,
                x,
                y,
                BORDER_WIDTH,
                SLOT_SIZE,
                border_color,
            );

            // Right border
            let right_base = vertices.len() as u16;
            add_quad(
                &mut vertices,
                &mut indices,
                right_base,
                x + SLOT_SIZE - BORDER_WIDTH,
                y,
                BORDER_WIDTH,
                SLOT_SIZE,
                border_color,
            );
        }
    }

    (vertices, indices)
}

fn add_quad(
    vertices: &mut Vec<HotbarVertex>,
    indices: &mut Vec<u16>,
    base_vertex: u16,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
) {
    // Add 4 vertices for the quad
    vertices.push(HotbarVertex {
        position: [x, y],
        color,
    });
    vertices.push(HotbarVertex {
        position: [x + width, y],
        color,
    });
    vertices.push(HotbarVertex {
        position: [x + width, y + height],
        color,
    });
    vertices.push(HotbarVertex {
        position: [x, y + height],
        color,
    });

    // Add 6 indices for 2 triangles
    indices.push(base_vertex);
    indices.push(base_vertex + 1);
    indices.push(base_vertex + 2);
    indices.push(base_vertex);
    indices.push(base_vertex + 2);
    indices.push(base_vertex + 3);
}
