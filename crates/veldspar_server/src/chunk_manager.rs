use std::collections::HashSet;

use veldspar_shared::coords::ChunkPos;

#[derive(Default)]
pub struct ChunkManager {
    pending: HashSet<ChunkPos>,
}

impl ChunkManager {
    pub fn request(&mut self, pos: ChunkPos) {
        self.pending.insert(pos);
    }

    pub fn process_requests(&mut self) {
        self.pending.clear();
    }
}
