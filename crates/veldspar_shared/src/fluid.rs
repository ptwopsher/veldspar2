use std::collections::{HashMap, HashSet, VecDeque};

use glam::IVec3;

use crate::block::{
    is_lava_block, is_lava_source_block, is_water_block, is_water_source_block,
    lava_flow_block_from_level, water_flow_block_from_level, BlockId, MAX_WATER_FLOW_LEVEL,
};
use crate::chunk::ChunkData;
use crate::coords::{chunk_to_world, index_to_local, local_to_index, world_to_chunk, ChunkPos};

const FLUID_HORIZONTAL_DIRECTIONS: [IVec3; 4] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];
const FLUID_DOWN_DIRECTION: IVec3 = IVec3::new(0, -1, 0);
const NEIGHBOR_DIRECTIONS: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];
const MAX_LAVA_HORIZONTAL_FLOW_LEVEL: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FluidChange {
    pub world_pos: IVec3,
    pub new_block: BlockId,
}

pub type WaterChange = FluidChange;
pub type LavaChange = FluidChange;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FluidTarget {
    Source,
    Flow(u8),
}

#[derive(Clone, Copy)]
struct FluidConfig {
    source_block: BlockId,
    is_source_block: fn(BlockId) -> bool,
    is_fluid_block: fn(BlockId) -> bool,
    flow_block_from_level: fn(u8) -> BlockId,
    max_horizontal_level: u8,
    down_flow_level: u8,
    can_occupy: fn(BlockId) -> bool,
}

const WATER_CONFIG: FluidConfig = FluidConfig {
    source_block: BlockId::STILL_WATER,
    is_source_block: is_water_source_block,
    is_fluid_block: is_water_block,
    flow_block_from_level: water_flow_block_from_level,
    max_horizontal_level: MAX_WATER_FLOW_LEVEL,
    down_flow_level: 0,
    can_occupy: can_water_occupy,
};

const LAVA_CONFIG: FluidConfig = FluidConfig {
    source_block: BlockId::LAVA_SOURCE,
    is_source_block: is_lava_source_block,
    is_fluid_block: is_lava_block,
    flow_block_from_level: lava_flow_block_from_level,
    max_horizontal_level: MAX_LAVA_HORIZONTAL_FLOW_LEVEL,
    down_flow_level: 0,
    can_occupy: can_lava_occupy,
};

pub fn simulate_water(chunks: &mut HashMap<ChunkPos, ChunkData>) -> Vec<WaterChange> {
    simulate_water_near(chunks, None)
}

/// Simulate water flow, optionally limited to chunks near `center` within `radius` chunks.
pub fn simulate_water_near(
    chunks: &mut HashMap<ChunkPos, ChunkData>,
    center_and_radius: Option<(ChunkPos, i32)>,
) -> Vec<WaterChange> {
    simulate_fluid_near(chunks, WATER_CONFIG, center_and_radius)
}

pub fn simulate_lava(chunks: &mut HashMap<ChunkPos, ChunkData>) -> Vec<LavaChange> {
    simulate_lava_near(chunks, None)
}

pub fn simulate_lava_near(
    chunks: &mut HashMap<ChunkPos, ChunkData>,
    center_and_radius: Option<(ChunkPos, i32)>,
) -> Vec<LavaChange> {
    let mut changes = simulate_fluid_near(chunks, LAVA_CONFIG, center_and_radius);
    apply_lava_water_mixing(chunks, &mut changes, center_and_radius);
    changes
}

fn simulate_fluid_near(
    chunks: &mut HashMap<ChunkPos, ChunkData>,
    config: FluidConfig,
    center_and_radius: Option<(ChunkPos, i32)>,
) -> Vec<FluidChange> {
    if chunks.is_empty() {
        return Vec::new();
    }

    let mut sources = Vec::new();
    let mut existing_fluid = HashSet::new();

    for (&chunk_pos, chunk) in chunks.iter() {
        if !is_in_simulation_range(chunk_pos, center_and_radius) {
            continue;
        }

        for (index, block) in chunk.blocks.iter().copied().enumerate() {
            if !(config.is_fluid_block)(block) {
                continue;
            }

            let local = index_to_local(index);
            let world_pos = chunk_to_world(chunk_pos, local);
            existing_fluid.insert(world_pos);

            if (config.is_source_block)(block) {
                sources.push(world_pos);
            }
        }
    }

    let mut targets: HashMap<IVec3, FluidTarget> = HashMap::new();
    let mut queue: VecDeque<(IVec3, u8)> = VecDeque::new();
    for source in sources {
        targets.insert(source, FluidTarget::Source);
        queue.push_back((source, 0));
    }

    while let Some((pos, level)) = queue.pop_front() {
        let down = pos + FLUID_DOWN_DIRECTION;
        try_enqueue_flow(chunks, &mut targets, &mut queue, down, config.down_flow_level, config);

        if level >= config.max_horizontal_level {
            continue;
        }

        for dir in FLUID_HORIZONTAL_DIRECTIONS {
            try_enqueue_flow(chunks, &mut targets, &mut queue, pos + dir, level + 1, config);
        }
    }

    let mut positions_to_update = existing_fluid;
    positions_to_update.extend(targets.keys().copied());

    let mut changes = Vec::new();
    for world_pos in positions_to_update {
        let Some(current_block) = get_loaded_block(chunks, world_pos) else {
            continue;
        };

        let desired_block = match targets.get(&world_pos).copied() {
            Some(FluidTarget::Source) => config.source_block,
            Some(FluidTarget::Flow(level)) => (config.flow_block_from_level)(level),
            None => BlockId::AIR,
        };

        if current_block == desired_block {
            continue;
        }

        if set_loaded_block(chunks, world_pos, desired_block) {
            record_change(
                &mut changes,
                FluidChange {
                    world_pos,
                    new_block: desired_block,
                },
            );
        }
    }

    changes
}

fn try_enqueue_flow(
    chunks: &HashMap<ChunkPos, ChunkData>,
    targets: &mut HashMap<IVec3, FluidTarget>,
    queue: &mut VecDeque<(IVec3, u8)>,
    world_pos: IVec3,
    level: u8,
    config: FluidConfig,
) {
    let Some(block) = get_loaded_block(chunks, world_pos) else {
        return;
    };
    if !(config.can_occupy)(block) {
        return;
    }

    if (config.is_source_block)(block) {
        if !matches!(targets.get(&world_pos), Some(FluidTarget::Source)) {
            targets.insert(world_pos, FluidTarget::Source);
            queue.push_back((world_pos, 0));
        }
        return;
    }

    match targets.get(&world_pos).copied() {
        Some(FluidTarget::Source) => {}
        Some(FluidTarget::Flow(existing_level)) if existing_level <= level => {}
        _ => {
            targets.insert(world_pos, FluidTarget::Flow(level));
            queue.push_back((world_pos, level));
        }
    }
}

fn apply_lava_water_mixing(
    chunks: &mut HashMap<ChunkPos, ChunkData>,
    changes: &mut Vec<FluidChange>,
    center_and_radius: Option<(ChunkPos, i32)>,
) {
    let mut mixed_blocks: HashMap<IVec3, BlockId> = HashMap::new();

    for (&chunk_pos, chunk) in chunks.iter() {
        if !is_in_simulation_range(chunk_pos, center_and_radius) {
            continue;
        }

        for (index, block) in chunk.blocks.iter().copied().enumerate() {
            if !is_lava_block(block) {
                continue;
            }

            let local = index_to_local(index);
            let world_pos = chunk_to_world(chunk_pos, local);

            let mut touches_water_source = false;
            let mut touches_water_flowing = false;
            for direction in NEIGHBOR_DIRECTIONS {
                let Some(neighbor_block) = get_loaded_block(chunks, world_pos + direction) else {
                    continue;
                };

                if is_water_source_block(neighbor_block) {
                    touches_water_source = true;
                } else if is_water_block(neighbor_block) {
                    touches_water_flowing = true;
                }
            }

            let mixed_block = if is_lava_source_block(block) && touches_water_flowing {
                Some(BlockId::OBSIDIAN)
            } else if !is_lava_source_block(block)
                && (touches_water_source || touches_water_flowing)
            {
                Some(BlockId::RUBBLESTONE)
            } else {
                None
            };

            if let Some(new_block) = mixed_block {
                mixed_blocks.insert(world_pos, new_block);
            }
        }
    }

    for (world_pos, new_block) in mixed_blocks {
        if !set_loaded_block(chunks, world_pos, new_block) {
            continue;
        }

        record_change(
            changes,
            FluidChange {
                world_pos,
                new_block,
            },
        );
    }
}

fn record_change(changes: &mut Vec<FluidChange>, change: FluidChange) {
    if let Some(existing) = changes
        .iter_mut()
        .find(|existing| existing.world_pos == change.world_pos)
    {
        existing.new_block = change.new_block;
        return;
    }

    changes.push(change);
}

fn is_in_simulation_range(chunk_pos: ChunkPos, center_and_radius: Option<(ChunkPos, i32)>) -> bool {
    let Some((center, radius)) = center_and_radius else {
        return true;
    };

    let dx = (chunk_pos.x - center.x).abs();
    let dy = (chunk_pos.y - center.y).abs();
    let dz = (chunk_pos.z - center.z).abs();
    dx <= radius && dy <= radius && dz <= radius
}

fn can_water_occupy(block: BlockId) -> bool {
    block == BlockId::AIR || is_water_block(block)
}

fn can_lava_occupy(block: BlockId) -> bool {
    block == BlockId::AIR || is_lava_block(block)
}

fn get_loaded_block(chunks: &HashMap<ChunkPos, ChunkData>, world_pos: IVec3) -> Option<BlockId> {
    let (chunk_pos, local_pos) = world_to_chunk(world_pos);
    let chunk = chunks.get(&chunk_pos)?;
    Some(chunk.blocks[local_to_index(local_pos)])
}

fn set_loaded_block(
    chunks: &mut HashMap<ChunkPos, ChunkData>,
    world_pos: IVec3,
    block: BlockId,
) -> bool {
    let (chunk_pos, local_pos) = world_to_chunk(world_pos);
    let Some(chunk) = chunks.get_mut(&chunk_pos) else {
        return false;
    };
    chunk.blocks[local_to_index(local_pos)] = block;
    true
}

#[cfg(test)]
mod tests {
    use super::{simulate_lava, simulate_water};
    use crate::block::{lava_flow_block_from_level, water_flow_block_from_level, BlockId};
    use crate::chunk::ChunkData;
    use crate::coords::{local_to_index, world_to_chunk, ChunkPos};
    use glam::IVec3;
    use std::collections::HashMap;

    #[test]
    fn water_source_spreads_and_recedes_when_removed() {
        let mut chunks = HashMap::from([(ChunkPos { x: 0, y: 0, z: 0 }, ChunkData::new_empty())]);
        set_block(&mut chunks, IVec3::new(8, 8, 8), BlockId::STILL_WATER);

        let changes = simulate_water(&mut chunks);
        assert!(!changes.is_empty());
        assert_eq!(block(&chunks, IVec3::new(8, 8, 8)), Some(BlockId::STILL_WATER));
        assert_eq!(
            block(&chunks, IVec3::new(8, 7, 8)),
            Some(water_flow_block_from_level(0))
        );
        assert_eq!(
            block(&chunks, IVec3::new(9, 8, 8)),
            Some(water_flow_block_from_level(1))
        );

        set_block(&mut chunks, IVec3::new(8, 8, 8), BlockId::AIR);
        let recede_changes = simulate_water(&mut chunks);
        assert!(!recede_changes.is_empty());
        assert_eq!(block(&chunks, IVec3::new(8, 7, 8)), Some(BlockId::AIR));
        assert_eq!(block(&chunks, IVec3::new(9, 8, 8)), Some(BlockId::AIR));
    }

    #[test]
    fn lava_spreads_with_shorter_horizontal_range() {
        let mut chunks = HashMap::from([(ChunkPos { x: 0, y: 0, z: 0 }, ChunkData::new_empty())]);
        set_block(&mut chunks, IVec3::new(8, 8, 8), BlockId::LAVA_SOURCE);

        simulate_lava(&mut chunks);

        assert_eq!(block(&chunks, IVec3::new(9, 8, 8)), Some(lava_flow_block_from_level(1)));
        assert_eq!(
            block(&chunks, IVec3::new(10, 8, 8)),
            Some(lava_flow_block_from_level(2))
        );
        assert_eq!(block(&chunks, IVec3::new(11, 8, 8)), Some(BlockId::AIR));
    }

    #[test]
    fn mixing_lava_source_with_flowing_water_creates_obsidian() {
        let mut chunks = HashMap::from([(ChunkPos { x: 0, y: 0, z: 0 }, ChunkData::new_empty())]);
        set_block(&mut chunks, IVec3::new(8, 8, 8), BlockId::LAVA_SOURCE);
        set_block(
            &mut chunks,
            IVec3::new(9, 8, 8),
            water_flow_block_from_level(1),
        );

        simulate_lava(&mut chunks);
        assert_eq!(block(&chunks, IVec3::new(8, 8, 8)), Some(BlockId::OBSIDIAN));
    }

    #[test]
    fn mixing_flowing_lava_with_source_or_flowing_water_creates_cobblestone() {
        let mut chunks_source_water =
            HashMap::from([(ChunkPos { x: 0, y: 0, z: 0 }, ChunkData::new_empty())]);
        set_block(
            &mut chunks_source_water,
            IVec3::new(7, 8, 8),
            BlockId::LAVA_SOURCE,
        );
        set_block(
            &mut chunks_source_water,
            IVec3::new(9, 8, 8),
            BlockId::STILL_WATER,
        );

        simulate_lava(&mut chunks_source_water);
        assert_eq!(
            block(&chunks_source_water, IVec3::new(8, 8, 8)),
            Some(BlockId::RUBBLESTONE)
        );

        let mut chunks_flowing_water =
            HashMap::from([(ChunkPos { x: 0, y: 0, z: 0 }, ChunkData::new_empty())]);
        set_block(
            &mut chunks_flowing_water,
            IVec3::new(7, 8, 8),
            BlockId::LAVA_SOURCE,
        );
        set_block(
            &mut chunks_flowing_water,
            IVec3::new(9, 8, 8),
            water_flow_block_from_level(2),
        );

        simulate_lava(&mut chunks_flowing_water);
        assert_eq!(
            block(&chunks_flowing_water, IVec3::new(8, 8, 8)),
            Some(BlockId::RUBBLESTONE)
        );
    }

    fn block(chunks: &HashMap<ChunkPos, ChunkData>, pos: IVec3) -> Option<BlockId> {
        let (chunk_pos, local_pos) = world_to_chunk(pos);
        let chunk = chunks.get(&chunk_pos)?;
        Some(chunk.blocks[local_to_index(local_pos)])
    }

    fn set_block(chunks: &mut HashMap<ChunkPos, ChunkData>, pos: IVec3, block: BlockId) {
        let (chunk_pos, local_pos) = world_to_chunk(pos);
        let chunk = chunks
            .get_mut(&chunk_pos)
            .expect("test chunk should exist for set_block");
        chunk.blocks[local_to_index(local_pos)] = block;
    }
}
