use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::block::BlockId;
use crate::coords::{local_to_index, LocalPos, CHUNK_VOLUME};

#[derive(Clone, Debug)]
pub struct ChunkData {
    pub blocks: Box<[BlockId; CHUNK_VOLUME]>,
}

impl ChunkData {
    pub fn new_empty() -> Self {
        Self {
            blocks: Box::new([BlockId::AIR; CHUNK_VOLUME]),
        }
    }

    pub fn new_filled(block: BlockId) -> Self {
        Self {
            blocks: Box::new([block; CHUNK_VOLUME]),
        }
    }

    pub fn get(&self, local: LocalPos) -> BlockId {
        self.blocks[local_to_index(local)]
    }

    pub fn set(&mut self, local: LocalPos, block: BlockId) {
        let index = local_to_index(local);
        self.blocks[index] = block;
    }

    pub fn get_index(&self, index: usize) -> BlockId {
        self.blocks[index]
    }

    pub fn set_index(&mut self, index: usize, block: BlockId) {
        self.blocks[index] = block;
    }
}

impl Default for ChunkData {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl Serialize for ChunkData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.blocks.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChunkData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let blocks = Vec::<BlockId>::deserialize(deserializer)?;
        if blocks.len() != CHUNK_VOLUME {
            return Err(de::Error::custom(format!(
                "expected {CHUNK_VOLUME} blocks, got {}",
                blocks.len()
            )));
        }

        let blocks: [BlockId; CHUNK_VOLUME] = blocks
            .try_into()
            .map_err(|_| de::Error::custom("failed to deserialize chunk block array"))?;

        Ok(Self {
            blocks: Box::new(blocks),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ChunkData;
    use crate::block::BlockId;
    use crate::coords::{local_to_index, LocalPos, CHUNK_VOLUME};

    #[test]
    fn chunk_creation_and_get_set_work() {
        let mut chunk = ChunkData::new_empty();
        let pos = LocalPos { x: 3, y: 7, z: 11 };
        assert_eq!(chunk.get(pos), BlockId::AIR);

        chunk.set(pos, BlockId(2));
        assert_eq!(chunk.get(pos), BlockId(2));
        assert_eq!(chunk.get_index(local_to_index(pos)), BlockId(2));

        chunk.set_index(0, BlockId(10));
        assert_eq!(chunk.get_index(0), BlockId(10));
    }

    #[test]
    fn chunk_bincode_round_trip_preserves_data() {
        let mut original = ChunkData::new_filled(BlockId(3));
        original.set(LocalPos { x: 0, y: 0, z: 0 }, BlockId(1));
        original.set(LocalPos { x: 31, y: 31, z: 31 }, BlockId(9));
        original.set(LocalPos { x: 5, y: 13, z: 27 }, BlockId(12));

        let encoded = bincode::serialize(&original).expect("serialize chunk");
        let decoded: ChunkData = bincode::deserialize(&encoded).expect("deserialize chunk");

        assert_eq!(decoded.blocks.len(), CHUNK_VOLUME);
        for (lhs, rhs) in original.blocks.iter().zip(decoded.blocks.iter()) {
            assert_eq!(lhs, rhs);
        }
    }
}
