use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use veldspar_persist::region::RegionFile;
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::ChunkPos;

pub struct PersistenceLayer {
    world_dir: PathBuf,
    regions: HashMap<(i32, i32), RegionFile>,
}

impl PersistenceLayer {
    pub fn open(world_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(world_dir)?;
        Ok(Self {
            world_dir: world_dir.to_path_buf(),
            regions: HashMap::new(),
        })
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) -> io::Result<Option<ChunkData>> {
        let region_coords = Self::region_coords(pos);
        let region = self.region_mut(region_coords)?;
        Ok(region.load_chunk(pos))
    }

    pub fn save_chunk(&mut self, pos: ChunkPos, chunk: &ChunkData) -> io::Result<()> {
        let region_coords = Self::region_coords(pos);
        let region = self.region_mut(region_coords)?;
        region.save_chunk(pos, chunk);
        region.flush()
    }

    pub fn flush_all(&mut self) -> io::Result<()> {
        for region in self.regions.values() {
            region.flush()?;
        }
        Ok(())
    }

    fn region_coords(pos: ChunkPos) -> (i32, i32) {
        (pos.x.div_euclid(16), pos.z.div_euclid(16))
    }

    fn region_path(&self, region_coords: (i32, i32)) -> PathBuf {
        let (rx, rz) = region_coords;
        self.world_dir
            .join("region")
            .join(format!("r.{rx}.{rz}.vsr"))
    }

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
