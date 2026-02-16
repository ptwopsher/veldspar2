use std::collections::HashMap;

use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::ChunkPos;

#[derive(Default)]
pub struct ClientWorld {
    pub loaded_chunks: HashMap<ChunkPos, ChunkData>,
}

impl ClientWorld {
    pub fn set_chunk(&mut self, pos: ChunkPos, chunk: ChunkData) {
        self.loaded_chunks.insert(pos, chunk);
    }

    pub fn remove_chunk(&mut self, pos: &ChunkPos) {
        self.loaded_chunks.remove(pos);
    }
}
