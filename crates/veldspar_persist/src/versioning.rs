use serde::{Deserialize, Serialize};
use tracing::info;

use veldspar_shared::block::BlockId;
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::{ChunkPos, CHUNK_VOLUME};

pub const CURRENT_REGION_FORMAT_VERSION: u32 = 2;
pub const FORMAT_VERSION: u32 = CURRENT_REGION_FORMAT_VERSION;

#[derive(Serialize, Deserialize)]
struct RegionDiskV2 {
    format_version: u32,
    chunks: Vec<(ChunkPos, ChunkData)>,
}

#[derive(Serialize, Deserialize)]
struct RegionDiskV1ChunkData {
    format_version: u32,
    chunks: Vec<(ChunkPos, ChunkData)>,
}

#[derive(Serialize, Deserialize)]
struct RegionDiskV1LegacyU8 {
    format_version: u32,
    chunks: Vec<(ChunkPos, LegacyChunkDataU8)>,
}

#[derive(Serialize, Deserialize)]
struct RegionDiskV1LegacyU16 {
    format_version: u32,
    chunks: Vec<(ChunkPos, LegacyChunkDataU16)>,
}

#[derive(Serialize, Deserialize)]
struct LegacyChunkDataU8 {
    blocks: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct LegacyChunkDataU16 {
    blocks: Vec<u16>,
}

pub fn migrate_region_payload(mut version: u32, mut payload: Vec<u8>) -> Result<Vec<u8>, String> {
    if version == CURRENT_REGION_FORMAT_VERSION {
        return Ok(payload);
    }

    if version == 0 || version > CURRENT_REGION_FORMAT_VERSION {
        return Err(format!(
            "unsupported region format version {version}; current version is {CURRENT_REGION_FORMAT_VERSION}"
        ));
    }

    while version < CURRENT_REGION_FORMAT_VERSION {
        let next_version = version + 1;
        info!("Migrating region payload format v{version} -> v{next_version}");
        payload = migrate_one_version(version, payload)?;
        version = next_version;
    }

    Ok(payload)
}

fn migrate_one_version(version: u32, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    match version {
        1 => migrate_region_v1_to_v2(payload),
        other => Err(format!(
            "missing migration path for region format v{other} -> v{}",
            other + 1
        )),
    }
}

fn migrate_region_v1_to_v2(payload: Vec<u8>) -> Result<Vec<u8>, String> {
    if let Ok(v1) = bincode::deserialize::<RegionDiskV1ChunkData>(&payload) {
        let _ = v1.format_version;
        return encode_v2(v1.chunks);
    }

    if let Ok(v1) = bincode::deserialize::<RegionDiskV1LegacyU16>(&payload) {
        let _ = v1.format_version;
        let chunks = v1
            .chunks
            .into_iter()
            .map(|(pos, chunk)| legacy_blocks_to_chunk(pos, &chunk.blocks))
            .collect::<Result<Vec<_>, _>>()?;
        return encode_v2(chunks);
    }

    if let Ok(v1) = bincode::deserialize::<RegionDiskV1LegacyU8>(&payload) {
        let _ = v1.format_version;
        let chunks = v1
            .chunks
            .into_iter()
            .map(|(pos, chunk)| legacy_blocks_to_chunk(pos, &chunk.blocks))
            .collect::<Result<Vec<_>, _>>()?;
        return encode_v2(chunks);
    }

    Err("failed to decode region payload for migration from v1 to v2".to_string())
}

fn legacy_blocks_to_chunk<T>(pos: ChunkPos, blocks: &[T]) -> Result<(ChunkPos, ChunkData), String>
where
    T: Copy + Into<u16>,
{
    if blocks.len() != CHUNK_VOLUME {
        return Err(format!(
            "chunk {pos:?} has {} blocks; expected {}",
            blocks.len(),
            CHUNK_VOLUME
        ));
    }

    let mut chunk = ChunkData::new_empty();
    for (index, &block) in blocks.iter().enumerate() {
        chunk.set_index(index, BlockId(block.into()));
    }

    Ok((pos, chunk))
}

fn encode_v2(chunks: Vec<(ChunkPos, ChunkData)>) -> Result<Vec<u8>, String> {
    let v2 = RegionDiskV2 {
        format_version: CURRENT_REGION_FORMAT_VERSION,
        chunks,
    };
    bincode::serialize(&v2).map_err(|err| format!("failed to encode migrated v2 payload: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{
        migrate_region_payload, CURRENT_REGION_FORMAT_VERSION, LegacyChunkDataU8, RegionDiskV1LegacyU8,
        RegionDiskV2,
    };
    use veldspar_shared::block::BlockId;
    use veldspar_shared::chunk::ChunkData;
    use veldspar_shared::coords::{ChunkPos, CHUNK_VOLUME};

    #[test]
    fn migrate_v1_payload_to_current_v2_format() {
        let blocks = vec![7u8; CHUNK_VOLUME];
        let payload_v1 = bincode::serialize(&RegionDiskV1LegacyU8 {
            format_version: 1,
            chunks: vec![(
                ChunkPos { x: 2, y: 0, z: -1 },
                LegacyChunkDataU8 { blocks },
            )],
        })
        .expect("serialize v1 payload");

        let migrated =
            migrate_region_payload(1, payload_v1).expect("migrate region payload from v1 to v2");
        let decoded: RegionDiskV2 =
            bincode::deserialize(&migrated).expect("deserialize migrated v2 payload");

        assert_eq!(decoded.format_version, CURRENT_REGION_FORMAT_VERSION);
        assert_eq!(decoded.chunks.len(), 1);
        let (_, chunk) = &decoded.chunks[0];
        assert_eq!(chunk.get_index(0), BlockId(7));
        assert_eq!(chunk.get_index(CHUNK_VOLUME - 1), BlockId(7));
    }

    #[test]
    fn current_version_payload_is_unchanged() {
        let mut chunk = ChunkData::new_empty();
        chunk.set_index(0, BlockId(3));
        let payload = bincode::serialize(&RegionDiskV2 {
            format_version: CURRENT_REGION_FORMAT_VERSION,
            chunks: vec![(ChunkPos { x: 0, y: 0, z: 0 }, chunk)],
        })
        .expect("serialize v2 payload");

        let migrated = migrate_region_payload(CURRENT_REGION_FORMAT_VERSION, payload.clone())
            .expect("no-op migration should succeed");
        assert_eq!(migrated, payload);
    }

    #[test]
    fn unknown_version_returns_error() {
        let err = migrate_region_payload(99, vec![1, 2, 3]).expect_err("unknown version must fail");
        assert!(err.contains("unsupported region format version 99"));
    }
}
