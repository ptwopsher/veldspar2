use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::ChunkPos;

use crate::compression::{compress_zstd, decompress_zstd};
use crate::versioning::{
    migrate_region_payload, CURRENT_REGION_FORMAT_VERSION, FORMAT_VERSION,
};

#[derive(Serialize, Deserialize)]
struct RegionDisk {
    format_version: u32,
    chunks: Vec<(ChunkPos, ChunkData)>,
}

pub struct RegionFile {
    path: PathBuf,
    chunks: HashMap<ChunkPos, ChunkData>,
}

impl RegionFile {
    pub const MAGIC: [u8; 4] = *b"VSPR";
    const WIRE_VERSION_UNCOMPRESSED: u8 = 1;
    const WIRE_VERSION_ZSTD: u8 = 2;

    fn decode_region_disk(payload: &[u8]) -> io::Result<RegionDisk> {
        bincode::deserialize(payload).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to decode region payload: {err}"),
            )
        })
    }

    fn decode_region_version(payload: &[u8]) -> io::Result<u32> {
        let mut cursor = Cursor::new(payload);
        bincode::deserialize_from::<_, u32>(&mut cursor).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to decode region version prefix: {err}"),
            )
        })
    }

    fn decode_region_disk_with_migration(payload: &[u8]) -> io::Result<RegionDisk> {
        let source_version = Self::decode_region_version(payload)?;
        let migrated_payload = migrate_region_payload(source_version, payload.to_vec()).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to migrate region payload from format v{source_version}: {err}"),
            )
        })?;
        if source_version != CURRENT_REGION_FORMAT_VERSION {
            info!(
                "Migrated region payload format v{} -> v{}",
                source_version, CURRENT_REGION_FORMAT_VERSION
            );
        }
        Self::decode_region_disk(&migrated_payload)
    }

    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();

        if !path.exists() {
            return Ok(Self {
                path,
                chunks: HashMap::new(),
            });
        }

        let bytes = fs::read(&path)?;
        if bytes.is_empty() {
            return Ok(Self {
                path,
                chunks: HashMap::new(),
            });
        }

        if bytes.len() < Self::MAGIC.len() || bytes[..4] != Self::MAGIC[..] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid region file magic; expected VSPR",
            ));
        }

        let payload = &bytes[Self::MAGIC.len()..];
        if payload.is_empty() {
            return Ok(Self {
                path,
                chunks: HashMap::new(),
            });
        }

        let (wire_version, wire_payload) = payload.split_first().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing region wire format version")
        })?;

        let disk = match *wire_version {
            Self::WIRE_VERSION_UNCOMPRESSED => match Self::decode_region_disk_with_migration(wire_payload) {
                Ok(disk) => disk,
                Err(versioned_err) => {
                    // Backward compatibility for files written before the wire version byte existed.
                    Self::decode_region_disk_with_migration(payload).map_err(|legacy_err| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "failed to decode region payload (versioned: {versioned_err}; legacy: {legacy_err})"
                            ),
                        )
                    })?
                }
            },
            Self::WIRE_VERSION_ZSTD => {
                let decompressed = decompress_zstd(wire_payload).map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("failed to decompress region payload: {err}"),
                    )
                })?;
                Self::decode_region_disk_with_migration(&decompressed)?
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "unsupported region wire format version {other}; expected 1 or 2"
                    ),
                ))
            }
        };

        debug!(
            "Loaded region {:?} with {} chunks (format v{})",
            path,
            disk.chunks.len(),
            disk.format_version
        );

        Ok(Self {
            path,
            chunks: disk.chunks.into_iter().collect(),
        })
    }

    pub fn save_chunk(&mut self, pos: ChunkPos, chunk: &ChunkData) {
        self.chunks.insert(pos, chunk.clone());
    }

    pub fn load_chunk(&self, pos: ChunkPos) -> Option<ChunkData> {
        self.chunks.get(&pos).cloned()
    }

    pub fn flush(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let disk = RegionDisk {
            format_version: FORMAT_VERSION,
            chunks: self
                .chunks
                .iter()
                .map(|(pos, chunk)| (*pos, chunk.clone()))
                .collect(),
        };

        let encoded = bincode::serialize(&disk).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to encode region payload: {err}"),
            )
        })?;
        let compressed = compress_zstd(&encoded, 3).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to compress region payload: {err}"),
            )
        })?;
        let mut bytes = Vec::with_capacity(Self::MAGIC.len() + 1 + compressed.len());
        bytes.extend_from_slice(&Self::MAGIC);
        bytes.push(Self::WIRE_VERSION_ZSTD);
        bytes.extend_from_slice(&compressed);

        fs::write(&self.path, bytes)
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn chunk_positions(&self) -> Vec<ChunkPos> {
        self.chunks.keys().copied().collect()
    }
}
