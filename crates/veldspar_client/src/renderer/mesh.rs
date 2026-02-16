use bytemuck::{Pod, Zeroable};
use noise::{NoiseFn, Perlin};
use veldspar_shared::{
    block::{
        is_bed_block, is_button, is_cactus, is_carpet, is_cobweb, is_fire_block,
        is_glass_pane, is_lava_block, is_lever, is_mushroom, is_slab_block, is_stairs,
        is_trapdoor_block, is_vine, is_water_block, is_wheat_block, lava_level_from_block,
        water_level_from_block, wool_color_index, BlockId, BlockRegistry,
    },
    chunk::ChunkData,
    coords::{local_to_index, ChunkPos, LocalPos, CHUNK_SIZE},
    lighting::LightMap,
};

const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;
const MASK_SIZE: usize = CHUNK_SIZE * CHUNK_SIZE;
const ATLAS_TILE_SIZE: f32 = 16.0;
const ATLAS_SIZE: f32 = 512.0;
const ATLAS_TILES_PER_ROW: u16 = 32;
const WATER_SURFACE_DROP: f32 = 0.125;
const WATER_FACE_INSET: f32 = 0.0;
const TORCH_STICK_WIDTH: f32 = 0.14;
const TORCH_STICK_HEIGHT: f32 = 0.66;
const TORCH_FLAME_SIZE: f32 = 0.24;
const TORCH_FLAME_BASE_OFFSET: f32 = 0.48;
const DOOR_THICKNESS: f32 = 0.1875;
const LADDER_FACE_OFFSET: f32 = 0.0625;
const CARPET_THICKNESS: f32 = 0.0625;
const CACTUS_INSET: f32 = 0.0625;
const FENCE_POST_HALF_WIDTH: f32 = 0.125;
const FENCE_BAR_HALF_WIDTH: f32 = 0.0625;
const FENCE_BAR_HEIGHT: f32 = 0.125;
const FENCE_BAR_LOW_Y: f32 = 0.375;
const FENCE_BAR_HIGH_Y: f32 = 0.75;
const CROSS_PLANT_INSET: f32 = 0.02;
const LEVER_HEIGHT: f32 = 0.625;
const LEVER_HALF_EXTENT: f32 = 0.1875;
const BUTTON_HALF_SIZE: f32 = 0.125;
const BUTTON_HEIGHT: f32 = 0.125;
const MUSHROOM_HEIGHT: f32 = 0.5;
const HEWN_PLANK_BLOCK_ID: BlockId = BlockId(7);
const OAK_LOG_TOP_TILE_BLOCK_ID: BlockId = BlockId(960);
const OAK_LOG_SIDE_TILE_BLOCK_ID: BlockId = BlockId(961);
const CRAFTING_TABLE_TOP_TILE_BLOCK_ID: BlockId = BlockId(962);
const CRAFTING_TABLE_FRONT_TILE_BLOCK_ID: BlockId = BlockId(963);
const CRAFTING_TABLE_SIDE_TILE_BLOCK_ID: BlockId = BlockId(964);
const TORCH_STICK_TILE_BLOCK_ID: BlockId = BlockId(965);
const TORCH_FLAME_TILE_BLOCK_ID: BlockId = BlockId(966);
const LAVA_TINT: [f32; 3] = [1.0, 0.42, 0.08];

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ChunkVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    pub ao: f32,
    pub light: f32,
    pub emissive_light: f32,
    pub tile_origin: [f32; 2],
    pub color: [f32; 3],
}
const _: [(); 64] = [(); std::mem::size_of::<ChunkVertex>()];

impl ChunkVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32,
        4 => Float32,
        5 => Float32,
        6 => Float32x2,
        7 => Float32x3
    ];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ChunkVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChunkMesh {
    pub vertices: Vec<ChunkVertex>,
    pub indices: Vec<u32>,
}

impl ChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ChunkMeshes {
    pub opaque: ChunkMesh,
    pub water: ChunkMesh,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ChunkNeighbors<'a> {
    pub pos_x: Option<&'a ChunkData>,
    pub neg_x: Option<&'a ChunkData>,
    pub pos_y: Option<&'a ChunkData>,
    pub neg_y: Option<&'a ChunkData>,
    pub pos_z: Option<&'a ChunkData>,
    pub neg_z: Option<&'a ChunkData>,
}

#[derive(Copy, Clone)]
struct FaceSpec {
    axis: usize,
    sign: i32,
    u_axis: usize,
    v_axis: usize,
    normal: [f32; 3],
}

const FACE_SPECS: [FaceSpec; 6] = [
    // +X
    FaceSpec {
        axis: 0,
        sign: 1,
        u_axis: 1,
        v_axis: 2,
        normal: [1.0, 0.0, 0.0],
    },
    // -X
    FaceSpec {
        axis: 0,
        sign: -1,
        u_axis: 2,
        v_axis: 1,
        normal: [-1.0, 0.0, 0.0],
    },
    // +Y
    FaceSpec {
        axis: 1,
        sign: 1,
        u_axis: 2,
        v_axis: 0,
        normal: [0.0, 1.0, 0.0],
    },
    // -Y
    FaceSpec {
        axis: 1,
        sign: -1,
        u_axis: 0,
        v_axis: 2,
        normal: [0.0, -1.0, 0.0],
    },
    // +Z
    FaceSpec {
        axis: 2,
        sign: 1,
        u_axis: 0,
        v_axis: 1,
        normal: [0.0, 0.0, 1.0],
    },
    // -Z
    FaceSpec {
        axis: 2,
        sign: -1,
        u_axis: 1,
        v_axis: 0,
        normal: [0.0, 0.0, -1.0],
    },
];

pub fn build_chunk_meshes(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: &ChunkNeighbors<'_>,
    chunk_pos: ChunkPos,
    world_seed: u64,
    light_map: Option<&LightMap>,
    emissive_light_map: Option<&LightMap>,
    lod_level: u8,
) -> ChunkMeshes {
    let mut opaque_mesh = ChunkMesh {
        vertices: Vec::with_capacity(8_192),
        indices: Vec::with_capacity(12_288),
    };
    let mut water_mesh = ChunkMesh {
        vertices: Vec::with_capacity(2_048),
        indices: Vec::with_capacity(3_072),
    };
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];
    let mut mask = vec![None::<BlockId>; MASK_SIZE];
    let mut ao_fingerprints = vec![0u8; MASK_SIZE];
    let mut light_levels = vec![15u8; MASK_SIZE];

    let biome_perlin = Perlin::new(world_seed.wrapping_add(3) as u32);
    let humidity_perlin = Perlin::new(world_seed.wrapping_add(7) as u32);
    let lava_source_id = registry.get_by_name("lava_source");
    let still_water_id = registry.get_by_name("still_water");
    let torch_id = registry.get_by_name("torch");

    // Build opaque mesh
    for face in FACE_SPECS {
        for slice in 0..CHUNK_SIZE {
            mask.fill(None);
            ao_fingerprints.fill(0);
            light_levels.fill(15);

            for v in 0..CHUNK_SIZE {
                for u in 0..CHUNK_SIZE {
                    let block_coords = face_block_coords(face, slice, u, v);
                    let block = sample_block(chunk, neighbors, block_coords);
                    if !is_solid(block, registry) {
                        continue;
                    }
                    if lod_level == 0
                        && (block == BlockId::TIMBER_LOG || block == BlockId::CRAFTING_TABLE)
                    {
                        continue;
                    }

                    let mut adjacent_coords = block_coords;
                    adjacent_coords[face.axis] += face.sign;
                    let adjacent = sample_block(chunk, neighbors, adjacent_coords);

                    // At LOD 1, treat CANOPY_LEAVES as opaque (cull leaves-to-leaves faces)
                    let should_render_face = if lod_level >= 1 && block == BlockId::CANOPY_LEAVES && adjacent == BlockId::CANOPY_LEAVES {
                        false
                    } else {
                        adjacent == BlockId::AIR || registry.get_properties(adjacent).transparent
                    };

                    if should_render_face {
                        let idx = v * CHUNK_SIZE + u;
                        mask[idx] = Some(block);
                        if lod_level == 0 {
                            ao_fingerprints[idx] = compute_block_ao_fingerprint(
                                chunk, registry, neighbors, face, slice, u, v,
                            );
                            light_levels[idx] = face_light_level(light_map, adjacent_coords);
                        }
                    }
                }
            }

            for v in 0..CHUNK_SIZE {
                let mut u = 0usize;
                while u < CHUNK_SIZE {
                    let idx = v * CHUNK_SIZE + u;
                    let Some(block_id) = mask[idx] else {
                        u += 1;
                        continue;
                    };
                    let ao_fingerprint = ao_fingerprints[idx];
                    let light_level = light_levels[idx];

                    let mut width = 1usize;
                    while u + width < CHUNK_SIZE
                        && mask[v * CHUNK_SIZE + (u + width)] == Some(block_id)
                        && ao_fingerprints[v * CHUNK_SIZE + (u + width)] == ao_fingerprint
                        && light_levels[v * CHUNK_SIZE + (u + width)] == light_level
                    {
                        width += 1;
                    }

                    let mut height = 1usize;
                    'height: while v + height < CHUNK_SIZE {
                        let row = (v + height) * CHUNK_SIZE;
                        for du in 0..width {
                            if mask[row + u + du] != Some(block_id)
                                || ao_fingerprints[row + u + du] != ao_fingerprint
                                || light_levels[row + u + du] != light_level
                            {
                                break 'height;
                            }
                        }
                        height += 1;
                    }

                    let block_coords = face_block_coords(face, slice, u, v);
                    let world_x = chunk_pos.x * CHUNK_SIZE as i32 + block_coords[0];
                    let world_z = chunk_pos.z * CHUNK_SIZE as i32 + block_coords[2];
                    let tint =
                        biome_tint_color(&biome_perlin, &humidity_perlin, world_x, world_z, block_id, registry);

                    emit_quad(
                        &mut opaque_mesh,
                        chunk,
                        registry,
                        neighbors,
                        light_map,
                        emissive_light_map,
                        world_offset,
                        face,
                        slice,
                        u,
                        v,
                        width,
                        height,
                        block_id,
                        None,
                        false,
                        tint,
                    );

                    for dv in 0..height {
                        let row = (v + dv) * CHUNK_SIZE;
                        for du in 0..width {
                            mask[row + u + du] = None;
                            ao_fingerprints[row + u + du] = 0;
                            light_levels[row + u + du] = 15;
                        }
                    }

                    u += width;
                }
            }
        }
    }

    // Build lava mesh into opaque pass (no transparency).
    if lava_source_id.is_some() {
        for face in FACE_SPECS {
            for slice in 0..CHUNK_SIZE {
                mask.fill(None);
                ao_fingerprints.fill(0);
                light_levels.fill(15);

                for v in 0..CHUNK_SIZE {
                    for u in 0..CHUNK_SIZE {
                        let block_coords = face_block_coords(face, slice, u, v);
                        let block = sample_block(chunk, neighbors, block_coords);
                        if !is_lava_block(block) {
                            continue;
                        }

                        let mut adjacent_coords = block_coords;
                        adjacent_coords[face.axis] += face.sign;
                        let adjacent = sample_block(chunk, neighbors, adjacent_coords);

                        if !is_lava_block(adjacent)
                            && (adjacent == BlockId::AIR
                                || registry.get_properties(adjacent).transparent)
                        {
                            let idx = v * CHUNK_SIZE + u;
                            mask[idx] = Some(block);
                            if lod_level == 0 {
                                ao_fingerprints[idx] = compute_block_ao_fingerprint(
                                    chunk, registry, neighbors, face, slice, u, v,
                                );
                                light_levels[idx] = face_light_level(light_map, adjacent_coords);
                            }
                        }
                    }
                }

                for v in 0..CHUNK_SIZE {
                    let mut u = 0usize;
                    while u < CHUNK_SIZE {
                        let idx = v * CHUNK_SIZE + u;
                        let Some(block_id) = mask[idx] else {
                            u += 1;
                            continue;
                        };
                        let ao_fingerprint = ao_fingerprints[idx];
                        let light_level = light_levels[idx];
                        let is_fluid_face = true;

                        let mut width = 1usize;
                        if !is_fluid_face {
                            while u + width < CHUNK_SIZE
                                && mask[v * CHUNK_SIZE + (u + width)] == Some(block_id)
                                && ao_fingerprints[v * CHUNK_SIZE + (u + width)] == ao_fingerprint
                                && light_levels[v * CHUNK_SIZE + (u + width)] == light_level
                            {
                                width += 1;
                            }
                        }

                        let mut height = 1usize;
                        if !is_fluid_face {
                            'height: while v + height < CHUNK_SIZE {
                                let row = (v + height) * CHUNK_SIZE;
                                for du in 0..width {
                                    if mask[row + u + du] != Some(block_id)
                                        || ao_fingerprints[row + u + du] != ao_fingerprint
                                        || light_levels[row + u + du] != light_level
                                    {
                                        break 'height;
                                    }
                                }
                                height += 1;
                            }
                        }

                        emit_quad(
                            &mut opaque_mesh,
                            chunk,
                            registry,
                            neighbors,
                            light_map,
                            emissive_light_map,
                            world_offset,
                            face,
                            slice,
                            u,
                            v,
                            width,
                            height,
                            block_id,
                            lava_level_from_block(block_id),
                            false,
                            LAVA_TINT,
                        );

                        for dv in 0..height {
                            let row = (v + dv) * CHUNK_SIZE;
                            for du in 0..width {
                                mask[row + u + du] = None;
                                ao_fingerprints[row + u + du] = 0;
                                light_levels[row + u + du] = 15;
                            }
                        }

                        u += width;
                    }
                }
            }
        }
    }

    // Build water mesh (skip for LOD >= 1)
    if lod_level == 0 && still_water_id.is_some() {
        for face in FACE_SPECS {
            for slice in 0..CHUNK_SIZE {
                mask.fill(None);
                ao_fingerprints.fill(0);
                light_levels.fill(15);

                for v in 0..CHUNK_SIZE {
                    for u in 0..CHUNK_SIZE {
                        let block_coords = face_block_coords(face, slice, u, v);
                        let block = sample_block(chunk, neighbors, block_coords);
                        if !is_water_block(block) {
                            continue;
                        }

                        let mut adjacent_coords = block_coords;
                        adjacent_coords[face.axis] += face.sign;
                        let adjacent = sample_block(chunk, neighbors, adjacent_coords);

                        // Water face is visible when adjacent is air or transparent (not water).
                        if !is_water_block(adjacent)
                            && (adjacent == BlockId::AIR
                                || registry.get_properties(adjacent).transparent)
                        {
                            let idx = v * CHUNK_SIZE + u;
                            mask[idx] = Some(block);
                            ao_fingerprints[idx] = compute_block_ao_fingerprint(
                                chunk, registry, neighbors, face, slice, u, v,
                            );
                            light_levels[idx] = face_light_level(light_map, adjacent_coords);
                        }
                    }
                }

                for v in 0..CHUNK_SIZE {
                    let mut u = 0usize;
                    while u < CHUNK_SIZE {
                        let idx = v * CHUNK_SIZE + u;
                        let Some(block_id) = mask[idx] else {
                            u += 1;
                            continue;
                        };
                        let ao_fingerprint = ao_fingerprints[idx];
                        let light_level = light_levels[idx];
                        let is_fluid_face = true;

                        let mut width = 1usize;
                        if !is_fluid_face {
                            while u + width < CHUNK_SIZE
                                && mask[v * CHUNK_SIZE + (u + width)] == Some(block_id)
                                && ao_fingerprints[v * CHUNK_SIZE + (u + width)] == ao_fingerprint
                                && light_levels[v * CHUNK_SIZE + (u + width)] == light_level
                            {
                                width += 1;
                            }
                        }

                        let mut height = 1usize;
                        if !is_fluid_face {
                            'height: while v + height < CHUNK_SIZE {
                                let row = (v + height) * CHUNK_SIZE;
                                for du in 0..width {
                                    if mask[row + u + du] != Some(block_id)
                                        || ao_fingerprints[row + u + du] != ao_fingerprint
                                        || light_levels[row + u + du] != light_level
                                    {
                                        break 'height;
                                    }
                                }
                                height += 1;
                            }
                        }

                        emit_quad(
                            &mut water_mesh,
                            chunk,
                            registry,
                            neighbors,
                            light_map,
                            emissive_light_map,
                            world_offset,
                            face,
                            slice,
                            u,
                            v,
                            width,
                            height,
                            block_id,
                            water_level_from_block(block_id),
                            true,
                            [1.0, 1.0, 1.0],
                        );

                        for dv in 0..height {
                            let row = (v + dv) * CHUNK_SIZE;
                            for du in 0..width {
                                mask[row + u + du] = None;
                                ao_fingerprints[row + u + du] = 0;
                                light_levels[row + u + du] = 15;
                            }
                        }

                        u += width;
                    }
                }
            }
        }
    }

    // Custom meshes (skip for LOD >= 1 — not visible at distance)
    if lod_level == 0 {
        append_multiface_block_mesh(
            &mut opaque_mesh,
            chunk,
            registry,
            neighbors,
            light_map,
            chunk_pos,
        );
        if let Some(torch_id) = torch_id {
            append_torch_mesh(
                &mut opaque_mesh,
                chunk,
                light_map,
                chunk_pos,
                torch_id,
            );
        }
        append_door_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_ladder_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_vine_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_fence_mesh(&mut opaque_mesh, chunk, neighbors, light_map, chunk_pos);
        append_trapdoor_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_bed_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_pressure_plate_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_carpet_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_slab_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_cactus_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_stairs_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_lever_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_button_mesh(&mut opaque_mesh, chunk, light_map, chunk_pos);
        append_cross_plant_mesh(
            &mut opaque_mesh,
            chunk,
            light_map,
            chunk_pos,
            registry,
            &biome_perlin,
            &humidity_perlin,
        );
        append_glass_pane_mesh(&mut water_mesh, chunk, light_map, chunk_pos);
    }

    ChunkMeshes {
        opaque: opaque_mesh,
        water: water_mesh,
    }
}

pub fn build_chunk_mesh(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: &ChunkNeighbors<'_>,
    chunk_pos: ChunkPos,
    world_seed: u64,
    light_map: Option<&LightMap>,
    emissive_light_map: Option<&LightMap>,
    lod_level: u8,
) -> ChunkMeshes {
    build_chunk_meshes(
        chunk,
        registry,
        neighbors,
        chunk_pos,
        world_seed,
        light_map,
        emissive_light_map,
        lod_level,
    )
}

fn biome_tint_color(
    biome_noise: &Perlin,
    humidity_noise: &Perlin,
    world_x: i32,
    world_z: i32,
    block_id: BlockId,
    registry: &BlockRegistry,
) -> [f32; 3] {
    let block_name = registry.get_properties(block_id).name.as_str();
    if block_name != "verdant_turf"
        && block_name != "canopy_leaves"
        && block_name != "tall_grass"
        && block_name != "wildflower"
        && block_name != "sapling"
        && block_name != "sugar_cane"
    {
        return [1.0, 1.0, 1.0];
    }

    let wx = world_x as f64;
    let wz = world_z as f64;
    let temperature = ((biome_noise.get([wx * 0.002, wz * 0.002]) + 1.0) * 0.5) as f32;
    let humidity = ((humidity_noise.get([wx * 0.0025, wz * 0.0025]) + 1.0) * 0.5) as f32;
    let is_grass = block_name == "verdant_turf";
    let dryness = 1.0 - humidity;
    let cold_w = (1.0 - temperature).powf(1.3);
    let hot_w = temperature.powf(1.2);
    let wet_w = humidity.powf(1.1);
    let temperate_w = (1.0 - (temperature - 0.5).abs() * 2.0).max(0.0);
    let lush_w = (hot_w * wet_w).powf(0.75);
    let arid_w = (hot_w * dryness).powf(0.9);
    let snowy_w = (cold_w * (0.6 + wet_w * 0.4)).powf(0.9);
    let plains_w = temperate_w * (0.55 + dryness * 0.45);

    let mut mix = [0.0f32; 3];
    let mut weight_sum = 0.0f32;
    let palette: [([f32; 3], f32); 5] = [
        ([0.57, 0.72, 0.58], snowy_w),
        ([0.17, 0.56, 0.15], lush_w),
        ([0.71, 0.68, 0.34], arid_w),
        ([0.44, 0.71, 0.33], plains_w),
        ([0.29, 0.52, 0.23], temperate_w * wet_w),
    ];

    for (color, weight) in palette {
        mix[0] += color[0] * weight;
        mix[1] += color[1] * weight;
        mix[2] += color[2] * weight;
        weight_sum += weight;
    }

    if weight_sum > 0.0001 {
        mix[0] /= weight_sum;
        mix[1] /= weight_sum;
        mix[2] /= weight_sum;
    } else {
        mix = [0.42, 0.68, 0.32];
    }

    let noise_jitter = ((biome_noise.get([wx * 0.01 + 201.0, wz * 0.01 - 77.0]) as f32) * 0.03)
        .clamp(-0.03, 0.03);
    mix[0] = (mix[0] + noise_jitter * 0.4).clamp(0.05, 0.95);
    mix[1] = (mix[1] + noise_jitter).clamp(0.05, 0.95);
    mix[2] = (mix[2] - noise_jitter * 0.25).clamp(0.05, 0.95);

    // Keep foliage from drifting into muddy gray in extreme biome blends.
    let min_green = ((mix[0] + mix[2]) * 0.55 + 0.08).clamp(0.1, 0.9);
    mix[1] = mix[1].max(min_green).clamp(0.05, 0.95);

    if is_grass {
        mix
    } else {
        [
            (mix[0] * 0.86).clamp(0.03, 1.0),
            (mix[1] * 0.82).clamp(0.03, 1.0),
            (mix[2] * 0.72).clamp(0.03, 1.0),
        ]
    }
}

fn emit_quad(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: &ChunkNeighbors<'_>,
    light_map: Option<&LightMap>,
    emissive_light_map: Option<&LightMap>,
    world_offset: [f32; 3],
    face: FaceSpec,
    slice: usize,
    u: usize,
    v: usize,
    width: usize,
    height: usize,
    block_id: BlockId,
    fluid_level: Option<u8>,
    render_backface_on_top: bool,
    tint: [f32; 3],
) {
    const FLUID_TOP_VERTEX_EPSILON: f32 = 1e-4;

    let plane = if face.sign > 0 { slice + 1 } else { slice };

    let mut p0 = [0.0f32; 3];
    p0[face.axis] = plane as f32;
    p0[face.u_axis] = u as f32;
    p0[face.v_axis] = v as f32;

    let mut p1 = p0;
    p1[face.u_axis] += width as f32;

    let mut p2 = p1;
    p2[face.v_axis] += height as f32;

    let mut p3 = p0;
    p3[face.v_axis] += height as f32;

    p0[0] += world_offset[0];
    p0[1] += world_offset[1];
    p0[2] += world_offset[2];
    p1[0] += world_offset[0];
    p1[1] += world_offset[1];
    p1[2] += world_offset[2];
    p2[0] += world_offset[0];
    p2[1] += world_offset[1];
    p2[2] += world_offset[2];
    p3[0] += world_offset[0];
    p3[1] += world_offset[1];
    p3[2] += world_offset[2];

    if let Some(level) = fluid_level {
        let block_coords = face_block_coords(face, slice, u, v);
        let block_base_x = world_offset[0] + block_coords[0] as f32;
        let block_base_y = world_offset[1] + block_coords[1] as f32;
        let block_base_z = world_offset[2] + block_coords[2] as f32;
        let corner_heights = fluid_top_corner_heights(
            chunk,
            neighbors,
            block_coords,
            is_lava_block(block_id),
            level,
        );

        let apply_height = |vertex: &mut [f32; 3]| {
            let is_top_vertex = (vertex[1] - (block_base_y + 1.0)).abs() <= FLUID_TOP_VERTEX_EPSILON;
            if !is_top_vertex {
                return;
            }

            let near_x = (vertex[0] - block_base_x).abs() <= (vertex[0] - (block_base_x + 1.0)).abs();
            let near_z = (vertex[2] - block_base_z).abs() <= (vertex[2] - (block_base_z + 1.0)).abs();
            let corner_height = match (near_x, near_z) {
                (true, true) => corner_heights[0],
                (true, false) => corner_heights[1],
                (false, false) => corner_heights[2],
                (false, true) => corner_heights[3],
            };

            vertex[1] = block_base_y + corner_height;
        };

        apply_height(&mut p0);
        apply_height(&mut p1);
        apply_height(&mut p2);
        apply_height(&mut p3);

        if face.axis == 1 && face.sign == 1 {
            p0[1] = block_base_y + corner_heights[0];
            p1[1] = block_base_y + corner_heights[1];
            p2[1] = block_base_y + corner_heights[2];
            p3[1] = block_base_y + corner_heights[3];
        }

        // Pull water faces slightly inward so they do not fight with coplanar adjacent geometry.
        let inset_x = face.normal[0] * WATER_FACE_INSET;
        let inset_y = face.normal[1] * WATER_FACE_INSET;
        let inset_z = face.normal[2] * WATER_FACE_INSET;

        p0[0] -= inset_x;
        p0[1] -= inset_y;
        p0[2] -= inset_z;
        p1[0] -= inset_x;
        p1[1] -= inset_y;
        p1[2] -= inset_z;
        p2[0] -= inset_x;
        p2[1] -= inset_y;
        p2[2] -= inset_z;
        p3[0] -= inset_x;
        p3[1] -= inset_y;
        p3[2] -= inset_z;
    }

    let tile_origin = tile_origin_for_block(block_id);

    // Local UVs: 0..width / 0..height so the shader can fract() to tile
    let w = width as f32;
    let h = height as f32;
    let tex_coords = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];

    let ao_values = compute_quad_ao(chunk, registry, neighbors, face, slice, u, v, width, height);
    let light_values = compute_quad_light_values(light_map, face, slice, u, v, width, height);
    let emissive_values = compute_quad_emissive_light_values(
        emissive_light_map,
        face,
        slice,
        u,
        v,
        width,
        height,
    );
    let positions = [p0, p1, p2, p3];

    push_quad_lit(
        mesh,
        positions,
        face.normal,
        tex_coords,
        ao_values,
        light_values,
        emissive_values,
        tile_origin,
        tint,
        false,
    );

    if render_backface_on_top && face.axis == 1 && face.sign == 1 {
        // Keep fluid surfaces visible from below when back-face culling is enabled.
        push_quad_lit(
            mesh,
            positions,
            [0.0, -1.0, 0.0],
            tex_coords,
            ao_values,
            light_values,
            emissive_values,
            tile_origin,
            tint,
            true,
        );
    }
}

fn fluid_level_to_surface_height(level: u8) -> f32 {
    (1.0 - WATER_SURFACE_DROP * (f32::from(level) + 1.0)).clamp(0.0, 1.0)
}

fn sample_surface_height_at(
    chunk: &ChunkData,
    neighbors: &ChunkNeighbors<'_>,
    world_block_coords: [i32; 3],
    is_lava: bool,
) -> Option<f32> {
    let block = sample_block(chunk, neighbors, world_block_coords);
    if is_lava {
        lava_level_from_block(block).map(fluid_level_to_surface_height)
    } else {
        water_level_from_block(block).map(fluid_level_to_surface_height)
    }
}

fn fluid_top_corner_heights(
    chunk: &ChunkData,
    neighbors: &ChunkNeighbors<'_>,
    block_coords: [i32; 3],
    is_lava: bool,
    current_level: u8,
) -> [f32; 4] {
    let base_height = fluid_level_to_surface_height(current_level);
    let mut heights = [base_height; 4];
    let corner_signs = [(0, 0), (0, 1), (1, 1), (1, 0)];

    for (index, &(corner_x, corner_z)) in corner_signs.iter().enumerate() {
        let x_offsets = if corner_x == 0 { [0, -1] } else { [0, 1] };
        let z_offsets = if corner_z == 0 { [0, -1] } else { [0, 1] };

        let mut sum = 0.0;
        let mut count = 0u8;
        for x_off in x_offsets {
            for z_off in z_offsets {
                if let Some(height) = sample_surface_height_at(
                    chunk,
                    neighbors,
                    [block_coords[0] + x_off, block_coords[1], block_coords[2] + z_off],
                    is_lava,
                ) {
                    sum += height;
                    count += 1;
                }
            }
        }

        if count > 0 {
            heights[index] = (sum / f32::from(count)).clamp(0.0, 1.0);
        }
    }

    heights
}

fn append_torch_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
    torch_id: BlockId,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];
    let stick_tile = tile_origin_for_block(TORCH_STICK_TILE_BLOCK_ID);
    let flame_tile = tile_origin_for_block(TORCH_FLAME_TILE_BLOCK_ID);

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                if chunk.get(local) != torch_id {
                    continue;
                }

                let x0 = world_offset[0] + x as f32 + 0.5 - TORCH_STICK_WIDTH * 0.5;
                let x1 = world_offset[0] + x as f32 + 0.5 + TORCH_STICK_WIDTH * 0.5;
                let y0 = world_offset[1] + y as f32;
                let y1 = y0 + TORCH_STICK_HEIGHT;
                let z0 = world_offset[2] + z as f32 + 0.5 - TORCH_STICK_WIDTH * 0.5;
                let z1 = world_offset[2] + z as f32 + 0.5 + TORCH_STICK_WIDTH * 0.5;

                emit_box(
                    mesh,
                    light_map,
                    [x as i32, y as i32, z as i32],
                    [x0, y0, z0],
                    [x1, y1, z1],
                    stick_tile,
                    true,
                );

                let flame_base = world_offset[1] + y as f32 + TORCH_FLAME_BASE_OFFSET;
                let flame_top = flame_base + TORCH_FLAME_SIZE;
                let flame_half = TORCH_FLAME_SIZE * 0.5;
                let flame_light = (f32::from(sample_light_level(
                    light_map,
                    [x as i32, y as i32, z as i32],
                )) / 15.0)
                    .max(0.88);
                let light_values = [flame_light; 4];
                emit_cross_mesh(
                    mesh,
                    [
                        world_offset[0] + x as f32 + 0.5 - flame_half,
                        flame_base,
                        world_offset[2] + z as f32 + 0.5 - flame_half,
                    ],
                    [
                        world_offset[0] + x as f32 + 0.5 + flame_half,
                        flame_top,
                        world_offset[2] + z as f32 + 0.5 + flame_half,
                    ],
                    light_values,
                    flame_tile,
                    [1.0, 0.9, 0.65],
                );
            }
        }
    }
}

fn append_multiface_block_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: &ChunkNeighbors<'_>,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    let log_side = tile_origin_for_block(OAK_LOG_SIDE_TILE_BLOCK_ID);
    let log_top = tile_origin_for_block(OAK_LOG_TOP_TILE_BLOCK_ID);
    let table_side = tile_origin_for_block(CRAFTING_TABLE_SIDE_TILE_BLOCK_ID);
    let table_top = tile_origin_for_block(CRAFTING_TABLE_TOP_TILE_BLOCK_ID);
    let table_front = tile_origin_for_block(CRAFTING_TABLE_FRONT_TILE_BLOCK_ID);
    let table_bottom = tile_origin_for_block(HEWN_PLANK_BLOCK_ID);

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);
                if block != BlockId::TIMBER_LOG && block != BlockId::CRAFTING_TABLE {
                    continue;
                }

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;
                let base_coords = [x as i32, y as i32, z as i32];

                let faces = [
                    should_render_solid_face(
                        sample_block(
                            chunk,
                            neighbors,
                            [base_coords[0] + 1, base_coords[1], base_coords[2]],
                        ),
                        registry,
                    ),
                    should_render_solid_face(
                        sample_block(
                            chunk,
                            neighbors,
                            [base_coords[0] - 1, base_coords[1], base_coords[2]],
                        ),
                        registry,
                    ),
                    should_render_solid_face(
                        sample_block(
                            chunk,
                            neighbors,
                            [base_coords[0], base_coords[1] + 1, base_coords[2]],
                        ),
                        registry,
                    ),
                    should_render_solid_face(
                        sample_block(
                            chunk,
                            neighbors,
                            [base_coords[0], base_coords[1] - 1, base_coords[2]],
                        ),
                        registry,
                    ),
                    should_render_solid_face(
                        sample_block(
                            chunk,
                            neighbors,
                            [base_coords[0], base_coords[1], base_coords[2] + 1],
                        ),
                        registry,
                    ),
                    should_render_solid_face(
                        sample_block(
                            chunk,
                            neighbors,
                            [base_coords[0], base_coords[1], base_coords[2] - 1],
                        ),
                        registry,
                    ),
                ];

                let face_tiles = if block == BlockId::TIMBER_LOG {
                    [log_side, log_side, log_top, log_top, log_side, log_side]
                } else {
                    [
                        table_side,  // +X
                        table_side,  // -X
                        table_top,   // +Y
                        table_bottom, // -Y
                        table_side,  // +Z
                        table_front, // -Z
                    ]
                };

                emit_box_faces_tiled(
                    mesh,
                    light_map,
                    base_coords,
                    [bx, by, bz],
                    [bx + 1.0, by + 1.0, bz + 1.0],
                    face_tiles,
                    false,
                    faces,
                );
            }
        }
    }
}

fn should_render_solid_face(neighbor: BlockId, registry: &BlockRegistry) -> bool {
    neighbor == BlockId::AIR || registry.get_properties(neighbor).transparent
}

#[derive(Copy, Clone)]
enum HorizontalFacing {
    North,
    East,
    South,
    West,
}

fn append_door_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);
                let Some((facing, is_open)) = door_variant(block) else {
                    continue;
                };

                let ([min_x, min_y, min_z], [max_x, max_y, max_z]) = door_bounds(facing, is_open);
                emit_box(
                    mesh,
                    light_map,
                    [x as i32, y as i32, z as i32],
                    [
                        world_offset[0] + x as f32 + min_x,
                        world_offset[1] + y as f32 + min_y,
                        world_offset[2] + z as f32 + min_z,
                    ],
                    [
                        world_offset[0] + x as f32 + max_x,
                        world_offset[1] + y as f32 + max_y,
                        world_offset[2] + z as f32 + max_z,
                    ],
                    tile_origin_for_block(block),
                    false,
                );
            }
        }
    }
}

fn append_ladder_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);
                let Some(facing) = ladder_facing(block) else {
                    continue;
                };

                let x0 = world_offset[0] + x as f32;
                let x1 = x0 + 1.0;
                let y0 = world_offset[1] + y as f32;
                let y1 = y0 + 1.0;
                let z0 = world_offset[2] + z as f32;
                let z1 = z0 + 1.0;
                let tile_origin = tile_origin_for_block(block);

                match facing {
                    HorizontalFacing::North => {
                        let z = z1 - LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
                            [0.0, 0.0, -1.0],
                            tile_origin,
                        );
                    }
                    HorizontalFacing::East => {
                        let x = x0 + LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x, y0, z0], [x, y0, z1], [x, y1, z1], [x, y1, z0]],
                            [1.0, 0.0, 0.0],
                            tile_origin,
                        );
                    }
                    HorizontalFacing::South => {
                        let z = z0 + LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x1, y0, z], [x0, y0, z], [x0, y1, z], [x1, y1, z]],
                            [0.0, 0.0, 1.0],
                            tile_origin,
                        );
                    }
                    HorizontalFacing::West => {
                        let x = x1 - LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x, y0, z1], [x, y0, z0], [x, y1, z0], [x, y1, z1]],
                            [-1.0, 0.0, 0.0],
                            tile_origin,
                        );
                    }
                }
            }
        }
    }
}

fn append_vine_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);
                let Some(facing) = vine_facing(block) else {
                    continue;
                };

                let x0 = world_offset[0] + x as f32;
                let x1 = x0 + 1.0;
                let y0 = world_offset[1] + y as f32;
                let y1 = y0 + 1.0;
                let z0 = world_offset[2] + z as f32;
                let z1 = z0 + 1.0;
                let tile_origin = tile_origin_for_block(block);

                match facing {
                    HorizontalFacing::North => {
                        let z = z0 + LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x1, y0, z], [x0, y0, z], [x0, y1, z], [x1, y1, z]],
                            [0.0, 0.0, -1.0],
                            tile_origin,
                        );
                    }
                    HorizontalFacing::East => {
                        let x = x1 - LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x, y0, z0], [x, y0, z1], [x, y1, z1], [x, y1, z0]],
                            [1.0, 0.0, 0.0],
                            tile_origin,
                        );
                    }
                    HorizontalFacing::South => {
                        let z = z1 - LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
                            [0.0, 0.0, 1.0],
                            tile_origin,
                        );
                    }
                    HorizontalFacing::West => {
                        let x = x0 + LADDER_FACE_OFFSET;
                        emit_ladder_quad(
                            mesh,
                            light_map,
                            [x as i32, y as i32, z as i32],
                            [[x, y0, z1], [x, y0, z0], [x, y1, z0], [x, y1, z1]],
                            [-1.0, 0.0, 0.0],
                            tile_origin,
                        );
                    }
                }
            }
        }
    }
}

fn append_fence_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    neighbors: &ChunkNeighbors<'_>,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];
    let tile_origin = tile_origin_for_block(BlockId::FENCE);

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                if chunk.get(local) != BlockId::FENCE {
                    continue;
                }

                let xf = x as f32;
                let yf = y as f32;
                let zf = z as f32;
                emit_box(
                    mesh,
                    light_map,
                    [x as i32, y as i32, z as i32],
                    [
                        world_offset[0] + xf + 0.5 - FENCE_POST_HALF_WIDTH,
                        world_offset[1] + yf,
                        world_offset[2] + zf + 0.5 - FENCE_POST_HALF_WIDTH,
                    ],
                    [
                        world_offset[0] + xf + 0.5 + FENCE_POST_HALF_WIDTH,
                        world_offset[1] + yf + 1.0,
                        world_offset[2] + zf + 0.5 + FENCE_POST_HALF_WIDTH,
                    ],
                    tile_origin,
                    false,
                );

                let base_coords = [x as i32, y as i32, z as i32];
                let north = sample_block(chunk, neighbors, [base_coords[0], base_coords[1], base_coords[2] - 1]) == BlockId::FENCE;
                let south = sample_block(chunk, neighbors, [base_coords[0], base_coords[1], base_coords[2] + 1]) == BlockId::FENCE;
                let east = sample_block(chunk, neighbors, [base_coords[0] + 1, base_coords[1], base_coords[2]]) == BlockId::FENCE;
                let west = sample_block(chunk, neighbors, [base_coords[0] - 1, base_coords[1], base_coords[2]]) == BlockId::FENCE;

                for bar_y in [FENCE_BAR_LOW_Y, FENCE_BAR_HIGH_Y] {
                    if north {
                        emit_fence_connection(
                            mesh,
                            light_map,
                            base_coords,
                            world_offset,
                            HorizontalFacing::North,
                            bar_y,
                            tile_origin,
                        );
                    }
                    if south {
                        emit_fence_connection(
                            mesh,
                            light_map,
                            base_coords,
                            world_offset,
                            HorizontalFacing::South,
                            bar_y,
                            tile_origin,
                        );
                    }
                    if east {
                        emit_fence_connection(
                            mesh,
                            light_map,
                            base_coords,
                            world_offset,
                            HorizontalFacing::East,
                            bar_y,
                            tile_origin,
                        );
                    }
                    if west {
                        emit_fence_connection(
                            mesh,
                            light_map,
                            base_coords,
                            world_offset,
                            HorizontalFacing::West,
                            bar_y,
                            tile_origin,
                        );
                    }
                }
            }
        }
    }
}

fn append_trapdoor_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];
    let tile_origin = tile_origin_for_block(BlockId::TRAPDOOR_CLOSED);

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                let block = chunk.get(local);
                if !is_trapdoor_block(block) {
                    continue;
                }

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;

                // Determine if open or closed based on block ID
                let is_open = matches!(block, b if b == BlockId::TRAPDOOR_OPEN || b == BlockId::TRAPDOOR_OPEN_EAST || b == BlockId::TRAPDOOR_OPEN_SOUTH || b == BlockId::TRAPDOOR_OPEN_WEST);

                if is_open {
                    // Open: vertical thin slab on one side
                    let facing = if block == BlockId::TRAPDOOR_OPEN { 0 }
                        else if block == BlockId::TRAPDOOR_OPEN_EAST { 1 }
                        else if block == BlockId::TRAPDOOR_OPEN_SOUTH { 2 }
                        else { 3 };
                    let thickness = 0.1875_f32;
                    let (min, max) = match facing {
                        0 => ([bx, by, bz], [bx + 1.0, by + 1.0, bz + thickness]),           // North
                        1 => ([bx + 1.0 - thickness, by, bz], [bx + 1.0, by + 1.0, bz + 1.0]), // East
                        2 => ([bx, by, bz + 1.0 - thickness], [bx + 1.0, by + 1.0, bz + 1.0]), // South
                        _ => ([bx, by, bz], [bx + thickness, by + 1.0, bz + 1.0]),            // West
                    };
                    emit_box(mesh, light_map, [x as i32, y as i32, z as i32], min, max, tile_origin, false);
                } else {
                    // Closed: horizontal thin slab at bottom
                    let thickness = 0.1875_f32;
                    emit_box(
                        mesh, light_map,
                        [x as i32, y as i32, z as i32],
                        [bx, by, bz],
                        [bx + 1.0, by + thickness, bz + 1.0],
                        tile_origin, false,
                    );
                }
            }
        }
    }
}

fn append_bed_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                let block = chunk.get(local);
                if !is_bed_block(block) {
                    continue;
                }

                let is_head = matches!(block, b if b == BlockId::BED_HEAD || b == BlockId::BED_HEAD_EAST || b == BlockId::BED_HEAD_SOUTH || b == BlockId::BED_HEAD_WEST);
                let tile_origin = if is_head {
                    tile_origin_for_block(BlockId::BED_HEAD)
                } else {
                    tile_origin_for_block(BlockId::BED_FOOT)
                };

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;
                let bed_height = 0.5625_f32; // 9/16

                emit_box(
                    mesh, light_map,
                    [x as i32, y as i32, z as i32],
                    [bx, by, bz],
                    [bx + 1.0, by + bed_height, bz + 1.0],
                    tile_origin, false,
                );
            }
        }
    }
}

fn append_pressure_plate_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];
    let tile_origin = tile_origin_for_block(BlockId::STONE_PRESSURE_PLATE);

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                if chunk.get(local) != BlockId::STONE_PRESSURE_PLATE {
                    continue;
                }

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;
                let inset = 0.0625_f32; // 1/16
                let height = 0.0625_f32;

                emit_box(
                    mesh, light_map,
                    [x as i32, y as i32, z as i32],
                    [bx + inset, by, bz + inset],
                    [bx + 1.0 - inset, by + height, bz + 1.0 - inset],
                    tile_origin, false,
                );
            }
        }
    }
}

fn append_carpet_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                let block = chunk.get(local);
                if !is_carpet(block) {
                    continue;
                }

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;

                emit_box_faces(
                    mesh,
                    light_map,
                    [x as i32, y as i32, z as i32],
                    [bx, by, bz],
                    [bx + 1.0, by + CARPET_THICKNESS, bz + 1.0],
                    carpet_tile_origin(block),
                    false,
                    [true, true, true, true, true, true],
                );
            }
        }
    }
}

fn append_slab_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                let block = chunk.get(local);
                if !is_slab_block(block) {
                    continue;
                }

                let is_top = block == BlockId::STONE_SLAB_TOP || block == BlockId::WOODEN_SLAB_TOP;
                let tile_origin = if block == BlockId::STONE_SLAB_BOTTOM || block == BlockId::STONE_SLAB_TOP {
                    tile_origin_for_block(BlockId::STONE_SLAB_BOTTOM)
                } else {
                    tile_origin_for_block(BlockId::WOODEN_SLAB_BOTTOM)
                };

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;

                let (y_min, y_max) = if is_top {
                    (by + 0.5, by + 1.0)
                } else {
                    (by, by + 0.5)
                };

                emit_box(
                    mesh, light_map,
                    [x as i32, y as i32, z as i32],
                    [bx, y_min, bz],
                    [bx + 1.0, y_max, bz + 1.0],
                    tile_origin, false,
                );
            }
        }
    }
}

fn append_cactus_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                let block = chunk.get(local);
                if !is_cactus(block) {
                    continue;
                }

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;

                emit_box_faces(
                    mesh,
                    light_map,
                    [x as i32, y as i32, z as i32],
                    [bx + CACTUS_INSET, by, bz + CACTUS_INSET],
                    [bx + 1.0 - CACTUS_INSET, by + 1.0, bz + 1.0 - CACTUS_INSET],
                    tile_origin_for_block(block),
                    false,
                    [true, true, true, true, true, true],
                );
            }
        }
    }
}

fn append_cross_plant_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
    registry: &BlockRegistry,
    biome_noise: &Perlin,
    humidity_noise: &Perlin,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);
                if !is_cross_mesh_block(block) {
                    continue;
                }

                let light = f32::from(sample_light_level(
                    light_map,
                    [x as i32, y as i32, z as i32],
                )) / 15.0;
                let light_values = [light; 4];
                let tile_origin = tile_origin_for_block(block);
                let world_x = chunk_pos.x * CHUNK_SIZE as i32 + x as i32;
                let world_z = chunk_pos.z * CHUNK_SIZE as i32 + z as i32;
                let tint = biome_tint_color(
                    biome_noise,
                    humidity_noise,
                    world_x,
                    world_z,
                    block,
                    registry,
                );

                let x0 = world_offset[0] + x as f32 + CROSS_PLANT_INSET;
                let x1 = world_offset[0] + x as f32 + 1.0 - CROSS_PLANT_INSET;
                let y0 = world_offset[1] + y as f32;
                let y1 = y0 + cross_mesh_height(block);
                let z0 = world_offset[2] + z as f32 + CROSS_PLANT_INSET;
                let z1 = world_offset[2] + z as f32 + 1.0 - CROSS_PLANT_INSET;

                emit_cross_mesh(
                    mesh,
                    [x0, y0, z0],
                    [x1, y1, z1],
                    light_values,
                    tile_origin,
                    tint,
                );
            }
        }
    }
}

fn append_glass_pane_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];
    let tex_coords = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let ao_values = [1.0; 4];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);
                if !is_glass_pane(block) {
                    continue;
                }

                let light = f32::from(sample_light_level(
                    light_map,
                    [x as i32, y as i32, z as i32],
                )) / 15.0;
                let light_values = [light; 4];
                let tile_origin = tile_origin_for_block(BlockId::GLASS_PANE);

                let x0 = world_offset[0] + x as f32;
                let x1 = x0 + 1.0;
                let y0 = world_offset[1] + y as f32;
                let y1 = y0 + 1.0;
                let z0 = world_offset[2] + z as f32;
                let z1 = z0 + 1.0;
                let tint = [1.0, 1.0, 1.0];

                let diag_a = [[x0, y0, z0], [x1, y0, z1], [x1, y1, z1], [x0, y1, z0]];
                push_quad(
                    mesh,
                    diag_a,
                    [0.707, 0.0, -0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    false,
                );
                push_quad(
                    mesh,
                    diag_a,
                    [-0.707, 0.0, 0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    true,
                );

                let diag_b = [[x1, y0, z0], [x0, y0, z1], [x0, y1, z1], [x1, y1, z0]];
                push_quad(
                    mesh,
                    diag_b,
                    [-0.707, 0.0, -0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    false,
                );
                push_quad(
                    mesh,
                    diag_b,
                    [0.707, 0.0, 0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    true,
                );
            }
        }
    }
}

fn append_stairs_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                let block = chunk.get(local);
                let Some((_, is_stone)) = stairs_variant(block) else {
                    continue;
                };
                let tile_origin = if is_stone {
                    tile_origin_for_block(BlockId::RUBBLESTONE)
                } else {
                    tile_origin_for_block(HEWN_PLANK_BLOCK_ID)
                };

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;
                let block_coords = [x as i32, y as i32, z as i32];
                emit_box(
                    mesh,
                    light_map,
                    block_coords,
                    [bx, by, bz],
                    [bx + 1.0, by + 0.5, bz + 1.0],
                    tile_origin,
                    false,
                );
            }
        }
    }
}

fn append_lever_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];
    let tex_coords = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let ao_values = [1.0; 4];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);
                if !is_lever(block) {
                    continue;
                }

                let light = f32::from(sample_light_level(
                    light_map,
                    [x as i32, y as i32, z as i32],
                )) / 15.0;
                let light_values = [light; 4];
                let tile_origin = tile_origin_for_block(block);
                let tint = [1.0, 1.0, 1.0];
                let cx = world_offset[0] + x as f32 + 0.5;
                let cy = world_offset[1] + y as f32;
                let cz = world_offset[2] + z as f32 + 0.5;
                let x0 = cx - LEVER_HALF_EXTENT;
                let x1 = cx + LEVER_HALF_EXTENT;
                let y0 = cy;
                let y1 = cy + LEVER_HEIGHT;
                let z0 = cz - LEVER_HALF_EXTENT;
                let z1 = cz + LEVER_HALF_EXTENT;

                let diag_a = [[x0, y0, z0], [x1, y0, z1], [x1, y1, z1], [x0, y1, z0]];
                push_quad(
                    mesh,
                    diag_a,
                    [0.707, 0.0, -0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    false,
                );
                push_quad(
                    mesh,
                    diag_a,
                    [-0.707, 0.0, 0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    true,
                );

                let diag_b = [[x1, y0, z0], [x0, y0, z1], [x0, y1, z1], [x1, y1, z0]];
                push_quad(
                    mesh,
                    diag_b,
                    [-0.707, 0.0, -0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    false,
                );
                push_quad(
                    mesh,
                    diag_b,
                    [0.707, 0.0, 0.707],
                    tex_coords,
                    ao_values,
                    light_values,
                    tile_origin,
                    tint,
                    true,
                );
            }
        }
    }
}

fn append_button_mesh(
    mesh: &mut ChunkMesh,
    chunk: &ChunkData,
    light_map: Option<&LightMap>,
    chunk_pos: ChunkPos,
) {
    let world_offset = [
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    ];

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos { x: x as u8, y: y as u8, z: z as u8 };
                let block = chunk.get(local);
                if !is_button(block) {
                    continue;
                }

                let bx = world_offset[0] + x as f32;
                let by = world_offset[1] + y as f32;
                let bz = world_offset[2] + z as f32;
                let top = by + 1.0;
                let tile_origin = tile_origin_for_block(block);

                emit_box(
                    mesh,
                    light_map,
                    [x as i32, y as i32, z as i32],
                    [
                        bx + 0.5 - BUTTON_HALF_SIZE,
                        top - BUTTON_HEIGHT,
                        bz + 0.5 - BUTTON_HALF_SIZE,
                    ],
                    [
                        bx + 0.5 + BUTTON_HALF_SIZE,
                        top,
                        bz + 0.5 + BUTTON_HALF_SIZE,
                    ],
                    tile_origin,
                    false,
                );
            }
        }
    }
}

fn emit_fence_connection(
    mesh: &mut ChunkMesh,
    light_map: Option<&LightMap>,
    block_coords: [i32; 3],
    world_offset: [f32; 3],
    direction: HorizontalFacing,
    local_y: f32,
    tile_origin: [f32; 2],
) {
    let x = block_coords[0] as f32;
    let y = block_coords[1] as f32;
    let z = block_coords[2] as f32;
    let (min, max) = match direction {
        HorizontalFacing::North => (
            [x + 0.5 - FENCE_BAR_HALF_WIDTH, y + local_y, z],
            [
                x + 0.5 + FENCE_BAR_HALF_WIDTH,
                y + local_y + FENCE_BAR_HEIGHT,
                z + 0.5,
            ],
        ),
        HorizontalFacing::South => (
            [x + 0.5 - FENCE_BAR_HALF_WIDTH, y + local_y, z + 0.5],
            [
                x + 0.5 + FENCE_BAR_HALF_WIDTH,
                y + local_y + FENCE_BAR_HEIGHT,
                z + 1.0,
            ],
        ),
        HorizontalFacing::East => (
            [x + 0.5, y + local_y, z + 0.5 - FENCE_BAR_HALF_WIDTH],
            [
                x + 1.0,
                y + local_y + FENCE_BAR_HEIGHT,
                z + 0.5 + FENCE_BAR_HALF_WIDTH,
            ],
        ),
        HorizontalFacing::West => (
            [x, y + local_y, z + 0.5 - FENCE_BAR_HALF_WIDTH],
            [
                x + 0.5,
                y + local_y + FENCE_BAR_HEIGHT,
                z + 0.5 + FENCE_BAR_HALF_WIDTH,
            ],
        ),
    };

    emit_box(
        mesh,
        light_map,
        block_coords,
        [
            world_offset[0] + min[0],
            world_offset[1] + min[1],
            world_offset[2] + min[2],
        ],
        [
            world_offset[0] + max[0],
            world_offset[1] + max[1],
            world_offset[2] + max[2],
        ],
        tile_origin,
        false,
    );
}

fn emit_ladder_quad(
    mesh: &mut ChunkMesh,
    light_map: Option<&LightMap>,
    block_coords: [i32; 3],
    positions: [[f32; 3]; 4],
    normal: [f32; 3],
    tile_origin: [f32; 2],
) {
    let tex_coords = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let ao_values = [1.0; 4];
    let light = f32::from(sample_light_level(light_map, block_coords)) / 15.0;
    let light_values = [light; 4];
    let tint = [1.0, 1.0, 1.0];
    push_quad(
        mesh,
        positions,
        normal,
        tex_coords,
        ao_values,
        light_values,
        tile_origin,
        tint,
        false,
    );
    push_quad(
        mesh,
        positions,
        [-normal[0], -normal[1], -normal[2]],
        tex_coords,
        ao_values,
        light_values,
        tile_origin,
        tint,
        true,
    );
}

fn emit_cross_mesh(
    mesh: &mut ChunkMesh,
    min: [f32; 3],
    max: [f32; 3],
    light_values: [f32; 4],
    tile_origin: [f32; 2],
    tint: [f32; 3],
) {
    let tex_coords = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let ao_values = [1.0; 4];
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;

    let diag_a = [[x0, y0, z0], [x1, y0, z1], [x1, y1, z1], [x0, y1, z0]];
    push_quad(
        mesh,
        diag_a,
        [0.707, 0.0, -0.707],
        tex_coords,
        ao_values,
        light_values,
        tile_origin,
        tint,
        false,
    );
    push_quad(
        mesh,
        diag_a,
        [-0.707, 0.0, 0.707],
        tex_coords,
        ao_values,
        light_values,
        tile_origin,
        tint,
        true,
    );

    let diag_b = [[x1, y0, z0], [x0, y0, z1], [x0, y1, z1], [x1, y1, z0]];
    push_quad(
        mesh,
        diag_b,
        [-0.707, 0.0, -0.707],
        tex_coords,
        ao_values,
        light_values,
        tile_origin,
        tint,
        false,
    );
    push_quad(
        mesh,
        diag_b,
        [0.707, 0.0, 0.707],
        tex_coords,
        ao_values,
        light_values,
        tile_origin,
        tint,
        true,
    );
}

fn emit_box(
    mesh: &mut ChunkMesh,
    light_map: Option<&LightMap>,
    block_coords: [i32; 3],
    min: [f32; 3],
    max: [f32; 3],
    tile_origin: [f32; 2],
    source_light_boost: bool,
) {
    emit_box_faces(
        mesh,
        light_map,
        block_coords,
        min,
        max,
        tile_origin,
        source_light_boost,
        [true, true, true, true, true, true],
    );
}

fn emit_box_faces(
    mesh: &mut ChunkMesh,
    light_map: Option<&LightMap>,
    block_coords: [i32; 3],
    min: [f32; 3],
    max: [f32; 3],
    tile_origin: [f32; 2],
    source_light_boost: bool,
    faces: [bool; 6], // +X, -X, +Y, -Y, +Z, -Z
) {
    emit_box_faces_tiled(
        mesh,
        light_map,
        block_coords,
        min,
        max,
        [tile_origin; 6],
        source_light_boost,
        faces,
    );
}

fn emit_box_faces_tiled(
    mesh: &mut ChunkMesh,
    light_map: Option<&LightMap>,
    block_coords: [i32; 3],
    min: [f32; 3],
    max: [f32; 3],
    tile_origins: [[f32; 2]; 6],
    source_light_boost: bool,
    faces: [bool; 6], // +X, -X, +Y, -Y, +Z, -Z
) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let source_light = if source_light_boost {
        sample_light_level(light_map, block_coords)
    } else {
        0
    };

    let v000 = [x0, y0, z0];
    let v100 = [x1, y0, z0];
    let v110 = [x1, y1, z0];
    let v010 = [x0, y1, z0];
    let v001 = [x0, y0, z1];
    let v101 = [x1, y0, z1];
    let v111 = [x1, y1, z1];
    let v011 = [x0, y1, z1];

    // +X
    if faces[0] {
        emit_custom_box_face(
            mesh,
            light_map,
            block_coords,
            [v100, v110, v111, v101],
            [1.0, 0.0, 0.0],
            [block_coords[0] + 1, block_coords[1], block_coords[2]],
            tile_origins[0],
            source_light,
        );
    }
    // -X
    if faces[1] {
        emit_custom_box_face(
            mesh,
            light_map,
            block_coords,
            [v000, v001, v011, v010],
            [-1.0, 0.0, 0.0],
            [block_coords[0] - 1, block_coords[1], block_coords[2]],
            tile_origins[1],
            source_light,
        );
    }
    // +Y
    if faces[2] {
        emit_custom_box_face(
            mesh,
            light_map,
            block_coords,
            [v010, v011, v111, v110],
            [0.0, 1.0, 0.0],
            [block_coords[0], block_coords[1] + 1, block_coords[2]],
            tile_origins[2],
            source_light,
        );
    }
    // -Y
    if faces[3] {
        emit_custom_box_face(
            mesh,
            light_map,
            block_coords,
            [v000, v100, v101, v001],
            [0.0, -1.0, 0.0],
            [block_coords[0], block_coords[1] - 1, block_coords[2]],
            tile_origins[3],
            source_light,
        );
    }
    // +Z
    if faces[4] {
        emit_custom_box_face(
            mesh,
            light_map,
            block_coords,
            [v001, v101, v111, v011],
            [0.0, 0.0, 1.0],
            [block_coords[0], block_coords[1], block_coords[2] + 1],
            tile_origins[4],
            source_light,
        );
    }
    // -Z
    if faces[5] {
        emit_custom_box_face(
            mesh,
            light_map,
            block_coords,
            [v000, v010, v110, v100],
            [0.0, 0.0, -1.0],
            [block_coords[0], block_coords[1], block_coords[2] - 1],
            tile_origins[5],
            source_light,
        );
    }
}

fn emit_custom_box_face(
    mesh: &mut ChunkMesh,
    light_map: Option<&LightMap>,
    _block_coords: [i32; 3],
    positions: [[f32; 3]; 4],
    normal: [f32; 3],
    sample_coords: [i32; 3],
    tile_origin: [f32; 2],
    source_light: u8,
) {
    let tex_coords = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let ao_values = [1.0; 4];
    let tint = [1.0, 1.0, 1.0];
    let light_values = box_face_light(light_map, source_light, sample_coords);
    push_quad(
        mesh,
        positions,
        normal,
        tex_coords,
        ao_values,
        light_values,
        tile_origin,
        tint,
        false,
    );
}

fn box_face_light(
    light_map: Option<&LightMap>,
    source_light: u8,
    sample_coords: [i32; 3],
) -> [f32; 4] {
    let sampled = face_light_level(light_map, sample_coords).max(source_light);
    let normalized = f32::from(sampled) / 15.0;
    [normalized; 4]
}

fn door_variant(block: BlockId) -> Option<(HorizontalFacing, bool)> {
    match block {
        BlockId::DOOR_LOWER | BlockId::DOOR_UPPER => Some((HorizontalFacing::North, false)),
        BlockId::DOOR_LOWER_EAST | BlockId::DOOR_UPPER_EAST => {
            Some((HorizontalFacing::East, false))
        }
        BlockId::DOOR_LOWER_SOUTH | BlockId::DOOR_UPPER_SOUTH => {
            Some((HorizontalFacing::South, false))
        }
        BlockId::DOOR_LOWER_WEST | BlockId::DOOR_UPPER_WEST => {
            Some((HorizontalFacing::West, false))
        }
        BlockId::DOOR_LOWER_OPEN | BlockId::DOOR_UPPER_OPEN => {
            Some((HorizontalFacing::North, true))
        }
        BlockId::DOOR_LOWER_OPEN_EAST | BlockId::DOOR_UPPER_OPEN_EAST => {
            Some((HorizontalFacing::East, true))
        }
        BlockId::DOOR_LOWER_OPEN_SOUTH | BlockId::DOOR_UPPER_OPEN_SOUTH => {
            Some((HorizontalFacing::South, true))
        }
        BlockId::DOOR_LOWER_OPEN_WEST | BlockId::DOOR_UPPER_OPEN_WEST => {
            Some((HorizontalFacing::West, true))
        }
        _ => None,
    }
}

fn door_bounds(facing: HorizontalFacing, is_open: bool) -> ([f32; 3], [f32; 3]) {
    match (facing, is_open) {
        (HorizontalFacing::North, false) => ([0.0, 0.0, 1.0 - DOOR_THICKNESS], [1.0, 1.0, 1.0]),
        (HorizontalFacing::East, false) => ([0.0, 0.0, 0.0], [DOOR_THICKNESS, 1.0, 1.0]),
        (HorizontalFacing::South, false) => ([0.0, 0.0, 0.0], [1.0, 1.0, DOOR_THICKNESS]),
        (HorizontalFacing::West, false) => ([1.0 - DOOR_THICKNESS, 0.0, 0.0], [1.0, 1.0, 1.0]),
        (HorizontalFacing::North, true) => ([1.0 - DOOR_THICKNESS, 0.0, 0.0], [1.0, 1.0, 1.0]),
        (HorizontalFacing::East, true) => ([0.0, 0.0, 0.0], [1.0, 1.0, DOOR_THICKNESS]),
        (HorizontalFacing::South, true) => ([0.0, 0.0, 0.0], [DOOR_THICKNESS, 1.0, 1.0]),
        (HorizontalFacing::West, true) => ([0.0, 0.0, 1.0 - DOOR_THICKNESS], [1.0, 1.0, 1.0]),
    }
}

fn ladder_facing(block: BlockId) -> Option<HorizontalFacing> {
    match block {
        BlockId::LADDER => Some(HorizontalFacing::North),
        BlockId::LADDER_EAST => Some(HorizontalFacing::East),
        BlockId::LADDER_SOUTH => Some(HorizontalFacing::South),
        BlockId::LADDER_WEST => Some(HorizontalFacing::West),
        _ => None,
    }
}

fn vine_facing(block: BlockId) -> Option<HorizontalFacing> {
    match block {
        BlockId::VINE_NORTH => Some(HorizontalFacing::North),
        BlockId::VINE_EAST => Some(HorizontalFacing::East),
        BlockId::VINE_SOUTH => Some(HorizontalFacing::South),
        BlockId::VINE_WEST => Some(HorizontalFacing::West),
        _ => None,
    }
}

fn stairs_variant(block: BlockId) -> Option<(HorizontalFacing, bool)> {
    match block {
        BlockId::STONE_STAIRS_NORTH => Some((HorizontalFacing::North, true)),
        BlockId::STONE_STAIRS_EAST => Some((HorizontalFacing::East, true)),
        BlockId::STONE_STAIRS_SOUTH => Some((HorizontalFacing::South, true)),
        BlockId::STONE_STAIRS_WEST => Some((HorizontalFacing::West, true)),
        BlockId::WOODEN_STAIRS_NORTH => Some((HorizontalFacing::North, false)),
        BlockId::WOODEN_STAIRS_EAST => Some((HorizontalFacing::East, false)),
        BlockId::WOODEN_STAIRS_SOUTH => Some((HorizontalFacing::South, false)),
        BlockId::WOODEN_STAIRS_WEST => Some((HorizontalFacing::West, false)),
        _ => None,
    }
}

fn tile_origin_for_block(block_id: BlockId) -> [f32; 2] {
    let tile_x = f32::from(block_id.0 % ATLAS_TILES_PER_ROW);
    let tile_y = f32::from(block_id.0 / ATLAS_TILES_PER_ROW);
    [
        (tile_x * ATLAS_TILE_SIZE) / ATLAS_SIZE,
        (tile_y * ATLAS_TILE_SIZE) / ATLAS_SIZE,
    ]
}

fn carpet_tile_origin(block_id: BlockId) -> [f32; 2] {
    let color_index = u16::from(wool_color_index(block_id).unwrap_or(0));
    tile_origin_for_block(BlockId(BlockId::WOOL_WHITE.0 + color_index))
}

fn push_quad(
    mesh: &mut ChunkMesh,
    positions: [[f32; 3]; 4],
    normal: [f32; 3],
    tex_coords: [[f32; 2]; 4],
    ao_values: [f32; 4],
    light_values: [f32; 4],
    tile_origin: [f32; 2],
    tint: [f32; 3],
    reverse_winding: bool,
) {
    push_quad_lit(
        mesh,
        positions,
        normal,
        tex_coords,
        ao_values,
        light_values,
        [0.0; 4],
        tile_origin,
        tint,
        reverse_winding,
    );
}

fn push_quad_lit(
    mesh: &mut ChunkMesh,
    positions: [[f32; 3]; 4],
    normal: [f32; 3],
    tex_coords: [[f32; 2]; 4],
    ao_values: [f32; 4],
    light_values: [f32; 4],
    emissive_light_values: [f32; 4],
    tile_origin: [f32; 2],
    tint: [f32; 3],
    reverse_winding: bool,
) {
    let base_index = mesh.vertices.len() as u32;
    for i in 0..4 {
        mesh.vertices.push(ChunkVertex {
            position: positions[i],
            normal,
            tex_coord: tex_coords[i],
            ao: ao_values[i],
            light: light_values[i],
            emissive_light: emissive_light_values[i],
            tile_origin,
            color: tint,
        });
    }

    let indices = if reverse_winding {
        [
            base_index,
            base_index + 2,
            base_index + 1,
            base_index,
            base_index + 3,
            base_index + 2,
        ]
    } else {
        [
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]
    };
    mesh.indices.extend_from_slice(&indices);
}

fn compute_quad_ao(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: &ChunkNeighbors<'_>,
    face: FaceSpec,
    slice: usize,
    u: usize,
    v: usize,
    width: usize,
    height: usize,
) -> [f32; 4] {
    let corners = [
        (u as i32, v as i32, -1, -1),
        ((u + width - 1) as i32, v as i32, 1, -1),
        ((u + width - 1) as i32, (v + height - 1) as i32, 1, 1),
        (u as i32, (v + height - 1) as i32, -1, 1),
    ];

    let mut ao = [1.0f32; 4];
    for (i, (cu, cv, su, sv)) in corners.into_iter().enumerate() {
        let mut base = [0i32; 3];
        base[face.axis] = slice as i32 + face.sign;
        base[face.u_axis] = cu;
        base[face.v_axis] = cv;

        let mut side_u = base;
        side_u[face.u_axis] += su;

        let mut side_v = base;
        side_v[face.v_axis] += sv;

        let mut corner = base;
        corner[face.u_axis] += su;
        corner[face.v_axis] += sv;

        let side_u_solid = is_solid(sample_block(chunk, neighbors, side_u), registry) as u8;
        let side_v_solid = is_solid(sample_block(chunk, neighbors, side_v), registry) as u8;
        let corner_solid = is_solid(sample_block(chunk, neighbors, corner), registry) as u8;
        let count = side_u_solid + side_v_solid + corner_solid;

        ao[i] = 1.0 - 0.2 * f32::from(count);
    }

    ao
}

fn compute_block_ao_fingerprint(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: &ChunkNeighbors<'_>,
    face: FaceSpec,
    slice: usize,
    u: usize,
    v: usize,
) -> u8 {
    let ao_values = compute_quad_ao(chunk, registry, neighbors, face, slice, u, v, 1, 1);
    let mut packed = 0u8;
    for (i, ao_value) in ao_values.into_iter().enumerate() {
        let level = (((1.0 - ao_value) * 5.0).round() as i32).clamp(0, 4) as u8;
        packed |= level << (i * 2);
    }
    packed
}

fn face_light_level(light_map: Option<&LightMap>, block_coords: [i32; 3]) -> u8 {
    sample_light_level(light_map, block_coords)
}

fn compute_quad_light_values(
    light_map: Option<&LightMap>,
    face: FaceSpec,
    slice: usize,
    u: usize,
    v: usize,
    width: usize,
    height: usize,
) -> [f32; 4] {
    let corners = [
        (u as i32, v as i32),
        ((u + width) as i32, v as i32),
        ((u + width) as i32, (v + height) as i32),
        (u as i32, (v + height) as i32),
    ];

    let mut lights = [1.0f32; 4];
    for (i, (corner_u, corner_v)) in corners.into_iter().enumerate() {
        let mut accumulated = 0u32;
        for du in [0i32, -1] {
            for dv in [0i32, -1] {
                let mut sample_coords = [0i32; 3];
                sample_coords[face.axis] = slice as i32 + face.sign;
                sample_coords[face.u_axis] = corner_u + du;
                sample_coords[face.v_axis] = corner_v + dv;
                accumulated += u32::from(sample_light_level(light_map, sample_coords));
            }
        }
        lights[i] = (accumulated as f32 / 4.0) / 15.0;
    }

    lights
}

fn compute_quad_emissive_light_values(
    emissive_light_map: Option<&LightMap>,
    face: FaceSpec,
    slice: usize,
    u: usize,
    v: usize,
    width: usize,
    height: usize,
) -> [f32; 4] {
    let corners = [
        (u as i32, v as i32),
        ((u + width) as i32, v as i32),
        ((u + width) as i32, (v + height) as i32),
        (u as i32, (v + height) as i32),
    ];

    let mut lights = [0.0f32; 4];
    for (i, (corner_u, corner_v)) in corners.into_iter().enumerate() {
        let mut accumulated = 0u32;
        for du in [0i32, -1] {
            for dv in [0i32, -1] {
                let mut sample_coords = [0i32; 3];
                sample_coords[face.axis] = slice as i32 + face.sign;
                sample_coords[face.u_axis] = corner_u + du;
                sample_coords[face.v_axis] = corner_v + dv;
                accumulated += u32::from(sample_emissive_light_level(
                    emissive_light_map,
                    sample_coords,
                ));
            }
        }
        lights[i] = (accumulated as f32 / 4.0) / 15.0;
    }

    lights
}

fn sample_light_level(light_map: Option<&LightMap>, block_coords: [i32; 3]) -> u8 {
    let Some(light_map) = light_map else {
        return 15;
    };
    let [x, y, z] = block_coords;
    light_map.get_i32(x, y, z)
}

fn sample_emissive_light_level(
    emissive_light_map: Option<&LightMap>,
    block_coords: [i32; 3],
) -> u8 {
    let Some(emissive_light_map) = emissive_light_map else {
        return 0;
    };
    let [x, y, z] = block_coords;
    emissive_light_map.get_i32_with_default(x, y, z, 0)
}

fn face_block_coords(face: FaceSpec, slice: usize, u: usize, v: usize) -> [i32; 3] {
    let mut coords = [0i32; 3];
    coords[face.axis] = slice as i32;
    coords[face.u_axis] = u as i32;
    coords[face.v_axis] = v as i32;
    coords
}

fn is_solid(block: BlockId, registry: &BlockRegistry) -> bool {
    block != BlockId::AIR && registry.get_properties(block).solid && !uses_custom_mesh(block)
}

fn uses_custom_mesh(block: BlockId) -> bool {
    block == BlockId::TORCH
        || block == BlockId::WOODEN_DOOR
        || (BlockId::DOOR_LOWER.0..=BlockId::DOOR_UPPER_OPEN_WEST.0).contains(&block.0)
        || (BlockId::LADDER.0..=BlockId::LADDER_WEST.0).contains(&block.0)
        || is_vine(block)
        || block == BlockId::FENCE
        || is_trapdoor_block(block)
        || is_bed_block(block)
        || block == BlockId::STONE_PRESSURE_PLATE
        || is_carpet(block)
        || is_slab_block(block)
        || is_cactus(block)
        || is_cross_mesh_block(block)
        || is_glass_pane(block)
        || is_lever(block)
        || is_button(block)
        || is_stairs(block)
}

fn is_cross_mesh_block(block: BlockId) -> bool {
    block == BlockId::TALL_GRASS
        || block == BlockId::WILDFLOWER
        || block == BlockId::SAPLING
        || block == BlockId::SUGAR_CANE
        || is_cobweb(block)
        || is_mushroom(block)
        || is_wheat_block(block)
        || is_fire_block(block)
}

fn cross_mesh_height(block: BlockId) -> f32 {
    if is_mushroom(block) {
        MUSHROOM_HEIGHT
    } else {
        1.0
    }
}

fn sample_block(chunk: &ChunkData, neighbors: &ChunkNeighbors<'_>, coords: [i32; 3]) -> BlockId {
    let x = coords[0];
    let y = coords[1];
    let z = coords[2];

    if in_chunk_bounds(x) && in_chunk_bounds(y) && in_chunk_bounds(z) {
        return sample_chunk_local(chunk, x, y, z);
    }

    let x_out = axis_out(x);
    let y_out = axis_out(y);
    let z_out = axis_out(z);

    let neighbor = match (x_out, y_out, z_out) {
        (-1, 0, 0) => neighbors.neg_x,
        (1, 0, 0) => neighbors.pos_x,
        (0, -1, 0) => neighbors.neg_y,
        (0, 1, 0) => neighbors.pos_y,
        (0, 0, -1) => neighbors.neg_z,
        (0, 0, 1) => neighbors.pos_z,
        _ => None,
    };

    if let Some(neighbor_chunk) = neighbor {
        let nx = wrap_to_local(x);
        let ny = wrap_to_local(y);
        let nz = wrap_to_local(z);
        sample_chunk_local(neighbor_chunk, nx, ny, nz)
    } else {
        BlockId::AIR
    }
}

fn sample_chunk_local(chunk: &ChunkData, x: i32, y: i32, z: i32) -> BlockId {
    let local = LocalPos {
        x: x as u8,
        y: y as u8,
        z: z as u8,
    };
    chunk.blocks[local_to_index(local)]
}

fn in_chunk_bounds(value: i32) -> bool {
    (0..CHUNK_SIZE_I32).contains(&value)
}

fn axis_out(value: i32) -> i8 {
    if value < 0 {
        -1
    } else if value >= CHUNK_SIZE_I32 {
        1
    } else {
        0
    }
}

fn wrap_to_local(value: i32) -> i32 {
    if value < 0 {
        value + CHUNK_SIZE_I32
    } else if value >= CHUNK_SIZE_I32 {
        value - CHUNK_SIZE_I32
    } else {
        value
    }
}
