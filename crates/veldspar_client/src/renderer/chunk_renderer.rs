use std::cmp::Ordering;
use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use rustc_hash::FxHashMap;
use veldspar_shared::coords::ChunkPos;

use crate::renderer::mesh::{ChunkMesh, ChunkVertex};

pub type FrustumPlanes = [[f32; 4]; 6];
const CHUNK_WORLD_SIZE: f32 = 32.0;
const CHUNK_HALF_EXTENT: f32 = CHUNK_WORLD_SIZE * 0.5;

#[derive(Debug, Clone, Copy, Default)]
pub struct MeshUploadStats {
    pub uploaded_bytes: u64,
    pub buffer_reallocations: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChunkPassStats {
    pub draw_calls: u32,
    pub rendered_chunks: u32,
    pub rendered_indices: u64,
    pub rendered_vertices: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ChunkParamsUniform {
    spawn_time: f32,
    _padding: [f32; 3],
}

pub struct ChunkRenderData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_format: wgpu::IndexFormat,
    pub index_count: u32,
    pub vertex_count: u32,
    pub chunk_pos: ChunkPos,
    pub spawn_time: f32,
    pub vertex_capacity_bytes: u64,
    pub index_capacity_bytes: u64,
    pub chunk_params_buffer: wgpu::Buffer,
    pub chunk_params_bind_group: wgpu::BindGroup,
}

pub fn upload_mesh(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &ChunkMesh,
    chunk_pos: ChunkPos,
    chunk_params_layout: &wgpu::BindGroupLayout,
    spawn_time_seconds: f32,
) -> (ChunkRenderData, MeshUploadStats) {
    let vertex_bytes = mesh_vertex_bytes(mesh);
    let index_bytes = mesh_index_bytes(mesh);
    let vertex_capacity_bytes = grow_capacity(vertex_bytes);
    let index_capacity_bytes = grow_capacity(index_bytes);

    let vertex_buffer = create_vertex_buffer(device, vertex_capacity_bytes);
    let index_buffer = create_index_buffer(device, index_capacity_bytes);

    if vertex_bytes > 0 {
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&mesh.vertices));
    }
    if index_bytes > 0 {
        queue.write_buffer(&index_buffer, 0, mesh.indices.as_bytes());
    }

    let (chunk_params_buffer, chunk_params_bind_group) = create_chunk_params_binding(
        device,
        queue,
        chunk_params_layout,
        spawn_time_seconds,
    );

    (
        ChunkRenderData {
            vertex_buffer,
            index_buffer,
            index_format: mesh.indices.index_format(),
            index_count: mesh.indices.index_count() as u32,
            vertex_count: mesh.vertices.len() as u32,
            chunk_pos,
            spawn_time: spawn_time_seconds,
            vertex_capacity_bytes,
            index_capacity_bytes,
            chunk_params_buffer,
            chunk_params_bind_group,
        },
        MeshUploadStats {
            uploaded_bytes: vertex_bytes + index_bytes,
            buffer_reallocations: 2,
        },
    )
}

pub fn update_mesh_buffers(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    chunk: &mut ChunkRenderData,
    mesh: &ChunkMesh,
) -> MeshUploadStats {
    let vertex_bytes = mesh_vertex_bytes(mesh);
    let index_bytes = mesh_index_bytes(mesh);

    let mut stats = MeshUploadStats {
        uploaded_bytes: vertex_bytes + index_bytes,
        ..Default::default()
    };

    if vertex_bytes > chunk.vertex_capacity_bytes {
        chunk.vertex_capacity_bytes = grow_capacity(vertex_bytes);
        chunk.vertex_buffer = create_vertex_buffer(device, chunk.vertex_capacity_bytes);
        stats.buffer_reallocations += 1;
    }

    if index_bytes > chunk.index_capacity_bytes {
        chunk.index_capacity_bytes = grow_capacity(index_bytes);
        chunk.index_buffer = create_index_buffer(device, chunk.index_capacity_bytes);
        stats.buffer_reallocations += 1;
    }

    if vertex_bytes > 0 {
        queue.write_buffer(&chunk.vertex_buffer, 0, bytemuck::cast_slice(&mesh.vertices));
    }
    if index_bytes > 0 {
        queue.write_buffer(&chunk.index_buffer, 0, mesh.indices.as_bytes());
    }

    chunk.index_format = mesh.indices.index_format();
    chunk.vertex_count = mesh.vertices.len() as u32;
    chunk.index_count = mesh.indices.index_count() as u32;
    stats
}

pub fn write_chunk_spawn_time(
    queue: &wgpu::Queue,
    chunk: &mut ChunkRenderData,
    spawn_time_seconds: f32,
) {
    chunk.spawn_time = spawn_time_seconds;
    let params = chunk_params_uniform(spawn_time_seconds);
    queue.write_buffer(&chunk.chunk_params_buffer, 0, bytemuck::bytes_of(&params));
}

pub fn render_chunks(
    pass: &mut wgpu::RenderPass<'_>,
    chunks: &FxHashMap<ChunkPos, ChunkRenderData>,
    frustum_planes: &FrustumPlanes,
    frustum_culling_enabled: bool,
) -> ChunkPassStats {
    let mut stats = ChunkPassStats::default();
    for chunk in chunks.values() {
        if chunk.index_count == 0 {
            continue;
        }
        if frustum_culling_enabled {
            let chunk_world = chunk_center(chunk.chunk_pos);
            if !aabb_in_frustum(frustum_planes, chunk_world, CHUNK_HALF_EXTENT) {
                continue;
            }
        }
        draw_chunk(pass, chunk, &mut stats);
    }
    stats
}

pub fn render_chunks_with_camera<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    chunks: &'a FxHashMap<ChunkPos, ChunkRenderData>,
    frustum_planes: &FrustumPlanes,
    frustum_culling_enabled: bool,
    camera_bind_group: &'a wgpu::BindGroup,
) -> ChunkPassStats {
    pass.set_bind_group(0, camera_bind_group, &[]);
    render_chunks(pass, chunks, frustum_planes, frustum_culling_enabled)
}

pub fn collect_visible_transparent_chunks(
    chunks: &FxHashMap<ChunkPos, ChunkRenderData>,
    frustum_planes: &FrustumPlanes,
    camera_pos: Vec3,
    visible: &mut Vec<(ChunkPos, f32)>,
    frustum_culling_enabled: bool,
) {
    visible.clear();
    visible.reserve(chunks.len().saturating_sub(visible.capacity()));

    for chunk in chunks.values() {
        if chunk.index_count == 0 {
            continue;
        }

        let chunk_world = chunk_center(chunk.chunk_pos);
        if frustum_culling_enabled {
            if !aabb_in_frustum(frustum_planes, chunk_world, CHUNK_HALF_EXTENT) {
                continue;
            }
        }

        visible.push((chunk.chunk_pos, chunk_world.distance_squared(camera_pos)));
    }

    visible.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
}

pub fn render_visible_transparent_chunks(
    pass: &mut wgpu::RenderPass<'_>,
    chunks: &FxHashMap<ChunkPos, ChunkRenderData>,
    visible: &[(ChunkPos, f32)],
) -> ChunkPassStats {
    let mut stats = ChunkPassStats::default();
    for &(chunk_pos, _) in visible {
        let Some(chunk) = chunks.get(&chunk_pos) else {
            continue;
        };
        if chunk.index_count == 0 {
            continue;
        }
        draw_chunk(pass, chunk, &mut stats);
    }
    stats
}

pub fn extract_frustum_planes(vp: Mat4) -> FrustumPlanes {
    let m = vp.to_cols_array_2d();
    let row0 = [m[0][0], m[1][0], m[2][0], m[3][0]];
    let row1 = [m[0][1], m[1][1], m[2][1], m[3][1]];
    let row2 = [m[0][2], m[1][2], m[2][2], m[3][2]];
    let row3 = [m[0][3], m[1][3], m[2][3], m[3][3]];

    let planes = [
        [row3[0] + row0[0], row3[1] + row0[1], row3[2] + row0[2], row3[3] + row0[3]],
        [row3[0] - row0[0], row3[1] - row0[1], row3[2] - row0[2], row3[3] - row0[3]],
        [row3[0] + row1[0], row3[1] + row1[1], row3[2] + row1[2], row3[3] + row1[3]],
        [row3[0] - row1[0], row3[1] - row1[1], row3[2] - row1[2], row3[3] - row1[3]],
        [row3[0] + row2[0], row3[1] + row2[1], row3[2] + row2[2], row3[3] + row2[3]],
        [row3[0] - row2[0], row3[1] - row2[1], row3[2] - row2[2], row3[3] - row2[3]],
    ];

    let mut result = [[0.0f32; 4]; 6];
    for (i, p) in planes.iter().enumerate() {
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        if len > 0.0001 {
            result[i] = [p[0] / len, p[1] / len, p[2] / len, p[3] / len];
        }
    }
    result
}

fn mesh_vertex_bytes(mesh: &ChunkMesh) -> u64 {
    (mesh.vertices.len() * mem::size_of::<ChunkVertex>()) as u64
}

fn mesh_index_bytes(mesh: &ChunkMesh) -> u64 {
    mesh.indices.index_bytes()
}

fn grow_capacity(required: u64) -> u64 {
    required.max(4).next_power_of_two()
}

fn create_vertex_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Chunk Vertex Buffer"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_index_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Chunk Index Buffer"),
        size,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn chunk_params_uniform(spawn_time_seconds: f32) -> ChunkParamsUniform {
    ChunkParamsUniform {
        spawn_time: spawn_time_seconds,
        _padding: [0.0; 3],
    }
}

fn create_chunk_params_binding(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    chunk_params_layout: &wgpu::BindGroupLayout,
    spawn_time_seconds: f32,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let params = chunk_params_uniform(spawn_time_seconds);
    let chunk_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Chunk Params Buffer"),
        size: mem::size_of::<ChunkParamsUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&chunk_params_buffer, 0, bytemuck::bytes_of(&params));

    let chunk_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Chunk Params Bind Group"),
        layout: chunk_params_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: chunk_params_buffer.as_entire_binding(),
        }],
    });
    (chunk_params_buffer, chunk_params_bind_group)
}

fn draw_chunk(
    pass: &mut wgpu::RenderPass<'_>,
    chunk: &ChunkRenderData,
    stats: &mut ChunkPassStats,
) {
    pass.set_bind_group(2, &chunk.chunk_params_bind_group, &[]);
    pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
    pass.set_index_buffer(chunk.index_buffer.slice(..), chunk.index_format);
    pass.draw_indexed(0..chunk.index_count, 0, 0..1);

    stats.draw_calls += 1;
    stats.rendered_chunks += 1;
    stats.rendered_indices += u64::from(chunk.index_count);
    stats.rendered_vertices += u64::from(chunk.vertex_count);
}

fn chunk_center(chunk_pos: ChunkPos) -> Vec3 {
    Vec3::new(
        chunk_pos.x as f32 * CHUNK_WORLD_SIZE + CHUNK_HALF_EXTENT,
        chunk_pos.y as f32 * CHUNK_WORLD_SIZE + CHUNK_HALF_EXTENT,
        chunk_pos.z as f32 * CHUNK_WORLD_SIZE + CHUNK_HALF_EXTENT,
    )
}

fn aabb_in_frustum(planes: &FrustumPlanes, center: Vec3, half: f32) -> bool {
    for plane in planes {
        let d = plane[0] * center.x + plane[1] * center.y + plane[2] * center.z + plane[3];
        let r = half * (plane[0].abs() + plane[1].abs() + plane[2].abs());
        if d < -r {
            return false;
        }
    }
    true
}
