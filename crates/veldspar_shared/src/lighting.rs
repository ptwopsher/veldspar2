use std::collections::{HashMap, VecDeque};

use crate::block::{BlockId, BlockRegistry};
use crate::chunk::ChunkData;
use crate::coords::{local_to_index, LocalPos, CHUNK_SIZE};

const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
const MAX_LIGHT_LEVEL: u8 = 15;
const EXTENDED_UNSET: u8 = u8::MAX;
const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;
const EXTENDED_SIZE: usize = CHUNK_SIZE + 2;
const EXTENDED_VOLUME: usize = EXTENDED_SIZE * EXTENDED_SIZE * EXTENDED_SIZE;
const NEIGHBOR_OFFSETS: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

#[derive(Clone, Debug)]
pub struct LightMap {
    levels: [u8; CHUNK_VOLUME],
    extended_levels: Option<Box<[u8; EXTENDED_VOLUME]>>,
}

impl LightMap {
    pub fn new() -> Self {
        Self {
            levels: [0; CHUNK_VOLUME],
            extended_levels: None,
        }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        self.levels[index_from_xyz(x, y, z)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, level: u8) {
        let clamped = level.min(MAX_LIGHT_LEVEL);
        let index = index_from_xyz(x, y, z);
        self.levels[index] = clamped;
    }

    pub fn get_i32(&self, x: i32, y: i32, z: i32) -> u8 {
        if is_in_chunk_bounds_i32(x, y, z) {
            return self.get(x as usize, y as usize, z as usize);
        }
        let Some(extended_levels) = self.extended_levels.as_ref() else {
            return MAX_LIGHT_LEVEL;
        };
        if !is_in_extended_sample_bounds(x, y, z) {
            return MAX_LIGHT_LEVEL;
        }
        let value = extended_levels[extended_index_from_xyz(x, y, z)];
        if value == EXTENDED_UNSET {
            MAX_LIGHT_LEVEL
        } else {
            value.min(MAX_LIGHT_LEVEL)
        }
    }

    pub fn get_i32_with_default(&self, x: i32, y: i32, z: i32, default: u8) -> u8 {
        if is_in_chunk_bounds_i32(x, y, z) {
            return self.get(x as usize, y as usize, z as usize);
        }
        let Some(extended_levels) = self.extended_levels.as_ref() else {
            return default.min(MAX_LIGHT_LEVEL);
        };
        if !is_in_extended_sample_bounds(x, y, z) {
            return default.min(MAX_LIGHT_LEVEL);
        }
        let value = extended_levels[extended_index_from_xyz(x, y, z)];
        if value == EXTENDED_UNSET {
            default.min(MAX_LIGHT_LEVEL)
        } else {
            value.min(MAX_LIGHT_LEVEL)
        }
    }

    pub fn set_extended(&mut self, x: i32, y: i32, z: i32, level: u8) {
        if !is_in_extended_sample_bounds(x, y, z) {
            return;
        }
        let clamped = level.min(MAX_LIGHT_LEVEL);
        if clamped == 0 {
            return;
        }
        if self.extended_levels.is_none() {
            self.extended_levels = Some(Box::new([EXTENDED_UNSET; EXTENDED_VOLUME]));
        }
        let extended_levels = self
            .extended_levels
            .as_mut()
            .expect("extended light map should be initialized");
        let idx = extended_index_from_xyz(x, y, z);
        if extended_levels[idx] == EXTENDED_UNSET || clamped > extended_levels[idx] {
            extended_levels[idx] = clamped;
        }
    }
}

impl Default for LightMap {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute_sunlight(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    above_chunk: Option<&ChunkData>,
) -> LightMap {
    let mut light_map = LightMap::new();

    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let mut light = initial_column_light(x, z, above_chunk, registry);

            for y in (0..CHUNK_SIZE).rev() {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = chunk.get(local);

                light = next_vertical_light(light, block, registry);
                light_map.set(x, y, z, light);
            }
        }
    }

    light_map
}

pub fn propagate_light(light_map: &mut LightMap, chunk: &ChunkData, registry: &BlockRegistry) {
    propagate_light_with_neighbors(light_map, chunk, registry, [None; 6]);
}

pub fn propagate_light_with_neighbors(
    light_map: &mut LightMap,
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: [Option<&ChunkData>; 6],
) {
    let mut queue = VecDeque::new();
    let mut extended_light = HashMap::<(i32, i32, i32), u8>::new();

    seed_emissive_sources(
        light_map,
        &mut extended_light,
        &mut queue,
        chunk,
        registry,
        (0, 0, 0),
    );
    if let Some(pos_x) = neighbors[0] {
        seed_emissive_sources(
            light_map,
            &mut extended_light,
            &mut queue,
            pos_x,
            registry,
            (CHUNK_SIZE_I32, 0, 0),
        );
    }
    if let Some(neg_x) = neighbors[1] {
        seed_emissive_sources(
            light_map,
            &mut extended_light,
            &mut queue,
            neg_x,
            registry,
            (-CHUNK_SIZE_I32, 0, 0),
        );
    }
    if let Some(pos_y) = neighbors[2] {
        seed_emissive_sources(
            light_map,
            &mut extended_light,
            &mut queue,
            pos_y,
            registry,
            (0, CHUNK_SIZE_I32, 0),
        );
    }
    if let Some(neg_y) = neighbors[3] {
        seed_emissive_sources(
            light_map,
            &mut extended_light,
            &mut queue,
            neg_y,
            registry,
            (0, -CHUNK_SIZE_I32, 0),
        );
    }
    if let Some(pos_z) = neighbors[4] {
        seed_emissive_sources(
            light_map,
            &mut extended_light,
            &mut queue,
            pos_z,
            registry,
            (0, 0, CHUNK_SIZE_I32),
        );
    }
    if let Some(neg_z) = neighbors[5] {
        seed_emissive_sources(
            light_map,
            &mut extended_light,
            &mut queue,
            neg_z,
            registry,
            (0, 0, -CHUNK_SIZE_I32),
        );
    }

    while let Some((x, y, z)) = queue.pop_front() {
        let source_light = get_light_level(light_map, &extended_light, x, y, z);
        if source_light <= 1 {
            continue;
        }

        let spread_light = source_light - 1;

        for (dx, dy, dz) in NEIGHBOR_OFFSETS {
            let nx = x + dx;
            let ny = y + dy;
            let nz = z + dz;
            if !is_in_extended_bounds(nx, ny, nz) {
                continue;
            }
            if !is_light_passable_with_neighbors(chunk, registry, neighbors, nx, ny, nz) {
                continue;
            }

            if try_set_light_level(
                light_map,
                &mut extended_light,
                nx,
                ny,
                nz,
                spread_light,
            ) {
                if spread_light > 1 {
                    queue.push_back((nx, ny, nz));
                }
            }
        }
    }

    for ((x, y, z), level) in extended_light {
        if !is_in_chunk_bounds_i32(x, y, z) {
            light_map.set_extended(x, y, z, level);
        }
    }
}

pub fn compute_chunk_lighting(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    above: Option<&ChunkData>,
) -> LightMap {
    compute_chunk_lighting_with_neighbors(chunk, registry, [None, None, above, None, None, None])
}

pub fn compute_chunk_lighting_with_neighbors(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: [Option<&ChunkData>; 6],
) -> LightMap {
    let mut light_map = compute_sunlight(chunk, registry, neighbors[2]);
    propagate_light_with_neighbors(&mut light_map, chunk, registry, neighbors);
    light_map
}

fn initial_column_light(
    x: usize,
    z: usize,
    above_chunk: Option<&ChunkData>,
    registry: &BlockRegistry,
) -> u8 {
    match above_chunk {
        Some(above) => {
            let above_local = LocalPos {
                x: x as u8,
                y: 0,
                z: z as u8,
            };
            let block_above = above.get(above_local);

            if !registry.get_properties(block_above).solid {
                MAX_LIGHT_LEVEL
            } else {
                0
            }
        }
        None => MAX_LIGHT_LEVEL,
    }
}

fn next_vertical_light(current: u8, block: BlockId, registry: &BlockRegistry) -> u8 {
    let props = registry.get_properties(block);
    if block == BlockId::AIR || !props.solid {
        current
    } else if props.transparent {
        // Transparent solid blocks (leaves, ice, water) filter light by 1 level
        current.saturating_sub(1)
    } else {
        0
    }
}

fn is_light_passable_with_neighbors(
    chunk: &ChunkData,
    registry: &BlockRegistry,
    neighbors: [Option<&ChunkData>; 6],
    x: i32,
    y: i32,
    z: i32,
) -> bool {
    let block = sample_block_with_neighbors(chunk, neighbors, x, y, z);
    let props = registry.get_properties(block);
    !props.solid || props.transparent
}

fn emitted_light_level(block: BlockId, registry: &BlockRegistry) -> u8 {
    if block == BlockId::AIR {
        0
    } else {
        registry.get_properties(block).light_level.min(MAX_LIGHT_LEVEL)
    }
}

fn seed_emissive_sources(
    light_map: &mut LightMap,
    extended_light: &mut HashMap<(i32, i32, i32), u8>,
    queue: &mut VecDeque<(i32, i32, i32)>,
    source_chunk: &ChunkData,
    registry: &BlockRegistry,
    offset: (i32, i32, i32),
) {
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos {
                    x: x as u8,
                    y: y as u8,
                    z: z as u8,
                };
                let block = source_chunk.get(local);
                let emission = emitted_light_level(block, registry);
                if emission == 0 {
                    continue;
                }

                let world_x = x as i32 + offset.0;
                let world_y = y as i32 + offset.1;
                let world_z = z as i32 + offset.2;

                if !is_in_extended_bounds(world_x, world_y, world_z)
                    || !can_source_affect_chunk(world_x, world_y, world_z, emission)
                {
                    continue;
                }

                if try_set_light_level(
                    light_map,
                    extended_light,
                    world_x,
                    world_y,
                    world_z,
                    emission,
                ) && emission > 1
                {
                    queue.push_back((world_x, world_y, world_z));
                }
            }
        }
    }
}

fn can_source_affect_chunk(x: i32, y: i32, z: i32, emission: u8) -> bool {
    if emission <= 1 {
        return false;
    }
    distance_to_chunk(x, y, z) <= i32::from(emission - 1)
}

fn distance_to_chunk(x: i32, y: i32, z: i32) -> i32 {
    let max_coord = CHUNK_SIZE_I32 - 1;
    let dx = if x < 0 {
        -x
    } else if x > max_coord {
        x - max_coord
    } else {
        0
    };
    let dy = if y < 0 {
        -y
    } else if y > max_coord {
        y - max_coord
    } else {
        0
    };
    let dz = if z < 0 {
        -z
    } else if z > max_coord {
        z - max_coord
    } else {
        0
    };

    dx + dy + dz
}

fn get_light_level(
    light_map: &LightMap,
    extended_light: &HashMap<(i32, i32, i32), u8>,
    x: i32,
    y: i32,
    z: i32,
) -> u8 {
    if is_in_chunk_bounds_i32(x, y, z) {
        light_map.get(x as usize, y as usize, z as usize)
    } else {
        extended_light.get(&(x, y, z)).copied().unwrap_or(0)
    }
}

fn try_set_light_level(
    light_map: &mut LightMap,
    extended_light: &mut HashMap<(i32, i32, i32), u8>,
    x: i32,
    y: i32,
    z: i32,
    level: u8,
) -> bool {
    let clamped = level.min(MAX_LIGHT_LEVEL);
    if clamped == 0 {
        return false;
    }

    if is_in_chunk_bounds_i32(x, y, z) {
        let existing = light_map.get(x as usize, y as usize, z as usize);
        if clamped > existing {
            light_map.set(x as usize, y as usize, z as usize, clamped);
            true
        } else {
            false
        }
    } else {
        let entry = extended_light.entry((x, y, z)).or_insert(0);
        if clamped > *entry {
            *entry = clamped;
            true
        } else {
            false
        }
    }
}

fn sample_block_with_neighbors(
    chunk: &ChunkData,
    neighbors: [Option<&ChunkData>; 6],
    x: i32,
    y: i32,
    z: i32,
) -> BlockId {
    if is_in_chunk_bounds_i32(x, y, z) {
        let local = LocalPos {
            x: x as u8,
            y: y as u8,
            z: z as u8,
        };
        return chunk.get(local);
    }

    let x_out = axis_out(x);
    let y_out = axis_out(y);
    let z_out = axis_out(z);

    let neighbor = match (x_out, y_out, z_out) {
        (-1, 0, 0) => neighbors[1],
        (1, 0, 0) => neighbors[0],
        (0, -1, 0) => neighbors[3],
        (0, 1, 0) => neighbors[2],
        (0, 0, -1) => neighbors[5],
        (0, 0, 1) => neighbors[4],
        _ => None,
    };

    let Some(neighbor_chunk) = neighbor else {
        return BlockId::AIR;
    };

    let nx = wrap_to_local(x);
    let ny = wrap_to_local(y);
    let nz = wrap_to_local(z);

    let local = LocalPos {
        x: nx as u8,
        y: ny as u8,
        z: nz as u8,
    };
    neighbor_chunk.get(local)
}

fn is_in_chunk_bounds_i32(x: i32, y: i32, z: i32) -> bool {
    in_chunk_bounds_i32(x) && in_chunk_bounds_i32(y) && in_chunk_bounds_i32(z)
}

fn in_chunk_bounds_i32(value: i32) -> bool {
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

fn is_in_extended_bounds(x: i32, y: i32, z: i32) -> bool {
    in_extended_bound(x) && in_extended_bound(y) && in_extended_bound(z)
}

fn in_extended_bound(value: i32) -> bool {
    (-CHUNK_SIZE_I32..(CHUNK_SIZE_I32 * 2)).contains(&value)
}

fn index_from_xyz(x: usize, y: usize, z: usize) -> usize {
    assert!(
        x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE,
        "light position out of bounds: ({x}, {y}, {z})"
    );

    local_to_index(LocalPos {
        x: x as u8,
        y: y as u8,
        z: z as u8,
    })
}

fn is_in_extended_sample_bounds(x: i32, y: i32, z: i32) -> bool {
    (-1..=CHUNK_SIZE_I32).contains(&x)
        && (-1..=CHUNK_SIZE_I32).contains(&y)
        && (-1..=CHUNK_SIZE_I32).contains(&z)
}

fn extended_index_from_xyz(x: i32, y: i32, z: i32) -> usize {
    let ex = (x + 1) as usize;
    let ey = (y + 1) as usize;
    let ez = (z + 1) as usize;
    ex + ez * EXTENDED_SIZE + ey * EXTENDED_SIZE * EXTENDED_SIZE
}

#[cfg(test)]
mod tests {
    use super::{compute_chunk_lighting_with_neighbors, MAX_LIGHT_LEVEL};
    use crate::block::register_default_blocks;
    use crate::chunk::ChunkData;
    use crate::coords::{LocalPos, CHUNK_SIZE};

    #[test]
    fn sunlight_propagates_down_until_solid_block() {
        let registry = register_default_blocks();
        let mut chunk = ChunkData::new_empty();
        let granite = registry.get_by_name("granite").expect("granite should exist");

        chunk.set(LocalPos { x: 3, y: 20, z: 7 }, granite);

        let light = compute_chunk_lighting_with_neighbors(&chunk, &registry, [None; 6]);
        assert_eq!(light.get(3, CHUNK_SIZE - 1, 7), MAX_LIGHT_LEVEL);
        assert_eq!(light.get(3, 21, 7), MAX_LIGHT_LEVEL);
        assert_eq!(light.get(3, 20, 7), 0);
        assert_eq!(light.get(3, 19, 7), 0);
    }

    #[test]
    fn torch_light_falls_off_with_manhattan_distance() {
        let registry = register_default_blocks();
        let mut chunk = ChunkData::new_empty();
        let granite = registry.get_by_name("granite").expect("granite should exist");
        let torch = registry.get_by_name("torch").expect("torch should exist");

        // Roof blocks skylight so the test only sees torch light.
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                chunk.set(
                    LocalPos {
                        x: x as u8,
                        y: (CHUNK_SIZE - 1) as u8,
                        z: z as u8,
                    },
                    granite,
                );
            }
        }

        chunk.set(LocalPos { x: 10, y: 10, z: 10 }, torch);

        let light = compute_chunk_lighting_with_neighbors(&chunk, &registry, [None; 6]);
        assert_eq!(light.get(10, 10, 10), 14);
        assert_eq!(light.get(11, 10, 10), 13);
        assert_eq!(light.get(12, 10, 10), 12);
        assert_eq!(light.get(25, 10, 10), 0);
    }

    #[test]
    fn solid_wall_stops_torch_light() {
        let registry = register_default_blocks();
        let mut chunk = ChunkData::new_empty();
        let granite = registry.get_by_name("granite").expect("granite should exist");
        let torch = registry.get_by_name("torch").expect("torch should exist");

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                chunk.set(
                    LocalPos {
                        x: x as u8,
                        y: (CHUNK_SIZE - 1) as u8,
                        z: z as u8,
                    },
                    granite,
                );
            }
        }

        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                chunk.set(
                    LocalPos {
                        x: 17,
                        y: y as u8,
                        z: z as u8,
                    },
                    granite,
                );
            }
        }

        chunk.set(LocalPos { x: 16, y: 10, z: 10 }, torch);

        let light = compute_chunk_lighting_with_neighbors(&chunk, &registry, [None; 6]);
        assert_eq!(light.get(18, 10, 10), 0);
    }

    #[test]
    fn neighbor_torch_lights_chunk_boundary() {
        let registry = register_default_blocks();
        let granite = registry.get_by_name("granite").expect("granite should exist");
        let torch = registry.get_by_name("torch").expect("torch should exist");

        let mut center = ChunkData::new_empty();
        let mut pos_x = ChunkData::new_empty();

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                center.set(
                    LocalPos {
                        x: x as u8,
                        y: (CHUNK_SIZE - 1) as u8,
                        z: z as u8,
                    },
                    granite,
                );
                pos_x.set(
                    LocalPos {
                        x: x as u8,
                        y: (CHUNK_SIZE - 1) as u8,
                        z: z as u8,
                    },
                    granite,
                );
            }
        }

        pos_x.set(LocalPos { x: 0, y: 10, z: 10 }, torch);

        let light = compute_chunk_lighting_with_neighbors(
            &center,
            &registry,
            [Some(&pos_x), None, None, None, None, None],
        );
        assert_eq!(light.get(CHUNK_SIZE - 1, 10, 10), 13);
    }
}
