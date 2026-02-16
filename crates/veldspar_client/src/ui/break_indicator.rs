use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;

const BREAK_OVERLAY_VERTEX_COUNT: usize = 36;
const MAX_BREAK_OVERLAYS: usize = 64;
const MAX_BREAK_OVERLAY_VERTICES: usize = BREAK_OVERLAY_VERTEX_COUNT * MAX_BREAK_OVERLAYS;
const BREAK_OVERLAY_EXPANSION: f32 = 0.002;
const MAX_BREAK_OVERLAY_ALPHA: f32 = 0.6;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct BreakOverlayVertex {
    position: [f32; 3],
    alpha: f32,
}

pub struct BreakIndicatorRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

impl BreakIndicatorRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Break Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/break_overlay.wgsl"
                ))
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Break Overlay Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let attributes = &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32,
            },
        ];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Break Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<BreakOverlayVertex>() as wgpu::BufferAddress,
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

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Break Overlay Vertex Buffer"),
            size: (MAX_BREAK_OVERLAY_VERTICES * mem::size_of::<BreakOverlayVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_count: 0,
        }
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        local_block_pos: Option<IVec3>,
        local_progress: f32,
        remote_overlays: &[(IVec3, f32)],
    ) {
        let mut vertices: Vec<BreakOverlayVertex> = Vec::with_capacity(MAX_BREAK_OVERLAY_VERTICES);

        if let Some(pos) = local_block_pos {
            let alpha = local_progress.clamp(0.0, 1.0) * MAX_BREAK_OVERLAY_ALPHA;
            if alpha > 0.0 {
                vertices.extend_from_slice(&build_overlay_cube(pos, alpha));
            }
        }

        for (block_pos, progress) in remote_overlays.iter().copied() {
            if vertices.len() + BREAK_OVERLAY_VERTEX_COUNT > MAX_BREAK_OVERLAY_VERTICES {
                break;
            }
            let alpha = progress.clamp(0.0, 1.0) * MAX_BREAK_OVERLAY_ALPHA;
            if alpha <= 0.0 {
                continue;
            }
            vertices.extend_from_slice(&build_overlay_cube(block_pos, alpha));
        }

        if vertices.is_empty() {
            self.vertex_count = 0;
            return;
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        self.vertex_count = vertices.len() as u32;
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

fn build_overlay_cube(block_pos: IVec3, alpha: f32) -> [BreakOverlayVertex; BREAK_OVERLAY_VERTEX_COUNT] {
    let x = block_pos.x as f32;
    let y = block_pos.y as f32;
    let z = block_pos.z as f32;

    let min_x = x - BREAK_OVERLAY_EXPANSION;
    let max_x = x + 1.0 + BREAK_OVERLAY_EXPANSION;
    let min_y = y - BREAK_OVERLAY_EXPANSION;
    let max_y = y + 1.0 + BREAK_OVERLAY_EXPANSION;
    let min_z = z - BREAK_OVERLAY_EXPANSION;
    let max_z = z + 1.0 + BREAK_OVERLAY_EXPANSION;

    let p000 = [min_x, min_y, min_z];
    let p001 = [min_x, min_y, max_z];
    let p010 = [min_x, max_y, min_z];
    let p011 = [min_x, max_y, max_z];
    let p100 = [max_x, min_y, min_z];
    let p101 = [max_x, min_y, max_z];
    let p110 = [max_x, max_y, min_z];
    let p111 = [max_x, max_y, max_z];

    [
        vertex(p001, alpha),
        vertex(p101, alpha),
        vertex(p111, alpha),
        vertex(p001, alpha),
        vertex(p111, alpha),
        vertex(p011, alpha),
        vertex(p100, alpha),
        vertex(p000, alpha),
        vertex(p010, alpha),
        vertex(p100, alpha),
        vertex(p010, alpha),
        vertex(p110, alpha),
        vertex(p000, alpha),
        vertex(p001, alpha),
        vertex(p011, alpha),
        vertex(p000, alpha),
        vertex(p011, alpha),
        vertex(p010, alpha),
        vertex(p101, alpha),
        vertex(p100, alpha),
        vertex(p110, alpha),
        vertex(p101, alpha),
        vertex(p110, alpha),
        vertex(p111, alpha),
        vertex(p011, alpha),
        vertex(p111, alpha),
        vertex(p110, alpha),
        vertex(p011, alpha),
        vertex(p110, alpha),
        vertex(p010, alpha),
        vertex(p000, alpha),
        vertex(p100, alpha),
        vertex(p101, alpha),
        vertex(p000, alpha),
        vertex(p101, alpha),
        vertex(p001, alpha),
    ]
}

fn vertex(position: [f32; 3], alpha: f32) -> BreakOverlayVertex {
    BreakOverlayVertex { position, alpha }
}
