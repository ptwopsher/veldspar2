use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};
use wgpu::util::DeviceExt;

const MAX_PARTICLES: usize = 8192;
const MIN_BREAK_PARTICLES: usize = 8;
const MAX_BREAK_PARTICLES: usize = 12;
const MIN_WALK_PARTICLES: usize = 2;
const MAX_WALK_PARTICLES: usize = 4;
const MIN_PLACE_PARTICLES: usize = 4;
const MAX_PLACE_PARTICLES: usize = 7;
const GRAVITY: f32 = -18.0;
const PARTICLE_SIZE: f32 = 0.12;
const DEFAULT_PARTICLE_SIZE: [f32; 2] = [1.0, 1.0];
const RAIN_PARTICLE_SIZE: [f32; 2] = [0.24, 2.8];
const SNOW_PARTICLE_SIZE: [f32; 2] = [0.95, 0.95];
const RAIN_COLOR: [f32; 4] = [0.66, 0.74, 0.86, 0.85];
const SNOW_COLOR: [f32; 4] = [0.95, 0.97, 1.0, 0.92];

#[derive(Clone, Debug)]
pub struct Particle {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub color: [f32; 4],
    pub lifetime: f32,
    pub age: f32,
    pub gravity_scale: f32,
    pub size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ParticleVertex {
    quad_pos: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ParticleInstance {
    position: [f32; 3],
    _pad0: f32,
    color: [f32; 4],
    age: f32,
    lifetime: f32,
    size: [f32; 2],
    _pad1: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ParticleParams {
    size: f32,
    _pad: [f32; 3],
    camera_right: [f32; 4],
    camera_up: [f32; 4],
}

pub struct ParticleRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    particles: Vec<Particle>,
    instance_count: u32,
    rng_state: u64,
}

impl ParticleRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/particles.wgsl"
                ))
                .into(),
            ),
        });

        let particle_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Params Bind Group Layout"),
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
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &particle_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<ParticleVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        }],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                                shader_location: 5,
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let quad_vertices = [
            ParticleVertex {
                quad_pos: [-0.5, -0.5],
            },
            ParticleVertex {
                quad_pos: [0.5, -0.5],
            },
            ParticleVertex {
                quad_pos: [-0.5, 0.5],
            },
            ParticleVertex {
                quad_pos: [0.5, 0.5],
            },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Instance Buffer"),
            size: (MAX_PARTICLES * mem::size_of::<ParticleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params = ParticleParams {
            size: PARTICLE_SIZE,
            _pad: [0.0; 3],
            camera_right: [1.0, 0.0, 0.0, 0.0],
            camera_up: [0.0, 1.0, 0.0, 0.0],
        };

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Params Buffer"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Params Bind Group"),
            layout: &particle_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            vertex_buffer,
            instance_buffer,
            bind_group,
            params_buffer,
            particles: Vec::with_capacity(MAX_PARTICLES),
            instance_count: 0,
            rng_state: 0xA4B3_C2D1_E0F9_8765,
        }
    }

    pub fn spawn_break_particles(&mut self, block_pos: IVec3, color: [f32; 3]) {
        let seed = mix_seed(block_pos, self.rng_state ^ self.particles.len() as u64);
        self.rng_state ^= seed;
        if self.rng_state == 0 {
            self.rng_state = 0x9E37_79B9_7F4A_7C15;
        }

        let spawn_count =
            MIN_BREAK_PARTICLES + (self.next_rand_u32() as usize % (MAX_BREAK_PARTICLES - MIN_BREAK_PARTICLES + 1));

        let base_pos = Vec3::new(
            block_pos.x as f32 + 0.5,
            block_pos.y as f32 + 0.5,
            block_pos.z as f32 + 0.5,
        );

        for _ in 0..spawn_count {
            let spawn_offset = Vec3::new(
                self.rand_range(-0.35, 0.35),
                self.rand_range(-0.35, 0.35),
                self.rand_range(-0.35, 0.35),
            );

            let mut direction = Vec3::new(
                self.rand_range(-1.0, 1.0),
                self.rand_range(0.2, 1.2),
                self.rand_range(-1.0, 1.0),
            );
            if direction.length_squared() < 1e-4 {
                direction = Vec3::Y;
            } else {
                direction = direction.normalize();
            }

            let speed = self.rand_range(2.0, 4.5);
            let lifetime = self.rand_range(0.4, 0.6);

            self.push_particle(Particle {
                position: (base_pos + spawn_offset).to_array(),
                velocity: (direction * speed).to_array(),
                color: [color[0], color[1], color[2], 1.0],
                lifetime,
                age: 0.0,
                gravity_scale: 1.0,
                size: DEFAULT_PARTICLE_SIZE,
            });
        }
    }

    pub fn spawn_walk_particles(&mut self, position: Vec3, color: [f32; 3]) {
        let block_pos = position.floor().as_ivec3();
        let seed = mix_seed(block_pos, self.rng_state ^ 0xD1CE_BAAD_5511_09E5);
        self.rng_state ^= seed;
        if self.rng_state == 0 {
            self.rng_state = 0x9E37_79B9_7F4A_7C15;
        }

        let spawn_count =
            MIN_WALK_PARTICLES + (self.next_rand_u32() as usize % (MAX_WALK_PARTICLES - MIN_WALK_PARTICLES + 1));

        for _ in 0..spawn_count {
            let spawn_offset = Vec3::new(
                self.rand_range(-0.18, 0.18),
                self.rand_range(0.0, 0.08),
                self.rand_range(-0.18, 0.18),
            );

            let velocity = Vec3::new(
                self.rand_range(-0.9, 0.9),
                self.rand_range(0.5, 1.4),
                self.rand_range(-0.9, 0.9),
            );
            let lifetime = self.rand_range(0.18, 0.3);

            self.push_particle(Particle {
                position: (position + spawn_offset).to_array(),
                velocity: velocity.to_array(),
                color: [color[0], color[1], color[2], 0.72],
                lifetime,
                age: 0.0,
                gravity_scale: 1.0,
                size: DEFAULT_PARTICLE_SIZE,
            });
        }
    }

    pub fn spawn_place_particles(&mut self, block_pos: IVec3, color: [f32; 3]) {
        let seed = mix_seed(block_pos, self.rng_state ^ 0x31A6_EA51_7A11_CE5C);
        self.rng_state ^= seed;
        if self.rng_state == 0 {
            self.rng_state = 0x9E37_79B9_7F4A_7C15;
        }

        let spawn_count =
            MIN_PLACE_PARTICLES + (self.next_rand_u32() as usize % (MAX_PLACE_PARTICLES - MIN_PLACE_PARTICLES + 1));

        let base_pos = Vec3::new(
            block_pos.x as f32 + 0.5,
            block_pos.y as f32 + 0.2,
            block_pos.z as f32 + 0.5,
        );

        for _ in 0..spawn_count {
            let mut direction = Vec3::new(
                self.rand_range(-1.0, 1.0),
                self.rand_range(0.1, 0.9),
                self.rand_range(-1.0, 1.0),
            );
            if direction.length_squared() < 1e-4 {
                direction = Vec3::Y;
            } else {
                direction = direction.normalize();
            }

            let spawn_offset = Vec3::new(
                self.rand_range(-0.22, 0.22),
                self.rand_range(0.0, 0.4),
                self.rand_range(-0.22, 0.22),
            );
            let speed = self.rand_range(0.8, 1.9);
            let lifetime = self.rand_range(0.2, 0.4);

            self.push_particle(Particle {
                position: (base_pos + spawn_offset).to_array(),
                velocity: (direction * speed).to_array(),
                color: [color[0], color[1], color[2], 0.85],
                lifetime,
                age: 0.0,
                gravity_scale: 1.0,
                size: DEFAULT_PARTICLE_SIZE,
            });
        }
    }

    pub fn spawn_rain_particles(&mut self, spawn_positions: &[Vec3]) {
        if spawn_positions.is_empty() {
            return;
        }

        let seed = mix_seed(
            spawn_positions[0].floor().as_ivec3(),
            self.rng_state ^ 0xEA15_8A41_7BC2_90D3 ^ spawn_positions.len() as u64,
        );
        self.rng_state ^= seed;
        if self.rng_state == 0 {
            self.rng_state = 0x9E37_79B9_7F4A_7C15;
        }

        for &spawn_pos in spawn_positions {
            let jitter = Vec3::new(
                self.rand_range(-0.18, 0.18),
                self.rand_range(-0.12, 0.12),
                self.rand_range(-0.18, 0.18),
            );
            let speed = Vec3::new(
                self.rand_range(-0.35, 0.35),
                self.rand_range(-30.0, -22.0),
                self.rand_range(-0.35, 0.35),
            );
            let brightness = self.rand_range(0.9, 1.08);
            let lifetime = self.rand_range(0.3, 0.5);

            self.push_particle(Particle {
                position: (spawn_pos + jitter).to_array(),
                velocity: speed.to_array(),
                color: [
                    (RAIN_COLOR[0] * brightness).min(1.0),
                    (RAIN_COLOR[1] * brightness).min(1.0),
                    (RAIN_COLOR[2] * brightness).min(1.0),
                    RAIN_COLOR[3],
                ],
                lifetime,
                age: 0.0,
                gravity_scale: 2.1,
                size: RAIN_PARTICLE_SIZE,
            });
        }
    }

    pub fn spawn_snow_particles(&mut self, spawn_positions: &[Vec3]) {
        if spawn_positions.is_empty() {
            return;
        }

        let seed = mix_seed(
            spawn_positions[0].floor().as_ivec3(),
            self.rng_state ^ 0x2D47_5FCB_A5E1_144F ^ spawn_positions.len() as u64,
        );
        self.rng_state ^= seed;
        if self.rng_state == 0 {
            self.rng_state = 0x9E37_79B9_7F4A_7C15;
        }

        for &spawn_pos in spawn_positions {
            let jitter = Vec3::new(
                self.rand_range(-0.2, 0.2),
                self.rand_range(-0.1, 0.16),
                self.rand_range(-0.2, 0.2),
            );
            let drift = Vec3::new(
                self.rand_range(-0.85, 0.85),
                self.rand_range(-4.2, -1.9),
                self.rand_range(-0.85, 0.85),
            );
            let whiteness = self.rand_range(0.92, 1.0);
            let lifetime = self.rand_range(1.0, 2.0);

            self.push_particle(Particle {
                position: (spawn_pos + jitter).to_array(),
                velocity: drift.to_array(),
                color: [
                    SNOW_COLOR[0] * whiteness,
                    SNOW_COLOR[1] * whiteness,
                    SNOW_COLOR[2] * whiteness,
                    SNOW_COLOR[3],
                ],
                lifetime,
                age: 0.0,
                gravity_scale: 0.22,
                size: SNOW_PARTICLE_SIZE,
            });
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32, camera_right: Vec3, camera_up: Vec3) {
        let dt = dt.max(0.0);

        // Update params uniform with current camera billboard axes
        let params = ParticleParams {
            size: PARTICLE_SIZE,
            _pad: [0.0; 3],
            camera_right: [camera_right.x, camera_right.y, camera_right.z, 0.0],
            camera_up: [camera_up.x, camera_up.y, camera_up.z, 0.0],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        if dt > 0.0 {
            self.particles.retain_mut(|particle| {
                particle.age += dt;
                if particle.age >= particle.lifetime {
                    return false;
                }

                let mut velocity = Vec3::from_array(particle.velocity);
                velocity.y += GRAVITY * particle.gravity_scale * dt;
                velocity *= (1.0 - 2.0 * dt).clamp(0.0, 1.0);

                let mut position = Vec3::from_array(particle.position);
                position += velocity * dt;

                particle.velocity = velocity.to_array();
                particle.position = position.to_array();
                true
            });
        }

        let mut instances = Vec::with_capacity(self.particles.len());
        for particle in &self.particles {
            instances.push(ParticleInstance {
                position: particle.position,
                _pad0: 0.0,
                color: particle.color,
                age: particle.age,
                lifetime: particle.lifetime,
                size: particle.size,
                _pad1: [0.0; 2],
            });
        }

        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }

        self.instance_count = instances.len() as u32;
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>, camera_bind_group: &wgpu::BindGroup) {
        if self.instance_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(0..4, 0..self.instance_count);
    }

    pub fn clear(&mut self) {
        self.particles.clear();
        self.instance_count = 0;
    }

    fn next_rand_u32(&mut self) -> u32 {
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D)) >> 32) as u32
    }

    fn rand_f32(&mut self) -> f32 {
        self.next_rand_u32() as f32 / u32::MAX as f32
    }

    fn rand_range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.rand_f32()
    }

    fn push_particle(&mut self, particle: Particle) {
        if self.particles.len() >= MAX_PARTICLES {
            self.particles.swap_remove(0);
        }
        self.particles.push(particle);
    }
}

fn mix_seed(block_pos: IVec3, salt: u64) -> u64 {
    let x = (block_pos.x as i64 as u64).wrapping_mul(0x9E37_79B1_85EB_CA87);
    let y = (block_pos.y as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let z = (block_pos.z as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9);

    let mut mixed = x ^ y ^ z ^ salt;
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}
