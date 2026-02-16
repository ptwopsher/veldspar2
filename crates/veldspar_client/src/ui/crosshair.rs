use std::mem;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const CROSSHAIR_LENGTH_PX: f32 = 20.0;
const CROSSHAIR_THICKNESS_PX: f32 = 2.0;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UiVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub struct CrosshairRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl CrosshairRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
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
            label: Some("Crosshair Pipeline Layout"),
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
            label: Some("Crosshair Pipeline"),
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

        let vertices = build_vertices(width, height);
        let indices = build_indices();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Crosshair Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Crosshair Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    pub fn resize(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let vertices = build_vertices(width, height);
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn build_vertices(width: u32, height: u32) -> [UiVertex; 8] {
    let width = width.max(1) as f32;
    let height = height.max(1) as f32;
    let color = [1.0, 1.0, 1.0, 1.0];

    let half_length_x = (CROSSHAIR_LENGTH_PX * 0.5) * (2.0 / width);
    let half_length_y = (CROSSHAIR_LENGTH_PX * 0.5) * (2.0 / height);
    let half_thickness_x = (CROSSHAIR_THICKNESS_PX * 0.5) * (2.0 / width);
    let half_thickness_y = (CROSSHAIR_THICKNESS_PX * 0.5) * (2.0 / height);

    [
        UiVertex {
            position: [-half_length_x, -half_thickness_y],
            color,
        },
        UiVertex {
            position: [half_length_x, -half_thickness_y],
            color,
        },
        UiVertex {
            position: [half_length_x, half_thickness_y],
            color,
        },
        UiVertex {
            position: [-half_length_x, half_thickness_y],
            color,
        },
        UiVertex {
            position: [-half_thickness_x, -half_length_y],
            color,
        },
        UiVertex {
            position: [half_thickness_x, -half_length_y],
            color,
        },
        UiVertex {
            position: [half_thickness_x, half_length_y],
            color,
        },
        UiVertex {
            position: [-half_thickness_x, half_length_y],
            color,
        },
    ]
}

fn build_indices() -> [u16; 12] {
    [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]
}
