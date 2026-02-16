pub mod atlas;
pub mod chunk_renderer;
pub mod clouds;
pub mod item_drop_renderer;
pub mod mesh;

use std::collections::HashMap;
pub mod particles;
pub mod pipeline;
pub mod player_renderer;
pub mod sky;
pub mod ui;
pub mod water_pipeline;

use std::cell::Cell;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use veldspar_shared::block::BlockRegistry;
use veldspar_shared::coords::ChunkPos;
use veldspar_shared::inventory::{Inventory, ItemId, ItemStack};
use winit::window::Window;

use crate::camera::Camera;
use crate::renderer::atlas::{build_atlas, AtlasBuildError, AtlasMapping};
use crate::renderer::chunk_renderer::{
    render_chunks,
    render_chunks_transparent,
    upload_mesh,
    ChunkRenderData,
};
use crate::renderer::clouds::CloudRenderer;
use crate::renderer::item_drop_renderer::{ItemDropRenderData, ItemDropRenderer};
use crate::renderer::mesh::{ChunkMesh, ChunkMeshes};
use crate::renderer::particles::ParticleRenderer;
use crate::renderer::pipeline::ChunkPipeline;
use crate::renderer::player_renderer::{MobRenderInfo, PlayerRenderer, RemotePlayer};
use crate::renderer::sky::{sky_horizon_color, SkyRenderer};
use crate::renderer::water_pipeline::WaterPipeline;
use crate::ui::block_highlight::BlockHighlightRenderer;
use crate::ui::break_indicator::BreakIndicatorRenderer;
use crate::ui::crosshair::CrosshairRenderer;
use crate::ui::health_hud::HealthHudRenderer;
use crate::ui::inventory::{CraftingUiMode, InventoryRenderer};
use crate::ui::main_menu::{MainMenuRenderer, SettingsMenuView, WorldSelectView};
use crate::ui::text_overlay::TextOverlayRenderer;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const FADE_DURATION: f32 = 0.4;
const WEATHER_DARKEN_STRENGTH: f32 = 0.3;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    fog_color: [f32; 4],
    fog_start: f32,
    fog_end: f32,
    time_of_day: f32,
    underwater: f32,
}

impl CameraUniform {
    fn from_camera(
        camera: &Camera,
        fog_start: f32,
        fog_end: f32,
        time_of_day: f32,
        underwater: bool,
        weather_dim: f32,
    ) -> Self {
        let mut fog_color = sky_horizon_color(time_of_day);
        let weather_factor = 1.0 - WEATHER_DARKEN_STRENGTH * weather_dim.clamp(0.0, 1.0);
        fog_color[0] *= weather_factor;
        fog_color[1] *= weather_factor;
        fog_color[2] *= weather_factor;

        Self {
            view_proj: camera.view_projection_matrix().to_cols_array_2d(),
            camera_pos: [camera.position.x, camera.position.y, camera.position.z, 0.0],
            fog_color,
            fog_start,
            fog_end,
            time_of_day,
            underwater: if underwater { 1.0 } else { 0.0 },
        }
    }
}

#[derive(Debug)]
struct DepthTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl DepthTexture {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Veldspar Depth Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
        }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    depth_texture: DepthTexture,
    chunk_pipeline: ChunkPipeline,
    water_pipeline: WaterPipeline,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    atlas_bind_group: wgpu::BindGroup,
    _atlas_texture: wgpu::Texture,
    atlas_mapping: AtlasMapping,
    chunk_meshes: HashMap<ChunkPos, ChunkRenderData>,
    water_meshes: HashMap<ChunkPos, ChunkRenderData>,
    sky_renderer: SkyRenderer,
    cloud_renderer: CloudRenderer,
    particle_renderer: ParticleRenderer,
    item_drop_renderer: ItemDropRenderer,
    crosshair_renderer: CrosshairRenderer,
    block_highlight: BlockHighlightRenderer,
    break_indicator: BreakIndicatorRenderer,
    inventory_renderer: InventoryRenderer,
    health_hud_renderer: HealthHudRenderer,
    text_overlay_renderer: TextOverlayRenderer,
    player_renderer: PlayerRenderer,
    main_menu_renderer: MainMenuRenderer,
    view_proj: Cell<glam::Mat4>,
    camera_pos: Cell<glam::Vec3>,
    crosshair_visible: bool,
}

#[derive(Debug)]
pub enum RendererInitError {
    CreateSurface(wgpu::CreateSurfaceError),
    RequestAdapter(wgpu::RequestAdapterError),
    RequestDevice(wgpu::RequestDeviceError),
    UnsupportedSurface,
    BuildAtlas(AtlasBuildError),
}

impl fmt::Display for RendererInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateSurface(err) => write!(f, "failed to create surface: {err}"),
            Self::RequestAdapter(err) => write!(f, "failed to request adapter: {err}"),
            Self::RequestDevice(err) => write!(f, "failed to request device: {err}"),
            Self::UnsupportedSurface => write!(f, "adapter does not support this surface"),
            Self::BuildAtlas(err) => write!(f, "failed to build texture atlas: {err}"),
        }
    }
}

impl std::error::Error for RendererInitError {}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Result<Self, RendererInitError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(RendererInitError::CreateSurface)?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(RendererInitError::RequestAdapter)?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Veldspar Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(RendererInitError::RequestDevice)?;

        let initial_size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, initial_size.width.max(1), initial_size.height.max(1))
            .ok_or(RendererInitError::UnsupportedSurface)?;

        surface.configure(&device, &surface_config);

        let chunk_pipeline = ChunkPipeline::new(&device, surface_config.format, DEPTH_FORMAT);
        let water_pipeline = WaterPipeline::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
            &chunk_pipeline.texture_bind_group_layout,
            &chunk_pipeline.chunk_params_bind_group_layout,
        );
        let atlas_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Excalibur_V1/assets/minecraft/textures/block");
        let (atlas_texture, atlas_view, atlas_sampler, atlas_mapping) =
            build_atlas(&device, &queue, &atlas_dir).map_err(RendererInitError::BuildAtlas)?;
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chunk Atlas Bind Group"),
            layout: &chunk_pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let initial_camera_uniform = CameraUniform {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0; 4],
            fog_color: sky_horizon_color(0.5),
            fog_start: 0.0,
            fog_end: 1.0,
            time_of_day: 0.5,
            underwater: 0.0,
        };
        let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&initial_camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chunk Camera Bind Group"),
            layout: &chunk_pipeline.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });
        let depth_texture = DepthTexture::new(&device, surface_config.width, surface_config.height);
        let sky_renderer = SkyRenderer::new(&device, surface_config.format);
        let cloud_renderer = CloudRenderer::new(&device, surface_config.format);
        let particle_renderer = ParticleRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let item_drop_renderer = ItemDropRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
            &chunk_pipeline.texture_bind_group_layout,
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Item Drop Atlas Bind Group"),
                layout: &chunk_pipeline.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&atlas_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                    },
                ],
            }),
        );
        let crosshair_renderer = CrosshairRenderer::new(
            &device,
            surface_config.format,
            surface_config.width,
            surface_config.height,
        );
        let block_highlight = BlockHighlightRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let break_indicator = BreakIndicatorRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let inventory_renderer = InventoryRenderer::new(
            &device,
            surface_config.format,
            &chunk_pipeline.texture_bind_group_layout,
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Inventory Atlas Bind Group"),
                layout: &chunk_pipeline.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&atlas_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                    },
                ],
            }),
            atlas_mapping.clone(),
        );
        let health_hud_renderer = HealthHudRenderer::new(&device, surface_config.format);
        let text_overlay_renderer = TextOverlayRenderer::new(&device, surface_config.format);
        let player_renderer = PlayerRenderer::new(
            &device,
            surface_config.format,
            DEPTH_FORMAT,
            &chunk_pipeline.camera_bind_group_layout,
        );
        let main_menu_renderer = MainMenuRenderer::new(
            &device,
            surface_config.format,
            surface_config.width,
            surface_config.height,
        );

        player_renderer.init_buffers(&queue);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            depth_texture,
            chunk_pipeline,
            water_pipeline,
            camera_uniform_buffer,
            camera_bind_group,
            atlas_bind_group,
            _atlas_texture: atlas_texture,
            atlas_mapping,
            chunk_meshes: HashMap::new(),
            water_meshes: HashMap::new(),
            sky_renderer,
            cloud_renderer,
            particle_renderer,
            item_drop_renderer,
            crosshair_renderer,
            block_highlight,
            break_indicator,
            inventory_renderer,
            health_hud_renderer,
            text_overlay_renderer,
            player_renderer,
            main_menu_renderer,
            view_proj: Cell::new(glam::Mat4::IDENTITY),
            camera_pos: Cell::new(glam::Vec3::ZERO),
            crosshair_visible: true,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.depth_texture = DepthTexture::new(&self.device, width, height);
        self.crosshair_renderer.resize(&self.queue, width, height);
    }

    pub fn atlas_mapping(&self) -> &AtlasMapping {
        &self.atlas_mapping
    }

    pub fn update_camera_uniform(
        &self,
        camera: &Camera,
        fog_start: f32,
        fog_end: f32,
        time_of_day: f32,
        underwater: bool,
        weather_dim: f32,
    ) {
        let uniform = CameraUniform::from_camera(
            camera,
            fog_start,
            fog_end,
            time_of_day,
            underwater,
            weather_dim,
        );
        self.queue
            .write_buffer(&self.camera_uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        // Store view_proj and camera_pos for frustum culling
        self.view_proj.set(camera.view_projection_matrix());
        self.camera_pos.set(camera.position);
    }

    pub fn update_sky(&self, camera: &Camera, time_of_day: f32, weather_dim: f32) {
        self.sky_renderer
            .update(&self.queue, camera, time_of_day, weather_dim);
    }

    pub fn update_clouds(&self, camera: &Camera, time: f32) {
        self.cloud_renderer.update(&self.queue, camera, time);
    }

    pub fn update_particles(&mut self, dt: f32, camera: &Camera) {
        let forward = camera.forward_direction();
        let mut right = forward.cross(glam::Vec3::Y).normalize_or_zero();
        if right.length_squared() < 1e-6 {
            right = glam::Vec3::X;
        }
        let up = right.cross(forward).normalize_or_zero();
        self.particle_renderer.update(&self.queue, dt, right, up);
    }

    pub fn spawn_break_particles(&mut self, block_pos: glam::IVec3, color: [f32; 3]) {
        self.particle_renderer.spawn_break_particles(block_pos, color);
    }

    pub fn spawn_walk_particles(&mut self, position: glam::Vec3, color: [f32; 3]) {
        self.particle_renderer.spawn_walk_particles(position, color);
    }

    pub fn spawn_place_particles(&mut self, block_pos: glam::IVec3, color: [f32; 3]) {
        self.particle_renderer.spawn_place_particles(block_pos, color);
    }

    pub fn spawn_rain_particles(&mut self, spawn_positions: &[glam::Vec3]) {
        self.particle_renderer.spawn_rain_particles(spawn_positions);
    }

    pub fn spawn_snow_particles(&mut self, spawn_positions: &[glam::Vec3]) {
        self.particle_renderer.spawn_snow_particles(spawn_positions);
    }

    pub fn advance_fades(&mut self, dt: f32) {
        let queue = &self.queue;
        for chunk in self.chunk_meshes.values_mut() {
            if chunk.fade < 1.0 {
                chunk.fade = (chunk.fade + dt / FADE_DURATION).min(1.0);
                queue.write_buffer(&chunk.fade_buffer, 0, bytemuck::bytes_of(&chunk.fade));
            }
        }
        for chunk in self.water_meshes.values_mut() {
            if chunk.fade < 1.0 {
                chunk.fade = (chunk.fade + dt / FADE_DURATION).min(1.0);
                queue.write_buffer(&chunk.fade_buffer, 0, bytemuck::bytes_of(&chunk.fade));
            }
        }
    }

    pub fn upload_chunk_mesh(&mut self, mesh: &ChunkMesh, chunk_pos: ChunkPos) {
        if mesh.is_empty() {
            return;
        }

        let data = upload_mesh(
            &self.device,
            mesh,
            chunk_pos,
            &self.chunk_pipeline.chunk_params_bind_group_layout,
        );
        self.chunk_meshes.insert(chunk_pos, data);
    }

    pub fn clear_chunk_meshes(&mut self) {
        self.chunk_meshes.clear();
        self.water_meshes.clear();
        self.particle_renderer.clear();
        self.item_drop_renderer.clear();
    }

    pub fn replace_chunk_mesh(&mut self, meshes: &ChunkMeshes, chunk_pos: ChunkPos) {
        // Preserve fade for existing chunks (remeshing should not re-trigger fade-in)
        let existing_opaque_fade = self.chunk_meshes.get(&chunk_pos).map(|c| c.fade);
        let existing_water_fade = self.water_meshes.get(&chunk_pos).map(|c| c.fade);

        if !meshes.opaque.is_empty() {
            let mut data = upload_mesh(
                &self.device,
                &meshes.opaque,
                chunk_pos,
                &self.chunk_pipeline.chunk_params_bind_group_layout,
            );
            if let Some(fade) = existing_opaque_fade {
                data.fade = fade;
                self.queue
                    .write_buffer(&data.fade_buffer, 0, bytemuck::bytes_of(&data.fade));
            }
            self.chunk_meshes.insert(chunk_pos, data);
        } else {
            self.chunk_meshes.remove(&chunk_pos);
        }

        if !meshes.water.is_empty() {
            let mut data = upload_mesh(
                &self.device,
                &meshes.water,
                chunk_pos,
                &self.chunk_pipeline.chunk_params_bind_group_layout,
            );
            if let Some(fade) = existing_water_fade {
                data.fade = fade;
                self.queue
                    .write_buffer(&data.fade_buffer, 0, bytemuck::bytes_of(&data.fade));
            }
            self.water_meshes.insert(chunk_pos, data);
        } else {
            self.water_meshes.remove(&chunk_pos);
        }
    }

    pub fn remove_chunk_mesh(&mut self, chunk_pos: ChunkPos) {
        self.chunk_meshes.remove(&chunk_pos);
        self.water_meshes.remove(&chunk_pos);
    }

    pub fn update_highlight(&mut self, block_pos: Option<glam::IVec3>) {
        self.block_highlight.update(&self.queue, block_pos);
    }

    pub fn update_break_indicator(
        &mut self,
        local_block_pos: Option<glam::IVec3>,
        local_progress: f32,
        remote_overlays: &[(glam::IVec3, f32)],
    ) {
        self.break_indicator
            .update(&self.queue, local_block_pos, local_progress, remote_overlays);
    }

    pub fn update_inventory_ui(
        &mut self,
        gui_scale: f32,
        inventory: &Inventory,
        selected_hotbar_slot: usize,
        inventory_open: bool,
        crafting_mode: CraftingUiMode,
        crafting_inputs: &[Option<ItemStack>],
        crafting_output: Option<ItemStack>,
        chest_inventory: Option<&Inventory>,
        cursor_stack: Option<ItemStack>,
        cursor_position: Option<(f64, f64)>,
        registry: Option<&BlockRegistry>,
    ) {
        self.inventory_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            gui_scale,
            inventory,
            selected_hotbar_slot,
            inventory_open,
            crafting_mode,
            crafting_inputs,
            crafting_output,
            chest_inventory,
            cursor_stack,
            cursor_position,
            registry,
        );
    }

    pub fn update_health_hud(
        &mut self,
        gui_scale: f32,
        health: f32,
        hunger: f32,
        damage_flash_timer: f32,
        visible: bool,
        air_supply: f32,
        max_air_supply: f32,
        xp_progress: f32,
        xp_level: u32,
    ) {
        self.health_hud_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            gui_scale,
            health,
            hunger,
            damage_flash_timer,
            visible,
            air_supply,
            max_air_supply,
            xp_progress,
            xp_level,
        );
    }

    pub fn set_crosshair_visible(&mut self, visible: bool) {
        self.crosshair_visible = visible;
    }

    pub fn update_overlay_lines(
        &mut self,
        lines: &[String],
        chat_lines: &[(String, f32)],
        chat_input_line: Option<&str>,
    ) {
        self.text_overlay_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            lines,
            chat_lines,
            chat_input_line,
        );
    }

    pub fn update_players(&mut self, players: &[RemotePlayer]) {
        self.player_renderer.update_players(&self.queue, players);
    }

    pub fn update_mob_data(&mut self, mobs: &[MobRenderInfo]) {
        self.player_renderer
            .update_mobs(&self.device, &self.queue, mobs);
    }

    pub fn update_item_drops(&mut self, drops: &[ItemDropRenderData]) {
        self.item_drop_renderer.update(&self.queue, drops);
    }

    pub fn update_first_person_hand(
        &mut self,
        camera_pos: glam::Vec3,
        camera_forward: glam::Vec3,
        attack_animation: f32,
    ) {
        self.player_renderer.update_first_person_hand(
            &self.queue,
            camera_pos,
            camera_forward,
            attack_animation,
        );
    }

    pub fn update_first_person_hand_item(&mut self, selected_item: ItemId) {
        self.player_renderer
            .update_hand_item(&self.device, selected_item, &self.atlas_mapping);
    }

    pub fn update_main_menu(&mut self, selected_item: u8, show_ip_field: bool, server_ip: &str) {
        self.main_menu_renderer.update(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            selected_item,
            show_ip_field,
            server_ip,
        );
    }

    pub fn update_world_select_menu(&mut self, view: &WorldSelectView<'_>) {
        self.main_menu_renderer.update_world_select(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            view,
        );
    }

    pub fn update_pause_menu(&mut self, selected_item: u8) {
        self.main_menu_renderer.update_pause(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            selected_item,
        );
    }

    pub fn update_settings_menu(&mut self, selected_item: u8, view: &SettingsMenuView) {
        self.main_menu_renderer.update_settings(
            &self.queue,
            self.surface_config.width,
            self.surface_config.height,
            selected_item,
            view,
        );
    }

    pub fn render_main_menu_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Menu Command Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Menu Sky Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.sky_renderer.render(&mut render_pass);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Menu UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.main_menu_renderer.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    pub fn render_frame(&mut self, draw_menu_overlay: bool) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Veldspar Command Encoder"),
            });

        // Sky rendering pass (background)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Sky Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.529,
                            g: 0.808,
                            b: 0.922,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.sky_renderer.render(&mut render_pass);
        }

        // Cloud rendering pass (alpha-blended over sky, before world geometry)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Cloud Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.cloud_renderer.render(&mut render_pass);
        }

        // Main opaque rendering pass (chunks + block highlight)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Opaque Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(self.chunk_pipeline.pipeline());
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.atlas_bind_group, &[]);
            render_chunks(&mut render_pass, &self.chunk_meshes, self.view_proj.get(), self.camera_pos.get());

            // Render block highlight after chunks, within the same render pass
            self.block_highlight.render(&mut render_pass, &self.camera_bind_group);
            self.break_indicator
                .render(&mut render_pass, &self.camera_bind_group);
            self.item_drop_renderer
                .render(&mut render_pass, &self.camera_bind_group);
            self.player_renderer.render(&mut render_pass, &self.camera_bind_group);
            self.player_renderer
                .render_mobs(&mut render_pass, &self.camera_bind_group);
        }

        // Water rendering pass (transparent)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Water Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(self.water_pipeline.pipeline());
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.atlas_bind_group, &[]);
            render_chunks_transparent(
                &mut render_pass,
                &self.water_meshes,
                self.view_proj.get(),
                self.camera_pos.get(),
            );
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Particle Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.particle_renderer
                .render(&mut render_pass, &self.camera_bind_group);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar First Person Hand Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.player_renderer
                .render_first_person_hand(&mut render_pass, &self.camera_bind_group);
            self.player_renderer.render_first_person_hand_block(
                &mut render_pass,
                &self.camera_bind_group,
                &self.atlas_bind_group,
                self.item_drop_renderer.pipeline(),
            );
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if self.crosshair_visible {
                self.crosshair_renderer.render(&mut render_pass);
            }
            self.health_hud_renderer.render(&mut render_pass);
            self.inventory_renderer.render(&mut render_pass);
            self.text_overlay_renderer.render(&mut render_pass);
        }

        if draw_menu_overlay {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Veldspar Menu Overlay Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.main_menu_renderer.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
