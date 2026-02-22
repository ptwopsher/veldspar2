use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::camera::Camera;

const WEATHER_DARKEN_STRENGTH: f32 = 0.3;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct SkyUniforms {
    inv_view_proj: [[f32; 4]; 4],
    horizon_color: [f32; 4],
    zenith_color: [f32; 4],
    time_of_day: f32,
    _pad: [f32; 7],
}

pub struct SkyRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl SkyRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sky Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/sky.wgsl"
                ))
                .into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sky Bind Group Layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sky Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sky Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[], // No vertex buffers needed
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
                cull_mode: None, // No culling for fullscreen triangle
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None, // Sky doesn't need depth
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create initial uniform buffer with identity matrix
        let initial_uniforms = SkyUniforms {
            inv_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            horizon_color: [0.529, 0.808, 0.922, 1.0],
            zenith_color: [0.25, 0.47, 0.82, 1.0],
            time_of_day: 0.5,
            _pad: [0.0; 7],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sky Uniform Buffer"),
            contents: bytemuck::bytes_of(&initial_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sky Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, camera: &Camera, time_of_day: f32, weather_dim: f32) {
        let inv_view_proj = camera.view_projection_matrix().inverse();
        let (mut horizon_color, mut zenith_color) = sky_colors(time_of_day);
        let weather_factor = 1.0 - WEATHER_DARKEN_STRENGTH * weather_dim.clamp(0.0, 1.0);
        for channel in 0..3 {
            horizon_color[channel] *= weather_factor;
            zenith_color[channel] *= weather_factor;
        }

        let uniforms = SkyUniforms {
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            horizon_color,
            zenith_color,
            time_of_day,
            _pad: [0.0; 7],
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Draw 3 vertices for fullscreen triangle
    }
}

pub(crate) fn sky_horizon_color(time_of_day: f32) -> [f32; 4] {
    sky_colors(time_of_day).0
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn cosine_blend(t: f32) -> f32 {
    (1.0 - (t * std::f32::consts::PI).cos()) * 0.5
}

fn sky_colors(time_of_day: f32) -> ([f32; 4], [f32; 4]) {
    // time: 0.0=midnight, 0.25=sunrise, 0.5=noon, 0.75=sunset, 1.0=midnight

    // Day colors (noon)
    let day_horizon = [0.529, 0.808, 0.922, 1.0];
    let day_zenith = [0.25, 0.47, 0.82, 1.0];

    // Night colors
    let night_horizon = [0.05, 0.05, 0.12, 1.0];
    let night_zenith = [0.01, 0.01, 0.05, 1.0];

    // Sunrise/sunset colors
    let sunset_horizon = [0.95, 0.55, 0.25, 1.0];
    let sunset_zenith = [0.35, 0.30, 0.60, 1.0];

    // Smooth interpolation around sunrise (0.25) and sunset (0.75)
    let t = time_of_day;

    // Determine which phase we're in and blend accordingly
    if t < 0.2 {
        // Late night to early sunrise
        let smooth_blend = cosine_blend(t / 0.2);
        (
            lerp_color(night_horizon, sunset_horizon, smooth_blend),
            lerp_color(night_zenith, sunset_zenith, smooth_blend),
        )
    } else if t < 0.3 {
        // Sunrise transition
        let smooth_blend = cosine_blend((t - 0.2) / 0.1);
        (
            lerp_color(sunset_horizon, day_horizon, smooth_blend),
            lerp_color(sunset_zenith, day_zenith, smooth_blend),
        )
    } else if t < 0.7 {
        // Full day
        (day_horizon, day_zenith)
    } else if t < 0.8 {
        // Sunset transition
        let smooth_blend = cosine_blend((t - 0.7) / 0.1);
        (
            lerp_color(day_horizon, sunset_horizon, smooth_blend),
            lerp_color(day_zenith, sunset_zenith, smooth_blend),
        )
    } else {
        // Evening to night
        let smooth_blend = cosine_blend((t - 0.8) / 0.2);
        (
            lerp_color(sunset_horizon, night_horizon, smooth_blend),
            lerp_color(sunset_zenith, night_zenith, smooth_blend),
        )
    }
}
