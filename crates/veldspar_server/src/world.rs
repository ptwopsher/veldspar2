use std::collections::{HashMap, HashSet};
use std::path::Path;

use glam::IVec3;
use tracing::warn;
use veldspar_shared::block::{register_default_blocks, BlockId, BlockRegistry};
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::{world_to_chunk, ChunkPos};
use veldspar_shared::fluid::{simulate_water, WaterChange};
use veldspar_shared::worldgen::WorldGenerator;

use crate::persistence::PersistenceLayer;

const DEFAULT_WORLD_SEED: u64 = 0xC0FFEE;

pub struct ServerWorld {
    pub loaded_chunks: HashMap<ChunkPos, ChunkData>,
    pub tick_count: u64,
    persistence: Option<PersistenceLayer>,
    dirty_chunks: HashSet<ChunkPos>,
    generator: WorldGenerator,
    registry: BlockRegistry,
}

impl Default for ServerWorld {
    fn default() -> Self {
        Self {
            loaded_chunks: HashMap::new(),
            tick_count: 0,
            persistence: None,
            dirty_chunks: HashSet::new(),
            generator: WorldGenerator::new(DEFAULT_WORLD_SEED),
            registry: register_default_blocks(),
        }
    }
}

impl ServerWorld {
    pub fn with_persistence(world_dir: &Path) -> Self {
        let mut world = Self::default();
        match PersistenceLayer::open(world_dir) {
            Ok(persistence) => {
                world.persistence = Some(persistence);
            }
            Err(err) => {
                warn!(
                    "Failed to initialize persistence at {}: {}",
                    world_dir.display(),
                    err
                );
            }
        }
        world
    }

    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);
    }

    pub fn tick_water(&mut self) -> Vec<WaterChange> {
        let changes = simulate_water(&mut self.loaded_chunks);
        for change in &changes {
            let (chunk_pos, _) = world_to_chunk(change.world_pos);
            self.dirty_chunks.insert(chunk_pos);
        }
        changes
    }

    pub fn world_seed(&self) -> u64 {
        self.generator.seed
    }

    pub fn is_valid_block(&self, block: BlockId) -> bool {
        usize::from(block.0) < self.registry.len()
    }

    pub fn get_or_generate_chunk(&mut self, pos: ChunkPos) -> &ChunkData {
        self.ensure_chunk(pos);
        self.loaded_chunks
            .get(&pos)
            .expect("chunk must exist after ensure_chunk")
    }

    pub fn get_or_generate_chunk_owned(&mut self, pos: ChunkPos) -> ChunkData {
        self.get_or_generate_chunk(pos).clone()
    }

    pub fn get_block(&mut self, world_pos: IVec3) -> BlockId {
        let (chunk_pos, local_pos) = world_to_chunk(world_pos);
        self.ensure_chunk(chunk_pos);
        self.loaded_chunks
            .get(&chunk_pos)
            .expect("chunk must exist after ensure_chunk")
            .get(local_pos)
    }

    pub fn set_block(&mut self, world_pos: IVec3, block: BlockId) -> BlockId {
        let (chunk_pos, local_pos) = world_to_chunk(world_pos);
        self.ensure_chunk(chunk_pos);
        let chunk = self
            .loaded_chunks
            .get_mut(&chunk_pos)
            .expect("chunk must exist after ensure_chunk");
        let previous = chunk.get(local_pos);
        chunk.set(local_pos, block);
        self.dirty_chunks.insert(chunk_pos);
        previous
    }

    pub fn save_dirty_chunks(&mut self) {
        let Some(persistence) = self.persistence.as_mut() else {
            return;
        };

        let dirty_positions: Vec<ChunkPos> = self.dirty_chunks.iter().copied().collect();
        for pos in dirty_positions {
            let Some(chunk) = self.loaded_chunks.get(&pos) else {
                self.dirty_chunks.remove(&pos);
                continue;
            };

            match persistence.save_chunk(pos, chunk) {
                Ok(()) => {
                    self.dirty_chunks.remove(&pos);
                }
                Err(err) => {
                    warn!("Failed to save dirty chunk {:?}: {}", pos, err);
                }
            }
        }

        if let Err(err) = persistence.flush_all() {
            warn!("Failed to flush region files: {}", err);
        }
    }

    fn ensure_chunk(&mut self, pos: ChunkPos) {
        if self.loaded_chunks.contains_key(&pos) {
            return;
        }

        if let Some(persistence) = self.persistence.as_mut() {
            match persistence.load_chunk(pos) {
                Ok(Some(chunk)) => {
                    self.loaded_chunks.insert(pos, chunk);
                    return;
                }
                Ok(None) => {}
                Err(err) => {
                    warn!("Failed to load chunk {:?}: {}", pos, err);
                }
            }
        }

        let generated = self.generator.generate_chunk(pos, &self.registry);
        if let Some(persistence) = self.persistence.as_mut() {
            if let Err(err) = persistence.save_chunk(pos, &generated) {
                warn!("Failed to save generated chunk {:?}: {}", pos, err);
            }
        }
        self.loaded_chunks.insert(pos, generated);
    }
}
