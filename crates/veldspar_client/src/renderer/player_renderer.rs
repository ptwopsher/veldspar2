use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};
use wgpu::util::DeviceExt;
use veldspar_shared::block::BlockId;
use veldspar_shared::inventory::ItemId;

use crate::renderer::atlas::AtlasMapping;
use crate::renderer::item_drop_renderer::ItemDropVertex;

const MAX_PLAYER_INSTANCES: usize = 32;
const INITIAL_MOB_VERTEX_CAPACITY: usize = 24;
const INITIAL_MOB_INDEX_CAPACITY: usize = 36;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PlayerVertex {
    position: [f32; 3],
    color: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PlayerInstance {
    model_matrix: [[f32; 4]; 4],
    animation_phase: f32,
    head_pitch: f32,
    attack_animation: f32,
    _padding: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct HeldBlockInstance {
    model_matrix: [[f32; 4]; 4],
    color: [f32; 4],
    tile_origin: [f32; 2],
    _padding: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct RemotePlayer {
    pub player_id: u64,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub animation_phase: f32,
    pub attack_animation: f32,
    pub is_crouching: bool,
}

#[derive(Clone, Debug)]
pub struct MobRenderInfo {
    pub position: Vec3,
    pub yaw: f32,
    pub width: f32,
    pub height: f32,
    pub color: [f32; 3],
    pub hurt_flash: bool,
}

struct FirstPersonHand {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    num_indices: u32,
}

pub struct PlayerRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    num_indices: u32,
    first_person_hand: FirstPersonHand,
    hand_block_vertex_buffer: Option<wgpu::Buffer>,
    hand_block_index_buffer: Option<wgpu::Buffer>,
    hand_block_index_count: u32,
    hand_block_instance_buffer: wgpu::Buffer,
    hand_block_tile_origin: Option<[f32; 2]>,
    hand_block_selected_item: Option<ItemId>,
    hand_block_is_flat: bool,
    mob_vertex_buffer: wgpu::Buffer,
    mob_index_buffer: wgpu::Buffer,
    mob_instance_buffer: wgpu::Buffer,
    mob_index_count: u32,
    mob_vertex_capacity: usize,
    mob_index_capacity: usize,
    players: Vec<RemotePlayer>,
}

impl PlayerRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Player Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/shaders/player.wgsl"
                ))
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Player Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_attributes = &[
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
        ];

        let instance_attributes = &[
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
            wgpu::VertexAttribute {
                offset: mem::size_of::<[[f32; 4]; 4]>() as wgpu::BufferAddress,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: (mem::size_of::<[[f32; 4]; 4]>() + mem::size_of::<f32>()) as wgpu::BufferAddress,
                shader_location: 7,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: (mem::size_of::<[[f32; 4]; 4]>() + 2 * mem::size_of::<f32>()) as wgpu::BufferAddress,
                shader_location: 8,
                format: wgpu::VertexFormat::Float32,
            },
        ];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Player Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<PlayerVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: vertex_attributes,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<PlayerInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: instance_attributes,
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let (vertices, indices) = build_player_model();
        let num_indices = indices.len() as u32;

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Player Vertex Buffer"),
            size: (vertices.len() * mem::size_of::<PlayerVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Player Index Buffer"),
            size: (indices.len() * mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Player Instance Buffer"),
            size: (MAX_PLAYER_INSTANCES * mem::size_of::<PlayerInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (hand_vertices, hand_indices) = build_first_person_hand_model();
        let hand_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("First Person Hand Vertex Buffer"),
            size: (hand_vertices.len() * mem::size_of::<PlayerVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hand_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("First Person Hand Index Buffer"),
            size: (hand_indices.len() * mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hand_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("First Person Hand Instance Buffer"),
            size: mem::size_of::<PlayerInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hand_block_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("First Person Held Block Instance Buffer"),
            contents: bytemuck::bytes_of(&HeldBlockInstance {
                model_matrix: Mat4::IDENTITY.to_cols_array_2d(),
                color: [1.0, 1.0, 1.0, 0.0],
                tile_origin: [0.0, 0.0],
                _padding: [0.0; 2],
            }),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let mob_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mob Vertex Buffer"),
            size: (INITIAL_MOB_VERTEX_CAPACITY * mem::size_of::<PlayerVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mob_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mob Index Buffer"),
            size: (INITIAL_MOB_INDEX_CAPACITY * mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mob_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mob Instance Buffer"),
            contents: bytemuck::bytes_of(&PlayerInstance {
                model_matrix: Mat4::IDENTITY.to_cols_array_2d(),
                animation_phase: 0.0,
                head_pitch: 0.0,
                attack_animation: 0.0,
                _padding: 0.0,
            }),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            num_indices,
            first_person_hand: FirstPersonHand {
                vertex_buffer: hand_vertex_buffer,
                index_buffer: hand_index_buffer,
                instance_buffer: hand_instance_buffer,
                num_indices: hand_indices.len() as u32,
            },
            hand_block_vertex_buffer: None,
            hand_block_index_buffer: None,
            hand_block_index_count: 0,
            hand_block_instance_buffer,
            hand_block_tile_origin: None,
            hand_block_selected_item: None,
            hand_block_is_flat: false,
            mob_vertex_buffer,
            mob_index_buffer,
            mob_instance_buffer,
            mob_index_count: 0,
            mob_vertex_capacity: INITIAL_MOB_VERTEX_CAPACITY,
            mob_index_capacity: INITIAL_MOB_INDEX_CAPACITY,
            players: Vec::new(),
        }
    }

    pub fn init_buffers(&self, queue: &wgpu::Queue) {
        let (vertices, indices) = build_player_model();
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));

        let (hand_vertices, hand_indices) = build_first_person_hand_model();
        queue.write_buffer(
            &self.first_person_hand.vertex_buffer,
            0,
            bytemuck::cast_slice(&hand_vertices),
        );
        queue.write_buffer(
            &self.first_person_hand.index_buffer,
            0,
            bytemuck::cast_slice(&hand_indices),
        );

        let hand_instance = PlayerInstance {
            model_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            animation_phase: 0.0,
            head_pitch: 0.0,
            attack_animation: 0.0,
            _padding: 0.0,
        };
        queue.write_buffer(
            &self.first_person_hand.instance_buffer,
            0,
            bytemuck::bytes_of(&hand_instance),
        );
    }

    pub fn update_players(&mut self, queue: &wgpu::Queue, players: &[RemotePlayer]) {
        self.players = players
            .iter()
            .take(MAX_PLAYER_INSTANCES)
            .cloned()
            .collect();

        if self.players.is_empty() {
            return;
        }

        let instances: Vec<PlayerInstance> = self
            .players
            .iter()
            .map(|player| {
                let crouch_offset = if player.is_crouching { -0.3 } else { 0.0 };
                let translation =
                    Mat4::from_translation(player.position + Vec3::new(0.0, crouch_offset, 0.0));
                let rotation = Mat4::from_rotation_y(player.yaw);
                let model_matrix = translation * rotation;

                PlayerInstance {
                    model_matrix: model_matrix.to_cols_array_2d(),
                    animation_phase: player.animation_phase,
                    head_pitch: player.pitch.clamp(-1.3, 1.3),
                    attack_animation: player.attack_animation.clamp(0.0, 1.0),
                    _padding: 0.0,
                }
            })
            .collect();

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
    }

    pub fn update_mobs(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mobs: &[MobRenderInfo],
    ) {
        if mobs.is_empty() {
            self.mob_index_count = 0;
            return;
        }

        let mut vertices = Vec::with_capacity(mobs.len() * 24);
        let mut indices = Vec::with_capacity(mobs.len() * 36);

        for mob in mobs {
            if vertices.len() > (u16::MAX as usize).saturating_sub(24) {
                break;
            }

            let color = if mob.hurt_flash {
                [1.0, mob.color[1] * 0.3, mob.color[2] * 0.3]
            } else {
                mob.color
            };
            let half_width = mob.width * 0.5;
            let (mut mob_vertices, mob_indices) = build_box_part(
                [-half_width, 0.0, -half_width],
                [half_width, mob.height, half_width],
                color,
            );

            let model = Mat4::from_translation(mob.position) * Mat4::from_rotation_y(mob.yaw);
            for vertex in &mut mob_vertices {
                let transformed = model.transform_point3(Vec3::from_array(vertex.position));
                vertex.position = transformed.to_array();
            }

            let base_index = vertices.len() as u16;
            vertices.extend(mob_vertices);
            indices.extend(mob_indices.into_iter().map(|idx| idx + base_index));
        }

        self.mob_index_count = indices.len() as u32;
        if self.mob_index_count == 0 {
            return;
        }

        self.ensure_mob_buffer_capacity(device, vertices.len(), indices.len());
        queue.write_buffer(&self.mob_vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&self.mob_index_buffer, 0, bytemuck::cast_slice(&indices));
    }

    fn ensure_mob_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        required_vertices: usize,
        required_indices: usize,
    ) {
        if required_vertices > self.mob_vertex_capacity {
            self.mob_vertex_capacity = required_vertices.next_power_of_two();
            self.mob_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mob Vertex Buffer"),
                size: (self.mob_vertex_capacity * mem::size_of::<PlayerVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if required_indices > self.mob_index_capacity {
            self.mob_index_capacity = required_indices.next_power_of_two();
            self.mob_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mob Index Buffer"),
                size: (self.mob_index_capacity * mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    pub fn update_first_person_hand(
        &self,
        queue: &wgpu::Queue,
        camera_pos: Vec3,
        camera_forward: Vec3,
        attack_animation: f32,
    ) {
        let mut forward = camera_forward.normalize_or_zero();
        if forward.length_squared() <= 1e-6 {
            forward = Vec3::X;
        }

        let mut right = forward.cross(Vec3::Y);
        if right.length_squared() <= 1e-6 {
            right = Vec3::Z;
        } else {
            right = right.normalize();
        }
        let up = right.cross(forward).normalize_or_zero();

        let hand_position = camera_pos + forward * 0.5 + right * 0.3 - up * 0.4;
        let view_space_orientation = Mat4::from_mat3(Mat3::from_cols(right, up, forward));

        let swing = (attack_animation.clamp(0.0, 1.0) * std::f32::consts::PI).sin();
        let base_pose = Mat4::from_rotation_z(-0.30) * Mat4::from_rotation_x(0.25);
        let swing_pose = Mat4::from_rotation_x(-swing * 1.1) * Mat4::from_rotation_y(-swing * 0.35);

        let model_matrix = Mat4::from_translation(hand_position)
            * view_space_orientation
            * swing_pose
            * base_pose;

        let hand_instance = PlayerInstance {
            model_matrix: model_matrix.to_cols_array_2d(),
            animation_phase: 0.0,
            head_pitch: 0.0,
            attack_animation: 0.0,
            _padding: 0.0,
        };
        queue.write_buffer(
            &self.first_person_hand.instance_buffer,
            0,
            bytemuck::bytes_of(&hand_instance),
        );

        if self.hand_block_vertex_buffer.is_some() {
            let (held_block_position, held_block_pose, held_item_scale) = if self.hand_block_is_flat {
                (
                    camera_pos + forward * 0.58 + right * 0.21 - up * 0.31,
                    Mat4::from_rotation_z(-0.44)
                        * Mat4::from_rotation_y(-0.24)
                        * Mat4::from_rotation_x(0.30),
                    1.16,
                )
            } else {
                (
                    camera_pos + forward * 0.62 + right * 0.23 - up * 0.28,
                    Mat4::from_rotation_z(-0.10)
                        * Mat4::from_rotation_y(0.76)
                        * Mat4::from_rotation_x(0.48),
                    1.34,
                )
            };
            let held_block_model =
                Mat4::from_translation(held_block_position)
                    * view_space_orientation
                    * swing_pose
                    * held_block_pose
                    * Mat4::from_scale(Vec3::splat(held_item_scale));

            let held_block_instance = HeldBlockInstance {
                model_matrix: held_block_model.to_cols_array_2d(),
                color: [1.0, 1.0, 1.0, 1.0],
                tile_origin: self.hand_block_tile_origin.unwrap_or([0.0, 0.0]),
                _padding: [0.0; 2],
            };
            queue.write_buffer(
                &self.hand_block_instance_buffer,
                0,
                bytemuck::bytes_of(&held_block_instance),
            );
        }
    }

    pub fn update_hand_item(
        &mut self,
        device: &wgpu::Device,
        selected_item: ItemId,
        atlas_mapping: &AtlasMapping,
    ) {
        let Some(atlas_offset) = atlas_mapping.offset_for_item(selected_item) else {
            self.hand_block_vertex_buffer = None;
            self.hand_block_index_buffer = None;
            self.hand_block_index_count = 0;
            self.hand_block_tile_origin = None;
            self.hand_block_selected_item = None;
            self.hand_block_is_flat = false;
            return;
        };

        let is_block_item = selected_item
            .as_block_id()
            .is_some_and(|block| block != BlockId::AIR);

        if self.hand_block_selected_item == Some(selected_item)
            && self.hand_block_vertex_buffer.is_some()
            && self.hand_block_index_buffer.is_some()
        {
            return;
        }

        let (vertices, indices) = if is_block_item {
            build_held_block_model(atlas_offset)
        } else {
            build_held_item_sprite_model(atlas_offset)
        };
        self.hand_block_vertex_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("First Person Held Item Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        self.hand_block_index_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("First Person Held Item Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));
        self.hand_block_index_count = indices.len() as u32;
        self.hand_block_tile_origin = Some(atlas_offset);
        self.hand_block_selected_item = Some(selected_item);
        self.hand_block_is_flat = !is_block_item;
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if self.players.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..self.players.len() as u32);
    }

    pub fn render_mobs<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if self.mob_index_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.mob_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.mob_instance_buffer.slice(..));
        render_pass.set_index_buffer(self.mob_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.mob_index_count, 0, 0..1);
    }

    pub fn render_first_person_hand<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.first_person_hand.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.first_person_hand.instance_buffer.slice(..));
        render_pass.set_index_buffer(
            self.first_person_hand.index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..self.first_person_hand.num_indices, 0, 0..1);
    }

    pub fn render_first_person_hand_block<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        atlas_bind_group: &'a wgpu::BindGroup,
        item_drop_pipeline: &'a wgpu::RenderPipeline,
    ) {
        let (Some(vertex_buffer), Some(index_buffer)) = (
            self.hand_block_vertex_buffer.as_ref(),
            self.hand_block_index_buffer.as_ref(),
        ) else {
            return;
        };

        render_pass.set_pipeline(item_drop_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, atlas_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.hand_block_instance_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.hand_block_index_count, 0, 0..1);
    }
}

fn build_player_model() -> (Vec<PlayerVertex>, Vec<u16>) {
    let skin = [0.85, 0.65, 0.45];
    let shirt = [0.2, 0.5, 0.8];
    let pants = [0.3, 0.3, 0.5];

    let parts = [
        build_box_part([-0.25, 1.5, -0.25], [0.25, 2.0, 0.25], skin),
        build_box_part([-0.25, 0.75, -0.125], [0.25, 1.5, 0.125], shirt),
        build_box_part([0.251, 0.75, -0.125], [0.501, 1.5, 0.125], shirt),
        build_box_part([-0.501, 0.75, -0.125], [-0.251, 1.5, 0.125], shirt),
        build_box_part([0.0, 0.0, -0.125], [0.25, 0.75, 0.125], pants),
        build_box_part([-0.25, 0.0, -0.125], [0.0, 0.75, 0.125], pants),
    ];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (part_vertices, part_indices) in parts {
        let base_index = vertices.len() as u16;
        vertices.extend(part_vertices);
        indices.extend(part_indices.into_iter().map(|idx| idx + base_index));
    }

    (vertices, indices)
}

fn build_first_person_hand_model() -> (Vec<PlayerVertex>, Vec<u16>) {
    build_box_part([-0.125, -0.75, -0.125], [0.125, 0.0, 0.125], [0.85, 0.65, 0.45])
}

fn build_held_block_model(atlas_offset: [f32; 2]) -> (Vec<ItemDropVertex>, Vec<u16>) {
    let _ = atlas_offset;

    let x0 = -0.2;
    let y0 = -0.2;
    let z0 = -0.2;
    let x1 = 0.2;
    let y1 = 0.2;
    let z1 = 0.2;

    let front = 1.0;
    let back = 0.86;
    let side = 0.78;
    let top = 1.12;
    let bottom = 0.68;

    let vertices = vec![
        ItemDropVertex { position: [x0, y0, z1], shade: front, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x1, y0, z1], shade: front, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z1], shade: front, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x0, y1, z1], shade: front, uv: [0.0, 0.0] },
        ItemDropVertex { position: [x1, y0, z0], shade: back, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x0, y0, z0], shade: back, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x0, y1, z0], shade: back, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x1, y1, z0], shade: back, uv: [0.0, 0.0] },
        ItemDropVertex { position: [x1, y0, z1], shade: side, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x1, y0, z0], shade: side, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z0], shade: side, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x1, y1, z1], shade: side, uv: [0.0, 0.0] },
        ItemDropVertex { position: [x0, y0, z0], shade: side, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x0, y0, z1], shade: side, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x0, y1, z1], shade: side, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x0, y1, z0], shade: side, uv: [0.0, 0.0] },
        ItemDropVertex { position: [x0, y1, z1], shade: top, uv: [0.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z1], shade: top, uv: [1.0, 1.0] },
        ItemDropVertex { position: [x1, y1, z0], shade: top, uv: [1.0, 0.0] },
        ItemDropVertex { position: [x0, y1, z0], shade: top, uv: [0.0, 0.0] },
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

fn build_held_item_sprite_model(atlas_offset: [f32; 2]) -> (Vec<ItemDropVertex>, Vec<u16>) {
    let _ = atlas_offset;

    let x0 = -0.2;
    let y0 = -0.2;
    let z0 = -0.03;
    let x1 = 0.2;
    let y1 = 0.2;
    let z1 = 0.03;

    let front = 1.0;
    let back = 0.9;
    let side = 0.77;
    let top = 1.08;
    let bottom = 0.7;

    let vertices = vec![
        // Front (+Z)
        ItemDropVertex {
            position: [x0, y0, z1],
            shade: front,
            uv: [0.0, 1.0],
        },
        ItemDropVertex {
            position: [x1, y0, z1],
            shade: front,
            uv: [1.0, 1.0],
        },
        ItemDropVertex {
            position: [x1, y1, z1],
            shade: front,
            uv: [1.0, 0.0],
        },
        ItemDropVertex {
            position: [x0, y1, z1],
            shade: front,
            uv: [0.0, 0.0],
        },
        // Back (-Z)
        ItemDropVertex {
            position: [x1, y0, z0],
            shade: back,
            uv: [0.0, 1.0],
        },
        ItemDropVertex {
            position: [x0, y0, z0],
            shade: back,
            uv: [1.0, 1.0],
        },
        ItemDropVertex {
            position: [x0, y1, z0],
            shade: back,
            uv: [1.0, 0.0],
        },
        ItemDropVertex {
            position: [x1, y1, z0],
            shade: back,
            uv: [0.0, 0.0],
        },
        // Right (+X) sampled from right edge of texture
        ItemDropVertex {
            position: [x1, y0, z1],
            shade: side,
            uv: [1.0, 1.0],
        },
        ItemDropVertex {
            position: [x1, y0, z0],
            shade: side,
            uv: [1.0, 1.0],
        },
        ItemDropVertex {
            position: [x1, y1, z0],
            shade: side,
            uv: [1.0, 0.0],
        },
        ItemDropVertex {
            position: [x1, y1, z1],
            shade: side,
            uv: [1.0, 0.0],
        },
        // Left (-X) sampled from left edge of texture
        ItemDropVertex {
            position: [x0, y0, z0],
            shade: side,
            uv: [0.0, 1.0],
        },
        ItemDropVertex {
            position: [x0, y0, z1],
            shade: side,
            uv: [0.0, 1.0],
        },
        ItemDropVertex {
            position: [x0, y1, z1],
            shade: side,
            uv: [0.0, 0.0],
        },
        ItemDropVertex {
            position: [x0, y1, z0],
            shade: side,
            uv: [0.0, 0.0],
        },
        // Top (+Y) sampled from top edge
        ItemDropVertex {
            position: [x0, y1, z1],
            shade: top,
            uv: [0.0, 0.0],
        },
        ItemDropVertex {
            position: [x1, y1, z1],
            shade: top,
            uv: [1.0, 0.0],
        },
        ItemDropVertex {
            position: [x1, y1, z0],
            shade: top,
            uv: [1.0, 0.0],
        },
        ItemDropVertex {
            position: [x0, y1, z0],
            shade: top,
            uv: [0.0, 0.0],
        },
        // Bottom (-Y) sampled from bottom edge
        ItemDropVertex {
            position: [x0, y0, z0],
            shade: bottom,
            uv: [0.0, 1.0],
        },
        ItemDropVertex {
            position: [x1, y0, z0],
            shade: bottom,
            uv: [1.0, 1.0],
        },
        ItemDropVertex {
            position: [x1, y0, z1],
            shade: bottom,
            uv: [1.0, 1.0],
        },
        ItemDropVertex {
            position: [x0, y0, z1],
            shade: bottom,
            uv: [0.0, 1.0],
        },
    ];

    let mut indices = Vec::with_capacity(36);
    for face in 0..6u16 {
        let base = face * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

fn build_box_part(min: [f32; 3], max: [f32; 3], color: [f32; 3]) -> (Vec<PlayerVertex>, Vec<u16>) {
    let x0 = min[0];
    let y0 = min[1];
    let z0 = min[2];
    let x1 = max[0];
    let y1 = max[1];
    let z1 = max[2];

    let vertices = vec![
        PlayerVertex {
            position: [x0, y0, z1],
            color,
        },
        PlayerVertex {
            position: [x1, y0, z1],
            color,
        },
        PlayerVertex {
            position: [x1, y1, z1],
            color,
        },
        PlayerVertex {
            position: [x0, y1, z1],
            color,
        },
        PlayerVertex {
            position: [x0, y0, z0],
            color,
        },
        PlayerVertex {
            position: [x1, y0, z0],
            color,
        },
        PlayerVertex {
            position: [x1, y1, z0],
            color,
        },
        PlayerVertex {
            position: [x0, y1, z0],
            color,
        },
        PlayerVertex {
            position: [x1, y0, z1],
            color,
        },
        PlayerVertex {
            position: [x1, y0, z0],
            color,
        },
        PlayerVertex {
            position: [x1, y1, z0],
            color,
        },
        PlayerVertex {
            position: [x1, y1, z1],
            color,
        },
        PlayerVertex {
            position: [x0, y0, z1],
            color,
        },
        PlayerVertex {
            position: [x0, y0, z0],
            color,
        },
        PlayerVertex {
            position: [x0, y1, z0],
            color,
        },
        PlayerVertex {
            position: [x0, y1, z1],
            color,
        },
        PlayerVertex {
            position: [x0, y1, z1],
            color,
        },
        PlayerVertex {
            position: [x1, y1, z1],
            color,
        },
        PlayerVertex {
            position: [x1, y1, z0],
            color,
        },
        PlayerVertex {
            position: [x0, y1, z0],
            color,
        },
        PlayerVertex {
            position: [x0, y0, z1],
            color,
        },
        PlayerVertex {
            position: [x1, y0, z1],
            color,
        },
        PlayerVertex {
            position: [x1, y0, z0],
            color,
        },
        PlayerVertex {
            position: [x0, y0, z0],
            color,
        },
    ];

    let indices = vec![
        0, 1, 2, 2, 3, 0, 5, 4, 7, 7, 6, 5, 8, 9, 10, 10, 11, 8, 13, 12, 15, 15, 14, 13, 16,
        17, 18, 18, 19, 16, 21, 20, 23, 23, 22, 21,
    ];

    (vertices, indices)
}
