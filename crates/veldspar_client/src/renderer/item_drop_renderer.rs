use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

const MAX_ITEM_DROPS: usize = 2048;
const ITEM_SCALE: f32 = 0.25;
const ITEM_BOB_AMPLITUDE: f32 = 0.08;
const ITEM_BOB_SPEED: f32 = 2.8;
const ITEM_ROTATION_SPEED: f32 = 1.7;

#[derive(Clone, Copy, Debug)]
pub struct ItemDropRenderData {
    pub position: Vec3,
    pub color: [f32; 3],
    pub age: f32,
    pub tile_origin: Option<[f32; 2]>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct ItemDropVertex {
    pub(crate) position: [f32; 3],
    pub(crate) shade: f32,
    pub(crate) uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ItemDropInstance {
    model_matrix: [[f32; 4]; 4],
    color: [f32; 4],
    tile_origin: [f32; 2],
    _padding: [f32; 2],
}

pub struct ItemDropRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    num_indices: u32,
    instance_count: u32,
    atlas_bind_group: wgpu::BindGroup,
}

impl ItemDropRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        atlas_bind_group: wgpu::BindGroup,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Item Drop Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/item_drop.wgsl"
                ))
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Item Drop Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Item Drop Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<ItemDropVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
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
                            wgpu::VertexAttribute {
                                offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<ItemDropInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: (2 * mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: (3 * mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                                shader_location: 6,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            // color
                            wgpu::VertexAttribute {
                                offset: mem::size_of::<[[f32; 4]; 4]>() as wgpu::BufferAddress,
                                shader_location: 7,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            // tile_origin
                            wgpu::VertexAttribute {
                                offset: (mem::size_of::<[[f32; 4]; 4]>()
                                    + mem::size_of::<[f32; 4]>())
                                    as wgpu::BufferAddress,
                                shader_location: 8,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let (vertices, indices) = build_cube_mesh();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Item Drop Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Item Drop Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Item Drop Instance Buffer"),
            size: (MAX_ITEM_DROPS * mem::size_of::<ItemDropInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            num_indices: indices.len() as u32,
            instance_count: 0,
            atlas_bind_group,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, drops: &[ItemDropRenderData]) {
        let clamped_len = drops.len().min(MAX_ITEM_DROPS);
        if clamped_len == 0 {
            self.instance_count = 0;
            return;
        }

        let mut instances = Vec::with_capacity(clamped_len);
        for drop in drops.iter().take(clamped_len) {
            let bob = (drop.age * ITEM_BOB_SPEED).sin() * ITEM_BOB_AMPLITUDE;
            let model_matrix = Mat4::from_translation(drop.position + Vec3::Y * bob)
                * Mat4::from_rotation_y(drop.age * ITEM_ROTATION_SPEED)
                * Mat4::from_scale(Vec3::splat(ITEM_SCALE));

            let (color_a, tile_origin) = match drop.tile_origin {
                Some(origin) => (1.0, origin),
                None => (0.0, [0.0, 0.0]),
            };

            instances.push(ItemDropInstance {
                model_matrix: model_matrix.to_cols_array_2d(),
                color: [drop.color[0], drop.color[1], drop.color[2], color_a],
                tile_origin,
                _padding: [0.0; 2],
            });
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        self.instance_count = instances.len() as u32;
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if self.instance_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.atlas_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..self.instance_count);
    }

    pub fn clear(&mut self) {
        self.instance_count = 0;
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

fn build_cube_mesh() -> (Vec<ItemDropVertex>, Vec<u16>) {
    let x0 = -0.5;
    let y0 = -0.5;
    let z0 = -0.5;
    let x1 = 0.5;
    let y1 = 0.5;
    let z1 = 0.5;

    let front = 1.0;
    let back = 0.9;
    let side = 0.82;
    let top = 1.15;
    let bottom = 0.7;

    // Each face has 4 vertices with UV coords (0,0) (1,0) (1,1) (0,1)
    let vertices = vec![
        // Front face (+Z)
        ItemDropVertex { position: [x0, y0, z1], shade: front, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x1, y0, z1], shade: front, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z1], shade: front, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x0, y1, z1], shade: front, uv: [0.0, 0.0] },
        // Back face (-Z)
        ItemDropVertex { position: [x1, y0, z0], shade: back, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x0, y0, z0], shade: back, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x0, y1, z0], shade: back, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x1, y1, z0], shade: back, uv: [0.0, 0.0] },
        // Right face (+X)
        ItemDropVertex { position: [x1, y0, z1], shade: side, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x1, y0, z0], shade: side, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z0], shade: side, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x1, y1, z1], shade: side, uv: [0.0, 0.0] },
        // Left face (-X)
        ItemDropVertex { position: [x0, y0, z0], shade: side, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x0, y0, z1], shade: side, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x0, y1, z1], shade: side, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x0, y1, z0], shade: side, uv: [0.0, 0.0] },
        // Top face (+Y)
        ItemDropVertex { position: [x0, y1, z1], shade: top, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z1], shade: top, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z0], shade: top, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x0, y1, z0], shade: top, uv: [0.0, 0.0] },
        // Bottom face (-Y)
        ItemDropVertex { position: [x0, y0, z0], shade: bottom, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x1, y0, z0], shade: bottom, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x1, y0, z1], shade: bottom, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x0, y0, z1], shade: bottom, uv: [0.0, 0.0] },
    ];

    let mut indices = Vec::with_capacity(36);
    for face in 0..6u16 {
        let base = face * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    (vertices, indices)
}
