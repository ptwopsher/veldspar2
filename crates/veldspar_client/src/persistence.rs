use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use glam::IVec3;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use veldspar_persist::region::RegionFile;
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::ChunkPos;
use veldspar_shared::inventory::Inventory;

const REGION_CHUNK_SPAN: i32 = 16;
const CHEST_DATA_FILE: &str = "chests.dat";
const INVENTORY_DATA_FILE: &str = "inventory.dat";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChestInventoryEntry {
    position: [i32; 3],
    inventory: Inventory,
}

#[derive(Debug, Clone)]
pub struct WorldSummary {
    pub folder_name: String,
    pub display_name: String,
    pub world_dir: PathBuf,
    pub world_seed: u64,
    pub regions_size_bytes: u64,
    pub last_opened: Option<SystemTime>,
}

/// Client-side persistence layer for saving and loading chunks
pub struct ClientPersistence {
    world_dir: PathBuf,
    regions: HashMap<(i32, i32), RegionFile>,
}

impl ClientPersistence {
    /// Opens or creates a world save directory
    pub fn open(world_dir: &Path) -> io::Result<Self> {
        let regions_dir = world_dir.join("regions");
        fs::create_dir_all(&regions_dir)?;

        Ok(Self {
            world_dir: world_dir.to_path_buf(),
            regions: HashMap::new(),
        })
    }

    /// Loads a chunk from disk if it exists
    pub fn load_chunk(&mut self, pos: ChunkPos) -> io::Result<Option<ChunkData>> {
        let region_coords = Self::region_coords(pos);
        let region = self.region_mut(region_coords)?;
        Ok(region.load_chunk(pos))
    }

    /// Saves a chunk to disk
    pub fn save_chunk(&mut self, pos: ChunkPos, chunk: &ChunkData) -> io::Result<()> {
        let region_coords = Self::region_coords(pos);
        let region = self.region_mut(region_coords)?;
        region.save_chunk(pos, chunk);
        region.flush()
    }

    /// Saves a chunk to memory only (no disk flush)
    pub fn stage_chunk(&mut self, pos: ChunkPos, chunk: &ChunkData) -> io::Result<()> {
        let region_coords = Self::region_coords(pos);
        let region = self.region_mut(region_coords)?;
        region.save_chunk(pos, chunk);
        Ok(())
    }

    /// Saves all chunks to disk
    pub fn save_all_chunks(&mut self, chunks: &HashMap<ChunkPos, ChunkData>) -> io::Result<()> {
        let mut touched_regions = HashSet::new();

        for (pos, chunk) in chunks {
            let region_coords = Self::region_coords(*pos);
            let region = self.region_mut(region_coords)?;
            region.save_chunk(*pos, chunk);
            touched_regions.insert(region_coords);
        }

        for region_coords in touched_regions {
            let region = self.regions.get(&region_coords).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "failed to access cached region file for flush",
                )
            })?;
            region.flush()?;
        }

        Ok(())
    }

    /// Flushes all cached region files to disk
    pub fn flush_all(&mut self) -> io::Result<()> {
        for region in self.regions.values() {
            region.flush()?;
        }
        Ok(())
    }

    /// Saves all non-empty chest inventories to disk.
    pub fn save_chest_inventories(
        &self,
        chest_inventories: &HashMap<IVec3, Inventory>,
    ) -> io::Result<()> {
        let mut entries: Vec<ChestInventoryEntry> = chest_inventories
            .iter()
            .filter_map(|(pos, inventory)| {
                inventory_has_items(inventory).then(|| ChestInventoryEntry {
                    position: [pos.x, pos.y, pos.z],
                    inventory: inventory.clone(),
                })
            })
            .collect();
        entries.sort_by_key(|entry| (entry.position[0], entry.position[1], entry.position[2]));

        let encoded = bincode::serialize(&entries).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to serialize chest inventories: {e}"),
            )
        })?;
        fs::write(self.chest_data_path(), encoded)
    }

    /// Loads chest inventories from disk. Missing files are treated as empty.
    pub fn load_chest_inventories(&self) -> io::Result<HashMap<IVec3, Inventory>> {
        let path = self.chest_data_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }

        let encoded = fs::read(path)?;
        let entries: Vec<ChestInventoryEntry> = bincode::deserialize(&encoded).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to deserialize chest inventories: {e}"),
            )
        })?;

        let mut chest_inventories = HashMap::with_capacity(entries.len());
        for entry in entries {
            if !inventory_has_items(&entry.inventory) {
                continue;
            }
            let pos = IVec3::new(entry.position[0], entry.position[1], entry.position[2]);
            chest_inventories.insert(pos, entry.inventory);
        }
        Ok(chest_inventories)
    }

    /// Calculates which region a chunk belongs to
    fn region_coords(pos: ChunkPos) -> (i32, i32) {
        (
            pos.x.div_euclid(REGION_CHUNK_SPAN),
            pos.z.div_euclid(REGION_CHUNK_SPAN),
        )
    }

    /// Gets the file path for a region
    fn region_path(&self, region_coords: (i32, i32)) -> PathBuf {
        let (rx, rz) = region_coords;
        self.world_dir
            .join("regions")
            .join(format!("r.{rx}.{rz}.vsr"))
    }

    fn chest_data_path(&self) -> PathBuf {
        self.world_dir.join(CHEST_DATA_FILE)
    }

    fn inventory_data_path(&self) -> PathBuf {
        self.world_dir.join(INVENTORY_DATA_FILE)
    }

    /// Saves the player inventory to disk.
    pub fn save_inventory(&self, inventory: &Inventory) -> io::Result<()> {
        let encoded = bincode::serialize(inventory).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to serialize player inventory: {e}"),
            )
        })?;
        fs::write(self.inventory_data_path(), encoded)
    }

    /// Loads the player inventory from disk. Missing file returns a fresh inventory.
    pub fn load_inventory(&self) -> io::Result<Inventory> {
        let path = self.inventory_data_path();
        if !path.exists() {
            return Ok(Inventory::new());
        }

        let encoded = fs::read(path)?;
        bincode::deserialize::<Inventory>(&encoded).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to deserialize player inventory: {e}"),
            )
        })
    }

    /// Gets or creates a mutable reference to a region file
    fn region_mut(&mut self, region_coords: (i32, i32)) -> io::Result<&mut RegionFile> {
        if !self.regions.contains_key(&region_coords) {
            let region_path = self.region_path(region_coords);
            let region = RegionFile::open(region_path)?;
            self.regions.insert(region_coords, region);
        }

        self.regions.get_mut(&region_coords).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "failed to access cached region file",
            )
        })
    }
}

fn inventory_has_items(inventory: &Inventory) -> bool {
    inventory
        .slots
        .iter()
        .flatten()
        .any(|stack| stack.count > 0)
}

pub fn scan_worlds(worlds_dir: &Path) -> io::Result<Vec<WorldSummary>> {
    fs::create_dir_all(worlds_dir)?;

    let mut worlds = Vec::new();
    for entry in fs::read_dir(worlds_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let world_dir = entry.path();
        let folder_name = entry.file_name().to_string_lossy().to_string();
        let world_meta = WorldMeta::load(&world_dir).ok().flatten();
        let display_name = world_meta
            .as_ref()
            .and_then(|meta| meta.world_name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| folder_name.clone());
        let world_seed = world_meta.as_ref().map(|meta| meta.world_seed).unwrap_or(0);
        let regions_size_bytes = directory_size_bytes(&world_dir.join("regions")).unwrap_or(0);
        let last_opened = fs::metadata(world_dir.join("world.toml"))
            .and_then(|meta| meta.modified())
            .ok();

        worlds.push(WorldSummary {
            folder_name,
            display_name,
            world_dir,
            world_seed,
            regions_size_bytes,
            last_opened,
        });
    }

    worlds.sort_by(|a, b| {
        let a_time = a
            .last_opened
            .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let b_time = b
            .last_opened
            .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        b_time
            .cmp(&a_time)
            .then_with(|| a.folder_name.cmp(&b.folder_name))
    });

    Ok(worlds)
}

fn directory_size_bytes(dir: &Path) -> io::Result<u64> {
    if !dir.exists() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total = total.saturating_add(directory_size_bytes(&entry.path())?);
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }

    Ok(total)
}

/// World metadata including seed, player state, and time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SavedPlayMode {
    Survival,
    Creative,
}

impl Default for SavedPlayMode {
    fn default() -> Self {
        Self::Survival
    }
}

/// World metadata including seed, player state, and time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_name: Option<String>,
    #[serde(with = "world_seed_serde")]
    pub world_seed: u64,
    pub player_position: [f32; 3],
    pub player_yaw: f32,
    pub player_pitch: f32,
    pub time_of_day: f32,
    #[serde(default)]
    pub play_mode: SavedPlayMode,
}

impl WorldMeta {
    /// Saves world metadata to world.toml
    pub fn save(&self, world_dir: &Path) -> io::Result<()> {
        fs::create_dir_all(world_dir)?;

        let toml_string = toml::to_string_pretty(self).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("failed to serialize world metadata: {}", e))
        })?;

        let meta_path = world_dir.join("world.toml");
        fs::write(meta_path, toml_string)
    }

    /// Loads world metadata from world.toml
    pub fn load(world_dir: &Path) -> io::Result<Option<Self>> {
        let meta_path = world_dir.join("world.toml");

        // Return None if the file doesn't exist
        if !meta_path.exists() {
            return Ok(None);
        }

        let toml_string = fs::read_to_string(meta_path)?;
        let meta = toml::from_str(&toml_string).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("failed to deserialize world metadata: {}", e))
        })?;

        Ok(Some(meta))
    }
}

mod world_seed_serde {
    use super::*;
    use std::fmt;

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SeedVisitor;

        impl<'de> Visitor<'de> for SeedVisitor {
            type Value = u64;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a non-negative 64-bit world seed as integer or string")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u64::try_from(value).map_err(|_| E::custom("world seed must be non-negative"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| E::custom("invalid world seed string"))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(SeedVisitor)
    }
}

// Legacy compatibility: Keep the old API for existing code
#[deprecated(note = "Use ClientPersistence instead")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[deprecated(note = "Use ClientPersistence instead")]
pub struct WorldSave {
    world_dir: PathBuf,
    regions_dir: PathBuf,
    regions: HashMap<(i32, i32), RegionFile>,
}

#[allow(deprecated)]
impl WorldSave {
    pub fn new(world_dir: &Path) -> Self {
        let world_dir = world_dir.to_path_buf();
        let regions_dir = world_dir.join("regions");

        let _ = fs::create_dir_all(&regions_dir);

        Self {
            world_dir,
            regions_dir,
            regions: HashMap::new(),
        }
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) -> Option<ChunkData> {
        let region_coords = Self::region_coords(pos);
        let region = self.region_mut(region_coords)?;
        region.load_chunk(pos)
    }

    pub fn save_chunk(&mut self, pos: ChunkPos, chunk: &ChunkData) {
        let region_coords = Self::region_coords(pos);
        let Some(region) = self.region_mut(region_coords) else {
            return;
        };

        region.save_chunk(pos, chunk);
        let _ = region.flush();
    }

    pub fn save_player_state(&self, pos: [f32; 3], yaw: f32, pitch: f32) {
        let player_state = PlayerState {
            position: pos,
            yaw,
            pitch,
        };

        let Ok(encoded) = bincode::serialize(&player_state) else {
            return;
        };

        let _ = fs::create_dir_all(&self.world_dir);
        let _ = fs::write(self.player_path(), encoded);
    }

    pub fn load_player_state(&self) -> Option<PlayerState> {
        let bytes = fs::read(self.player_path()).ok()?;
        bincode::deserialize::<PlayerState>(&bytes).ok()
    }

    fn region_coords(pos: ChunkPos) -> (i32, i32) {
        (
            pos.x.div_euclid(REGION_CHUNK_SPAN),
            pos.z.div_euclid(REGION_CHUNK_SPAN),
        )
    }

    fn region_path(&self, region_coords: (i32, i32)) -> PathBuf {
        let (rx, rz) = region_coords;
        self.regions_dir.join(format!("r.{rx}.{rz}.vsr"))
    }

    fn region_mut(&mut self, region_coords: (i32, i32)) -> Option<&mut RegionFile> {
        if !self.regions.contains_key(&region_coords) {
            let region_path = self.region_path(region_coords);
            let region = RegionFile::open(&region_path).ok()?;
            self.regions.insert(region_coords, region);
        }

        self.regions.get_mut(&region_coords)
    }

    fn player_path(&self) -> PathBuf {
        self.world_dir.join("player.dat")
    }
}
