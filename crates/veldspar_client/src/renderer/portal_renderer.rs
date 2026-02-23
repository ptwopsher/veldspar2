use std::f32::consts::PI;
use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat3, Mat4, Vec2, Vec3, Vec4};
use veldspar_client::portal::PortalColor;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::renderer::chunk_renderer::FrustumPlanes;

const PORTAL_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const RTT_SCALE: f32 = 0.5;
const PORTAL_SURFACE_OFFSET: f32 = 0.01;
const PORTAL_CLIP_BIAS: f32 = 0.025;
const PORTAL_EXIT_OFFSET: f32 = 0.05;
const PORTAL_FRAME_CELL_COUNT: usize = 10;

const PORTAL_ORANGE: [f32; 4] = [1.0, 165.0 / 255.0, 0.0, 1.0];
const PORTAL_BLUE: [f32; 4] = [0.0, 130.0 / 255.0, 1.0, 1.0];
const PORTAL_FRAME_ORANGE: [f32; 4] = [1.0, 140.0 / 255.0, 0.0, 1.0];
const PORTAL_FRAME_BLUE: [f32; 4] = [0.0, 100.0 / 255.0, 1.0, 1.0];

#[derive(Debug, Clone, Copy)]
pub struct PortalRenderPortal {
    pub center: Vec3,
    pub normal: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub half_extents: Vec2,
    pub linked_to: Option<usize>,
    pub frame_cells: [IVec3; PORTAL_FRAME_CELL_COUNT],
}

#[derive(Debug, Clone, Copy)]
pub struct PortalRenderInfo {
    pub portals: [Option<PortalRenderPortal>; 2],
    pub recursion_depth: u32,
}

impl Default for PortalRenderInfo {
    fn default() -> Self {
        Self {
            portals: [None, None],
            recursion_depth: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortalViewCamera {
    pub camera: Camera,
    pub view_proj: Mat4,
    pub position: Vec3,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PortalVertex {
    position: [f32; 3],
    uv: [f32; 2],
}

impl PortalVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<PortalVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct FrameCubeVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

impl FrameCubeVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<FrameCubeVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PortalFrameInstance {
    model: [[f32; 4]; 4],
}

impl PortalFrameInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<PortalFrameInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: (2 * mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: (3 * mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PortalParamsUniform {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    linked: f32,
    recursion: f32,
    _padding: [f32; 2],
}

impl Default for PortalParamsUniform {
    fn default() -> Self {
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            color: PORTAL_ORANGE,
            linked: 0.0,
            recursion: 0.0,
            _padding: [0.0; 2],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PortalFrameParamsUniform {
    color: [f32; 4],
    glow_params: [f32; 4],
}

struct PortalRenderTarget {
    _color_texture: wgpu::Texture,
    color_view: wgpu::TextureView,
    _depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    sample_bind_group: wgpu::BindGroup,
}

pub struct PortalRenderer {
    surface_pipeline: wgpu::RenderPipeline,
    frame_pipeline: wgpu::RenderPipeline,
    portal_texture_bind_group_layout: wgpu::BindGroupLayout,
    portal_params_bind_groups: [wgpu::BindGroup; 2],
    portal_params_buffers: [wgpu::Buffer; 2],
    frame_params_bind_groups: [wgpu::BindGroup; 2],
    surface_vertex_buffer: wgpu::Buffer,
    surface_index_buffer: wgpu::Buffer,
    surface_index_count: u32,
    frame_vertex_buffer: wgpu::Buffer,
    frame_index_buffer: wgpu::Buffer,
    frame_index_count: u32,
    frame_instance_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    targets: [PortalRenderTarget; 2],
    target_width: u32,
    target_height: u32,
    surface_format: wgpu::TextureFormat,
}

impl PortalRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let surface_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Portal Surface Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/portal_surface.wgsl"
                ))
                .into(),
            ),
        });
        let frame_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Portal Frame Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/portal_frame.wgsl"
                ))
                .into(),
            ),
        });

        let portal_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Portal Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let portal_params_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Portal Params Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let frame_params_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Portal Frame Params Bind Group Layout"),
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Portal RTT Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let portal_params_buffers = std::array::from_fn(|index| {
            let label = if index == PortalColor::Orange.index() {
                "Portal Params Buffer Orange"
            } else {
                "Portal Params Buffer Blue"
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::bytes_of(&PortalParamsUniform::default()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        });

        let portal_params_bind_groups = std::array::from_fn(|index| {
            let label = if index == PortalColor::Orange.index() {
                "Portal Params Bind Group Orange"
            } else {
                "Portal Params Bind Group Blue"
            };
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &portal_params_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: portal_params_buffers[index].as_entire_binding(),
                }],
            })
        });

        let frame_params_buffers: [wgpu::Buffer; 2] = std::array::from_fn(|index| {
            let label = if index == PortalColor::Orange.index() {
                "Portal Frame Params Buffer Orange"
            } else {
                "Portal Frame Params Buffer Blue"
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::bytes_of(&frame_params_uniform(index)),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });

        let frame_params_bind_groups = std::array::from_fn(|index| {
            let label = if index == PortalColor::Orange.index() {
                "Portal Frame Params Bind Group Orange"
            } else {
                "Portal Frame Params Bind Group Blue"
            };
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &frame_params_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_params_buffers[index].as_entire_binding(),
                }],
            })
        });

        let surface_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Portal Surface Pipeline Layout"),
                bind_group_layouts: &[
                    camera_bind_group_layout,
                    &portal_texture_bind_group_layout,
                    &portal_params_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let frame_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Portal Frame Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &frame_params_bind_group_layout],
            push_constant_ranges: &[],
        });

        let surface_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Portal Surface Pipeline"),
            layout: Some(&surface_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &surface_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[PortalVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &surface_shader,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: PORTAL_DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let frame_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Portal Frame Pipeline"),
            layout: Some(&frame_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &frame_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[FrameCubeVertex::desc(), PortalFrameInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &frame_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
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
                format: PORTAL_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let surface_vertices = [
            PortalVertex {
                position: [-1.0, -1.0, 0.0],
                uv: [0.0, 1.0],
            },
            PortalVertex {
                position: [1.0, -1.0, 0.0],
                uv: [1.0, 1.0],
            },
            PortalVertex {
                position: [1.0, 1.0, 0.0],
                uv: [1.0, 0.0],
            },
            PortalVertex {
                position: [-1.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
        ];
        let surface_indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let surface_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Portal Surface Vertex Buffer"),
            contents: bytemuck::cast_slice(&surface_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let surface_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Portal Surface Index Buffer"),
            contents: bytemuck::cast_slice(&surface_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (frame_vertices, frame_indices) = build_frame_cube_mesh();
        let frame_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Portal Frame Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(&frame_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let frame_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Portal Frame Cube Index Buffer"),
            contents: bytemuck::cast_slice(&frame_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let frame_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Portal Frame Instance Buffer"),
            size: (PORTAL_FRAME_CELL_COUNT * mem::size_of::<PortalFrameInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let targets = create_targets(
            device,
            1,
            1,
            surface_format,
            &portal_texture_bind_group_layout,
            &sampler,
        );

        Self {
            surface_pipeline,
            frame_pipeline,
            portal_texture_bind_group_layout,
            portal_params_bind_groups,
            portal_params_buffers,
            frame_params_bind_groups,
            surface_vertex_buffer,
            surface_index_buffer,
            surface_index_count: surface_indices.len() as u32,
            frame_vertex_buffer,
            frame_index_buffer,
            frame_index_count: frame_indices.len() as u32,
            frame_instance_buffer,
            sampler,
            targets,
            target_width: 1,
            target_height: 1,
            surface_format,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let target_width = scaled_dimension(width);
        let target_height = scaled_dimension(height);
        if target_width == self.target_width && target_height == self.target_height {
            return;
        }

        self.targets = create_targets(
            device,
            target_width,
            target_height,
            self.surface_format,
            &self.portal_texture_bind_group_layout,
            &self.sampler,
        );
        self.target_width = target_width;
        self.target_height = target_height;
    }

    pub fn render_portal_views<F>(
        &mut self,
        render_info: &PortalRenderInfo,
        main_camera: &Camera,
        main_camera_pos: Vec3,
        main_frustum: &FrustumPlanes,
        mut render_view: F,
    ) -> u32
    where
        F: FnMut(usize, &PortalViewCamera, &wgpu::TextureView, &wgpu::TextureView),
    {
        if render_info.recursion_depth == 0 {
            return 0;
        }

        let mut rendered_passes = 0;
        for source_index in 0..2 {
            let Some(source_portal) = render_info.portals[source_index] else {
                continue;
            };
            let Some(dest_index) = source_portal.linked_to else {
                continue;
            };
            let Some(dest_portal) = render_info.portals.get(dest_index).and_then(|p| *p) else {
                continue;
            };

            if !portal_is_visible(&source_portal, main_camera_pos, main_frustum) {
                continue;
            }

            let portal_view_camera =
                build_portal_view_camera(main_camera, &source_portal, &dest_portal);
            let target = &self.targets[source_index];
            render_view(
                source_index,
                &portal_view_camera,
                &target.color_view,
                &target.depth_view,
            );
            rendered_passes += 1;
        }

        rendered_passes
    }

    pub fn render_portal_surfaces<'a>(
        &'a self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        render_info: &PortalRenderInfo,
    ) -> u32 {
        render_pass.set_vertex_buffer(0, self.surface_vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            self.surface_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );

        let mut draw_calls = 0;
        for portal_index in 0..2 {
            let Some(portal) = render_info.portals[portal_index] else {
                continue;
            };

            let linked_portal_exists = portal
                .linked_to
                .and_then(|linked| render_info.portals.get(linked).and_then(|p| *p))
                .is_some();
            let params = PortalParamsUniform {
                model: portal_model_matrix(&portal).to_cols_array_2d(),
                color: portal_color(portal_index),
                linked: if linked_portal_exists { 1.0 } else { 0.0 },
                recursion: render_info.recursion_depth as f32,
                _padding: [0.0; 2],
            };
            queue.write_buffer(
                &self.portal_params_buffers[portal_index],
                0,
                bytemuck::bytes_of(&params),
            );

            render_pass.set_pipeline(&self.surface_pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.targets[portal_index].sample_bind_group, &[]);
            render_pass.set_bind_group(2, &self.portal_params_bind_groups[portal_index], &[]);
            render_pass.draw_indexed(0..self.surface_index_count, 0, 0..1);
            draw_calls += 1;
        }

        draw_calls
    }

    pub fn render_portal_frames<'a>(
        &'a self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        render_info: &PortalRenderInfo,
    ) -> u32 {
        render_pass.set_pipeline(&self.frame_pipeline);
        render_pass.set_vertex_buffer(0, self.frame_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.frame_instance_buffer.slice(..));
        render_pass.set_index_buffer(self.frame_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.set_bind_group(0, camera_bind_group, &[]);

        let mut draw_calls = 0;
        for portal_index in 0..2 {
            let Some(portal) = render_info.portals[portal_index] else {
                continue;
            };

            let instances = portal.frame_cells.map(frame_instance_for_cell);
            queue.write_buffer(
                &self.frame_instance_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );

            render_pass.set_bind_group(1, &self.frame_params_bind_groups[portal_index], &[]);
            render_pass.draw_indexed(0..self.frame_index_count, 0, 0..instances.len() as u32);
            draw_calls += 1;
        }

        draw_calls
    }
}

fn scaled_dimension(dimension: u32) -> u32 {
    ((dimension.max(1) as f32) * RTT_SCALE).round().max(1.0) as u32
}

fn create_targets(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    color_format: wgpu::TextureFormat,
    portal_texture_bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
) -> [PortalRenderTarget; 2] {
    std::array::from_fn(|index| {
        let color_label = format!("Portal RTT Color Texture {index}");
        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&color_label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_label = format!("Portal RTT Depth Texture {index}");
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&depth_label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PORTAL_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group_label = format!("Portal RTT Sample Bind Group {index}");
        let sample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&bind_group_label),
            layout: portal_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        PortalRenderTarget {
            _color_texture: color_texture,
            color_view,
            _depth_texture: depth_texture,
            depth_view,
            sample_bind_group,
        }
    })
}

fn portal_color(index: usize) -> [f32; 4] {
    if index == 0 {
        PORTAL_ORANGE
    } else {
        PORTAL_BLUE
    }
}

fn portal_frame_color(index: usize) -> [f32; 4] {
    if index == 0 {
        PORTAL_FRAME_ORANGE
    } else {
        PORTAL_FRAME_BLUE
    }
}

fn frame_params_uniform(index: usize) -> PortalFrameParamsUniform {
    let pulse_phase = if index == PortalColor::Orange.index() {
        0.0
    } else {
        PI * 0.5
    };

    PortalFrameParamsUniform {
        color: portal_frame_color(index),
        glow_params: [1.0, 0.08, 24.0, pulse_phase],
    }
}

fn frame_instance_for_cell(cell: IVec3) -> PortalFrameInstance {
    let translation = cell.as_vec3() + Vec3::splat(0.5);
    PortalFrameInstance {
        model: Mat4::from_translation(translation).to_cols_array_2d(),
    }
}

fn build_frame_cube_mesh() -> ([FrameCubeVertex; 24], [u16; 36]) {
    let vertices = [
        // +X
        FrameCubeVertex {
            position: [0.5, -0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
        },
        FrameCubeVertex {
            position: [0.5, -0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
        },
        FrameCubeVertex {
            position: [0.5, 0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
        },
        FrameCubeVertex {
            position: [0.5, 0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
        },
        // -X
        FrameCubeVertex {
            position: [-0.5, -0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
        },
        FrameCubeVertex {
            position: [-0.5, -0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
        },
        FrameCubeVertex {
            position: [-0.5, 0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
        },
        FrameCubeVertex {
            position: [-0.5, 0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
        },
        // +Y
        FrameCubeVertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
        },
        FrameCubeVertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
        },
        FrameCubeVertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
        },
        FrameCubeVertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
        },
        // -Y
        FrameCubeVertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
        },
        FrameCubeVertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
        },
        FrameCubeVertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
        },
        FrameCubeVertex {
            position: [-0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
        },
        // +Z
        FrameCubeVertex {
            position: [-0.5, -0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
        },
        FrameCubeVertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
        },
        FrameCubeVertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
        },
        FrameCubeVertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
        },
        // -Z
        FrameCubeVertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
        },
        FrameCubeVertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
        },
        FrameCubeVertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
        },
        FrameCubeVertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
        },
    ];

    let indices = [
        0, 1, 2, 0, 2, 3, // +X
        4, 5, 6, 4, 6, 7, // -X
        8, 9, 10, 8, 10, 11, // +Y
        12, 13, 14, 12, 14, 15, // -Y
        16, 17, 18, 16, 18, 19, // +Z
        20, 21, 22, 20, 22, 23, // -Z
    ];

    (vertices, indices)
}

fn portal_model_matrix(portal: &PortalRenderPortal) -> Mat4 {
    let normal = safe_normalize(portal.normal, Vec3::Z);
    let right = safe_normalize(portal.right, Vec3::X) * portal.half_extents.x.max(0.001);
    let up = safe_normalize(portal.up, Vec3::Y) * portal.half_extents.y.max(0.001);
    let translation = portal.center + normal * PORTAL_SURFACE_OFFSET;

    Mat4::from_cols(
        right.extend(0.0),
        up.extend(0.0),
        normal.extend(0.0),
        translation.extend(1.0),
    )
}

fn portal_is_visible(
    portal: &PortalRenderPortal,
    camera_pos: Vec3,
    frustum_planes: &FrustumPlanes,
) -> bool {
    let normal = safe_normalize(portal.normal, Vec3::Z);
    if (camera_pos - portal.center).dot(normal) <= 0.0 {
        return false;
    }

    let radius = portal.half_extents.length().max(0.5);
    sphere_in_frustum(frustum_planes, portal.center, radius)
}

fn sphere_in_frustum(planes: &FrustumPlanes, center: Vec3, radius: f32) -> bool {
    for plane in planes {
        let distance = plane[0] * center.x + plane[1] * center.y + plane[2] * center.z + plane[3];
        if distance < -radius {
            return false;
        }
    }
    true
}

fn safe_normalize(v: Vec3, fallback: Vec3) -> Vec3 {
    let n = v.normalize_or_zero();
    if n.length_squared() > 0.0 {
        n
    } else {
        fallback
    }
}

fn build_portal_view_camera(
    main_camera: &Camera,
    source: &PortalRenderPortal,
    dest: &PortalRenderPortal,
) -> PortalViewCamera {
    let source_right = safe_normalize(source.right, Vec3::X);
    let source_up = safe_normalize(source.up, Vec3::Y);
    let source_normal = safe_normalize(source.normal, Vec3::Z);
    let dest_right = safe_normalize(dest.right, Vec3::X);
    let dest_up = safe_normalize(dest.up, Vec3::Y);
    let dest_normal = safe_normalize(dest.normal, Vec3::Z);

    let source_basis = Mat3::from_cols(source_right, source_up, source_normal);
    let dest_basis = Mat3::from_cols(dest_right, dest_up, dest_normal);
    let rotation = dest_basis * Mat3::from_rotation_y(PI) * source_basis.transpose();

    let main_forward = safe_normalize(main_camera.forward_direction(), Vec3::NEG_Z);
    let mut main_right = main_forward.cross(Vec3::Y).normalize_or_zero();
    if main_right.length_squared() < 1e-6 {
        main_right = Vec3::X;
    }
    let main_up = safe_normalize(main_right.cross(main_forward), Vec3::Y);

    let transformed_forward = safe_normalize(rotation * main_forward, dest_normal);
    let transformed_up = safe_normalize(rotation * main_up, Vec3::Y);
    let position =
        dest.center + rotation * (main_camera.position - source.center) + dest_normal * PORTAL_EXIT_OFFSET;

    let view = Mat4::look_to_rh(position, transformed_forward, transformed_up);
    let projection = Mat4::perspective_rh(
        main_camera.fov,
        main_camera.aspect.max(0.0001),
        main_camera.near.max(0.0001),
        main_camera.far.max(main_camera.near + 0.0001),
    );

    let clip_normal = if (position - dest.center).dot(dest_normal) >= 0.0 {
        -dest_normal
    } else {
        dest_normal
    };
    let clip_point = dest.center + clip_normal * PORTAL_CLIP_BIAS;
    let plane_world = Vec4::new(
        clip_normal.x,
        clip_normal.y,
        clip_normal.z,
        -clip_normal.dot(clip_point),
    );
    let clip_plane_camera = view.inverse().transpose() * plane_world;
    let clipped_projection = apply_oblique_clip(projection, clip_plane_camera);

    let (yaw, pitch) = yaw_pitch_from_forward(transformed_forward);
    let camera = Camera {
        position,
        yaw,
        pitch,
        fov: main_camera.fov,
        aspect: main_camera.aspect,
        near: main_camera.near,
        far: main_camera.far,
    };

    PortalViewCamera {
        camera,
        view_proj: clipped_projection * view,
        position,
    }
}

fn yaw_pitch_from_forward(forward: Vec3) -> (f32, f32) {
    let direction = safe_normalize(forward, Vec3::NEG_Z);
    let yaw = direction.z.atan2(direction.x);
    let pitch = direction.y.asin();
    (yaw, pitch)
}

fn apply_oblique_clip(proj: Mat4, clip_plane_camera: Vec4) -> Mat4 {
    let q = proj.inverse()
        * Vec4::new(
            clip_plane_camera.x.signum(),
            clip_plane_camera.y.signum(),
            1.0,
            1.0,
        );
    let denom = clip_plane_camera.dot(q);
    if denom.abs() < 1e-5 {
        return proj;
    }

    let c = clip_plane_camera * (2.0 / denom);
    let mut m = proj.to_cols_array_2d();
    m[0][2] = c.x - m[0][3];
    m[1][2] = c.y - m[1][3];
    m[2][2] = c.z - m[2][3];
    m[3][2] = c.w - m[3][3];
    Mat4::from_cols_array_2d(&m)
}
