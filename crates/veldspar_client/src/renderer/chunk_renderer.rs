use veldspar_shared::coords::ChunkPos;
use wgpu::util::DeviceExt;
use glam;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::renderer::mesh::ChunkMesh;

pub struct ChunkRenderData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub chunk_pos: ChunkPos,
    pub fade: f32,
    pub fade_buffer: wgpu::Buffer,
    pub fade_bind_group: wgpu::BindGroup,
}

pub fn upload_mesh(
    device: &wgpu::Device,
    mesh: &ChunkMesh,
    chunk_pos: ChunkPos,
    chunk_params_layout: &wgpu::BindGroupLayout,
) -> ChunkRenderData {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Vertex Buffer"),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Index Buffer"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let fade_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Fade Buffer"),
        contents: bytemuck::bytes_of(&0.0f32),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let fade_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Chunk Fade Bind Group"),
        layout: chunk_params_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: fade_buffer.as_entire_binding(),
        }],
    });

    ChunkRenderData {
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
        chunk_pos,
        fade: 0.0,
        fade_buffer,
        fade_bind_group,
    }
}

pub fn render_chunks(
    pass: &mut wgpu::RenderPass<'_>,
    chunks: &HashMap<ChunkPos, ChunkRenderData>,
    view_proj: glam::Mat4,
    _camera_pos: glam::Vec3,
) {
    let frustum_planes = extract_frustum_planes(view_proj);

    for chunk in chunks.values() {
        if chunk.index_count == 0 {
            continue;
        }

        let chunk_world = chunk_center(chunk.chunk_pos);
        let half_extent = 16.0;

        // Skip chunks completely outside frustum
        if !aabb_in_frustum(&frustum_planes, chunk_world, half_extent) {
            continue;
        }

        pass.set_bind_group(2, &chunk.fade_bind_group, &[]);
        pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
        pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..chunk.index_count, 0, 0..1);
    }
}

pub fn render_chunks_transparent(
    pass: &mut wgpu::RenderPass<'_>,
    chunks: &HashMap<ChunkPos, ChunkRenderData>,
    view_proj: glam::Mat4,
    camera_pos: glam::Vec3,
) {
    let frustum_planes = extract_frustum_planes(view_proj);
    if chunks.len() <= 1 {
        for chunk in chunks.values() {
            if chunk.index_count == 0 {
                continue;
            }

            let chunk_world = chunk_center(chunk.chunk_pos);
            let half_extent = 16.0;
            if !aabb_in_frustum(&frustum_planes, chunk_world, half_extent) {
                continue;
            }

            pass.set_bind_group(2, &chunk.fade_bind_group, &[]);
            pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
            pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..chunk.index_count, 0, 0..1);
        }
        return;
    }
    let mut visible: Vec<(&ChunkRenderData, f32)> = Vec::with_capacity(chunks.len());

    for chunk in chunks.values() {
        if chunk.index_count == 0 {
            continue;
        }

        let chunk_world = chunk_center(chunk.chunk_pos);
        let half_extent = 16.0;
        if !aabb_in_frustum(&frustum_planes, chunk_world, half_extent) {
            continue;
        }

        let distance_sq = chunk_world.distance_squared(camera_pos);
        visible.push((chunk, distance_sq));
    }

    visible.sort_unstable_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)
    });

    for (chunk, _) in visible {
        pass.set_bind_group(2, &chunk.fade_bind_group, &[]);
        pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
        pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..chunk.index_count, 0, 0..1);
    }
}

fn chunk_center(chunk_pos: ChunkPos) -> glam::Vec3 {
    glam::Vec3::new(
        chunk_pos.x as f32 * 32.0 + 16.0,
        chunk_pos.y as f32 * 32.0 + 16.0,
        chunk_pos.z as f32 * 32.0 + 16.0,
    )
}

fn extract_frustum_planes(vp: glam::Mat4) -> [[f32; 4]; 6] {
    let m = vp.to_cols_array_2d();
    // The 4 rows of the VP matrix transposed
    let row0 = [m[0][0], m[1][0], m[2][0], m[3][0]];
    let row1 = [m[0][1], m[1][1], m[2][1], m[3][1]];
    let row2 = [m[0][2], m[1][2], m[2][2], m[3][2]];
    let row3 = [m[0][3], m[1][3], m[2][3], m[3][3]];

    // Gribb-Hartmann frustum extraction
    let planes = [
        // Left
        [row3[0]+row0[0], row3[1]+row0[1], row3[2]+row0[2], row3[3]+row0[3]],
        // Right
        [row3[0]-row0[0], row3[1]-row0[1], row3[2]-row0[2], row3[3]-row0[3]],
        // Bottom
        [row3[0]+row1[0], row3[1]+row1[1], row3[2]+row1[2], row3[3]+row1[3]],
        // Top
        [row3[0]-row1[0], row3[1]-row1[1], row3[2]-row1[2], row3[3]-row1[3]],
        // Near
        [row3[0]+row2[0], row3[1]+row2[1], row3[2]+row2[2], row3[3]+row2[3]],
        // Far
        [row3[0]-row2[0], row3[1]-row2[1], row3[2]-row2[2], row3[3]-row2[3]],
    ];

    // Normalize each plane
    let mut result = [[0.0f32; 4]; 6];
    for (i, p) in planes.iter().enumerate() {
        let len = (p[0]*p[0] + p[1]*p[1] + p[2]*p[2]).sqrt();
        if len > 0.0001 {
            result[i] = [p[0]/len, p[1]/len, p[2]/len, p[3]/len];
        }
    }
    result
}

fn aabb_in_frustum(planes: &[[f32; 4]; 6], center: glam::Vec3, half: f32) -> bool {
    for plane in planes {
        let d = plane[0] * center.x + plane[1] * center.y + plane[2] * center.z + plane[3];
        let r = half * (plane[0].abs() + plane[1].abs() + plane[2].abs());
        if d < -r {
            return false; // Completely outside this plane
        }
    }
    true
}
