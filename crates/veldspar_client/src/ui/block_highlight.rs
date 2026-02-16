use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;


#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct HighlightVertex {
    position: [f32; 3],
}

pub struct BlockHighlightRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

impl BlockHighlightRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Highlight Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/highlight.wgsl"
                ))
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Highlight Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let attributes = &[wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x3,
        }];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Highlight Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<HighlightVertex>() as wgpu::BufferAddress,
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
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create empty vertex buffer (24 vertices * 12 bytes each)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Highlight Vertex Buffer"),
            size: (24 * mem::size_of::<HighlightVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_count: 0,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, block_pos: Option<IVec3>) {
        if let Some(pos) = block_pos {
            let vertices = build_wireframe_cube(pos);
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            self.vertex_count = 24;
        } else {
            self.vertex_count = 0;
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>, camera_bind_group: &wgpu::BindGroup) {
        if self.vertex_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }
}

fn build_wireframe_cube(block_pos: IVec3) -> [HighlightVertex; 24] {
    let x = block_pos.x as f32;
    let y = block_pos.y as f32;
    let z = block_pos.z as f32;

    // Slightly expand the cube to avoid z-fighting
    let min_x = x - 0.001;
    let max_x = x + 1.001;
    let min_y = y - 0.001;
    let max_y = y + 1.001;
    let min_z = z - 0.001;
    let max_z = z + 1.001;

    [
        // Bottom face (4 edges = 8 vertices)
        HighlightVertex { position: [min_x, min_y, min_z] },
        HighlightVertex { position: [max_x, min_y, min_z] },

        HighlightVertex { position: [max_x, min_y, min_z] },
        HighlightVertex { position: [max_x, min_y, max_z] },

        HighlightVertex { position: [max_x, min_y, max_z] },
        HighlightVertex { position: [min_x, min_y, max_z] },

        HighlightVertex { position: [min_x, min_y, max_z] },
        HighlightVertex { position: [min_x, min_y, min_z] },

        // Top face (4 edges = 8 vertices)
        HighlightVertex { position: [min_x, max_y, min_z] },
        HighlightVertex { position: [max_x, max_y, min_z] },

        HighlightVertex { position: [max_x, max_y, min_z] },
        HighlightVertex { position: [max_x, max_y, max_z] },

        HighlightVertex { position: [max_x, max_y, max_z] },
        HighlightVertex { position: [min_x, max_y, max_z] },

        HighlightVertex { position: [min_x, max_y, max_z] },
        HighlightVertex { position: [min_x, max_y, min_z] },

        // Vertical edges (4 edges = 8 vertices)
        HighlightVertex { position: [min_x, min_y, min_z] },
        HighlightVertex { position: [min_x, max_y, min_z] },

        HighlightVertex { position: [max_x, min_y, min_z] },
        HighlightVertex { position: [max_x, max_y, min_z] },

        HighlightVertex { position: [max_x, min_y, max_z] },
        HighlightVertex { position: [max_x, max_y, max_z] },

        HighlightVertex { position: [min_x, min_y, max_z] },
        HighlightVertex { position: [min_x, max_y, max_z] },
    ]
}
