use noise::{NoiseFn, Perlin};

use crate::block::{BlockId, BlockRegistry};
use crate::chunk::ChunkData;
use crate::coords::{chunk_to_world, ChunkPos, LocalPos, CHUNK_SIZE};

const BEDSTONE_LEVEL: i32 = -64;
const SEA_LEVEL: i32 = 22;
const HEIGHT_OFFSET: f64 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Biome {
    Desert,
    Plains,
    Forest,
    Snowy,
    IceSpikes,
    SnowPeaks,
    Jungle,
    BambooJungle,
    Mangrove,
    Swamp,
    CherryGrove,
    Savanna,
    Steppe,
    Taiga,
    Badlands,
    Mesa,
    Mushroom,
    Mountain,
    Ocean,
    Volcanic,
    Tundra,
    BirchForest,
    DarkForest,
    FlowerForest,
}

#[derive(Debug, Clone)]
pub struct WorldGenerator {
    pub seed: u64,
}

impl WorldGenerator {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    fn sample_climate(
        &self,
        biome_noise: &Perlin,
        humidity_noise: &Perlin,
        world_x: i32,
        world_z: i32,
    ) -> (f64, f64) {
        let wx = world_x as f64;
        let wz = world_z as f64;
        let warp_x = biome_noise.get([wx * 0.0008 + 91.0, wz * 0.0008 - 47.0]) * 40.0;
        let warp_z = humidity_noise.get([wx * 0.0009 - 77.0, wz * 0.0009 + 113.0]) * 40.0;
        let climate_x = wx + warp_x;
        let climate_z = wz + warp_z;
        let temperature = biome_noise.get([climate_x * 0.0018, climate_z * 0.0018]);
        let humidity = humidity_noise.get([climate_x * 0.0022, climate_z * 0.0022]);
        (temperature, humidity)
    }

    fn biome_from_climate(temperature: f64, humidity: f64) -> Biome {
        if temperature < -0.5 {
            if humidity > 0.4 {
                Biome::IceSpikes
            } else if humidity > 0.0 {
                Biome::Snowy
            } else if (-0.4..=0.0).contains(&humidity) {
                Biome::SnowPeaks
            } else {
                Biome::Tundra
            }
        } else if temperature < -0.2 {
            if humidity > 0.2 {
                Biome::Taiga
            } else {
                Biome::Mountain
            }
        } else if temperature < 0.0 {
            if humidity > 0.5 {
                Biome::CherryGrove
            } else if humidity > 0.3 {
                Biome::Swamp
            } else {
                Biome::Plains
            }
        } else if temperature < 0.2 {
            if humidity > 0.5 {
                Biome::DarkForest
            } else if humidity > 0.3 {
                Biome::Forest
            } else if (0.1..=0.3).contains(&humidity) {
                Biome::FlowerForest
            } else {
                Biome::BirchForest
            }
        } else if temperature < 0.4 {
            if humidity > 0.5 {
                Biome::BambooJungle
            } else if humidity > 0.4 {
                Biome::Mangrove
            } else if humidity > 0.2 {
                Biome::Jungle
            } else if (-0.2..=0.0).contains(&humidity) {
                Biome::Steppe
            } else {
                Biome::Savanna
            }
        } else if temperature < 0.6 {
            if humidity > 0.2 {
                Biome::Mesa
            } else if humidity > 0.0 {
                Biome::Badlands
            } else {
                Biome::Desert
            }
        } else if humidity <= 0.0 {
            Biome::Volcanic
        } else if humidity > 0.3 {
            Biome::Mushroom
        } else {
            Biome::Ocean
        }
    }

    fn should_place_tree(&self, world_x: i32, world_z: i32, biome: Biome) -> bool {
        let hash = self.seed.wrapping_mul(6364136223846793005)
            .wrapping_add(world_x as u64 * 2654435761)
            .wrapping_add(world_z as u64 * 40503);

        let frequency = match biome {
            Biome::Desert => return false, // No trees in desert (cacti handled separately)
            Biome::Plains => 60,
            Biome::Forest => 25,
            Biome::Snowy => 120,
            Biome::IceSpikes => return false,
            Biome::SnowPeaks => return false,
            Biome::Jungle => 12,
            Biome::BambooJungle => 20,
            Biome::Mangrove => 20,
            Biome::Swamp => 40,
            Biome::CherryGrove => 35,
            Biome::Savanna => 150,
            Biome::Steppe => return false,
            Biome::Taiga => 30,
            Biome::Badlands => return false,
            Biome::Mesa => return false,
            Biome::Mushroom => return false,
            Biome::Mountain => 200,
            Biome::Ocean => return false,
            Biome::Volcanic => return false,
            Biome::Tundra => return false,
            Biome::BirchForest => 25,
            Biome::DarkForest => 10,
            Biome::FlowerForest => 40,
        };

        (hash >> 8) % frequency == 0
    }

    fn should_place_cactus(&self, world_x: i32, world_z: i32, biome: Biome) -> bool {
        if biome != Biome::Desert && biome != Biome::Badlands && biome != Biome::Mesa {
            return false;
        }

        let hash = self.seed.wrapping_mul(6364136223846793005)
            .wrapping_add(world_x as u64 * 2654435761)
            .wrapping_add(world_z as u64 * 40503);

        let frequency = match biome {
            Biome::Mesa => 100,
            _ => 80,
        };

        (hash >> 8) % frequency == 0
    }

    fn calculate_trunk_height(&self, world_x: i32, world_z: i32, biome: Biome) -> i32 {
        let hash = self.seed.wrapping_mul(6364136223846793005)
            .wrapping_add(world_x as u64 * 2654435761)
            .wrapping_add(world_z as u64 * 40503);

        let (min_height, max_height) = match biome {
            Biome::Forest => (5, 8),  // Tall oak trees
            Biome::Plains => (4, 6),  // Medium trees
            Biome::Snowy => (6, 10),  // Tall spruce trees
            Biome::IceSpikes => (0, 0),
            Biome::SnowPeaks => (0, 0),
            Biome::Desert => (2, 4),  // Cacti (not used for trees)
            Biome::Jungle => (7, 11),
            Biome::BambooJungle => (5, 8),
            Biome::Mangrove => (4, 7),
            Biome::Swamp => (4, 7),
            Biome::CherryGrove => (4, 6),
            Biome::Savanna => (4, 6),
            Biome::Steppe => (0, 0),
            Biome::Taiga => (6, 9),
            Biome::Badlands => (2, 4),
            Biome::Mesa => (0, 0),
            Biome::Mushroom => (3, 4),
            Biome::Mountain => (5, 8),
            Biome::Ocean => (2, 3),
            Biome::Volcanic => (0, 0),
            Biome::Tundra => (3, 4),
            Biome::BirchForest => (5, 7),
            Biome::DarkForest => (6, 9),
            Biome::FlowerForest => (4, 6),
        };

        min_height + ((hash >> 16) % (max_height - min_height + 1) as u64) as i32
    }

    fn calculate_cactus_height(&self, world_x: i32, world_z: i32) -> i32 {
        let hash = self.seed.wrapping_mul(6364136223846793005)
            .wrapping_add(world_x as u64 * 2654435761)
            .wrapping_add(world_z as u64 * 40503);

        2 + ((hash >> 16) % 3) as i32  // 2, 3, or 4
    }

    fn calculate_canopy_radius(&self, world_x: i32, world_z: i32, biome: Biome) -> i32 {
        let hash = self.seed.wrapping_mul(6364136223846793005)
            .wrapping_add(world_x as u64 * 2654435761)
            .wrapping_add(world_z as u64 * 40503);

        let (min_radius, max_radius) = match biome {
            Biome::Forest => (3, 4),  // Large canopy
            Biome::Plains => (2, 3),  // Medium canopy
            Biome::Snowy => (2, 2),   // Cone base radius (not really used for sphere)
            Biome::IceSpikes => (0, 0),
            Biome::SnowPeaks => (0, 0),
            Biome::Desert => (0, 0),  // No canopy
            Biome::Jungle => (4, 5),
            Biome::BambooJungle => (2, 3),
            Biome::Mangrove => (2, 3),
            Biome::Swamp => (2, 3),
            Biome::CherryGrove => (2, 3),
            Biome::Savanna => (2, 3),
            Biome::Steppe => (0, 0),
            Biome::Taiga => (2, 2),
            Biome::Badlands => (0, 0),
            Biome::Mesa => (0, 0),
            Biome::Mushroom => (0, 0),
            Biome::Mountain => (2, 3),
            Biome::Ocean => (0, 0),
            Biome::Volcanic => (0, 0),
            Biome::Tundra => (0, 0),
            Biome::BirchForest => (2, 3),
            Biome::DarkForest => (3, 4),
            Biome::FlowerForest => (2, 3),
        };

        min_radius + ((hash >> 20) % (max_radius - min_radius + 1) as u64) as i32
    }

    fn is_cone_canopy(&self, biome: Biome) -> bool {
        matches!(biome, Biome::Snowy | Biome::Taiga)
    }

    fn calculate_surface_height(
        &self,
        terrain: &Perlin,
        world_x: i32,
        world_z: i32,
        temperature: f64,
        humidity: f64,
        biome: Biome,
    ) -> i32 {
        let wx = world_x as f64;
        let wz = world_z as f64;

        let coarse = terrain.get([wx * 0.008, wz * 0.008]);
        let detail = terrain.get([wx * 0.032 + 101.3, wz * 0.032 - 73.7]) * 0.35;
        let ridge = (1.0 - terrain.get([wx * 0.004 + 401.0, wz * 0.004 - 257.0]).abs()).powf(1.7);
        let continental = terrain.get([wx * 0.0012 - 191.0, wz * 0.0012 + 83.0]);

        let temp01 = ((temperature + 1.0) * 0.5).clamp(0.0, 1.0);
        let humid01 = ((humidity + 1.0) * 0.5).clamp(0.0, 1.0);
        let cold = (1.0 - temp01).powf(1.2);
        let hot = temp01.powf(1.1);
        let dry = (1.0 - humid01).powf(1.1);
        let wet = humid01.powf(1.1);

        let mountain_factor = (cold * dry).powf(0.8);
        let plains_factor = (1.0 - (temp01 - 0.5).abs() * 1.8).max(0.0) * (0.65 + wet * 0.35);
        let desert_factor = (hot * dry).powf(0.9);
        let ocean_factor = ((0.45 - (continental + 1.0) * 0.5) / 0.45).clamp(0.0, 1.0);

        let mut height_multiplier =
            0.75 + plains_factor * 0.45 + mountain_factor * 1.35 - desert_factor * 0.2;
        height_multiplier = height_multiplier.clamp(0.45, 2.6);

        let mut height_bias = -18.0 * ocean_factor + (wet * 2.5 - dry * 1.5);
        height_bias += match biome {
            Biome::SnowPeaks => 10.0,
            Biome::IceSpikes => 6.0,
            Biome::Mesa => 5.0,
            Biome::Volcanic => 3.0,
            Biome::Ocean => -9.0,
            Biome::Steppe => -2.0,
            Biome::Mountain => 4.0,
            _ => 0.0,
        };

        (coarse * 22.0 * height_multiplier
            + detail * 10.0 * (0.7 + plains_factor * 0.3)
            + ridge * 14.0 * (0.45 + mountain_factor * 0.9)
            + HEIGHT_OFFSET
            + height_bias)
            .round() as i32
    }

    fn is_cave(&self, cave_noise: &Perlin, world_x: i32, world_y: i32, world_z: i32) -> bool {
        let wx = world_x as f64;
        let wy = world_y as f64;
        let wz = world_z as f64;

        // Use two 3D Perlin noise functions to create worm-like tunnels
        // Use low frequency for large-scale winding
        let n1 = cave_noise.get([wx * 0.02, wy * 0.03, wz * 0.02]);
        // Use different seed (offset) for second noise to ensure uncorrelated noise
        let n2 = cave_noise.get([wx * 0.02 + 500.0, wy * 0.025 + 500.0, wz * 0.02 + 500.0]);

        // Tunnel exists where both noises are near zero
        // Squaring makes the intersection region thin (tunnel-like)
        let tunnel = n1 * n1 + n2 * n2;

        // Lower threshold = thinner tunnels, higher = wider
        // 0.006 gives tunnels about 3-4 blocks wide
        tunnel < 0.006
    }

    fn should_place_ore(
        &self,
        ore_noise: &Perlin,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        frequency: u64,
        noise_threshold: f64,
        salt: u64,
    ) -> bool {
        let wx = world_x as f64;
        let wy = world_y as f64;
        let wz = world_z as f64;

        // Thresholded 3D noise gives ore-like blobs instead of isolated single blocks.
        let base = ore_noise.get([wx * 0.08, wy * 0.08, wz * 0.08]);
        let detail = ore_noise.get([wx * 0.16 + 137.0, wy * 0.16 - 91.0, wz * 0.16 + 53.0]) * 0.25;
        let blob = (base + detail).clamp(-1.0, 1.0);

        if blob <= noise_threshold {
            return false;
        }

        let blob_weight = ((blob - noise_threshold) / (1.0 - noise_threshold)).clamp(0.0, 1.0);
        let chance = (blob_weight * 4.0) / frequency as f64;

        let hash = self.seed
            .wrapping_add(salt)
            .wrapping_mul(6364136223846793005)
            .wrapping_add((world_x as i64 as u64).wrapping_mul(1442695040888963407))
            .wrapping_add((world_y as i64 as u64).wrapping_mul(22695477))
            .wrapping_add((world_z as i64 as u64).wrapping_mul(1103515245));

        let roll = ((hash >> 11) & 0xffff) as f64 / 65535.0;
        roll < chance
    }

    fn should_place_surface_decoration(
        &self,
        world_x: i32,
        world_z: i32,
        frequency: u64,
        salt: u64,
    ) -> bool {
        let hash = self.seed
            .wrapping_add(salt)
            .wrapping_mul(6364136223846793005)
            .wrapping_add((world_x as i64 as u64).wrapping_mul(2654435761))
            .wrapping_add((world_z as i64 as u64).wrapping_mul(40503));

        (hash >> 8) % frequency == 0
    }

    pub fn generate_chunk(&self, pos: ChunkPos, registry: &BlockRegistry) -> ChunkData {
        let mut chunk = ChunkData::new_empty();

        // Initialize noise generators
        let terrain = Perlin::new(self.seed as u32);
        let biome_noise = Perlin::new((self.seed.wrapping_add(3)) as u32);
        let humidity_noise = Perlin::new((self.seed.wrapping_add(7)) as u32);
        let cave_noise = Perlin::new((self.seed + 2000) as u32);
        let coal_ore_noise = Perlin::new((self.seed + 3000) as u32);
        let copper_ore_noise = Perlin::new((self.seed + 4000) as u32);
        let iron_ore_noise = Perlin::new((self.seed + 5000) as u32);
        let gold_ore_noise = Perlin::new((self.seed + 6000) as u32);
        let diamond_ore_noise = Perlin::new((self.seed + 7000) as u32);

        let air = BlockId::AIR;
        let bedstone = registry.get_by_name("bedstone").unwrap_or(air);
        let granite = registry.get_by_name("granite").unwrap_or(air);
        let loam = registry.get_by_name("loam").unwrap_or(air);
        let verdant_turf = registry.get_by_name("verdant_turf").unwrap_or(air);
        let dune_sand = registry.get_by_name("dune_sand").unwrap_or(air);
        let snowcap = registry.get_by_name("snowcap").unwrap_or(air);
        let still_water = registry.get_by_name("still_water").unwrap_or(air);
        let lava_source = registry.get_by_name("lava_source").unwrap_or(air);
        let rubblestone = registry.get_by_name("rubblestone").unwrap_or(air);
        let iron_vein = registry.get_by_name("iron_vein").unwrap_or(air);
        let coal_vein = registry.get_by_name("coal_vein").unwrap_or(air);
        let copper_vein = registry.get_by_name("copper_vein").unwrap_or(air);
        let gold_vein = registry.get_by_name("gold_vein").unwrap_or(air);
        let diamond_vein = registry.get_by_name("diamond_vein").unwrap_or(air);
        let tall_grass = registry.get_by_name("tall_grass").unwrap_or(air);
        let wildflower = registry.get_by_name("wildflower").unwrap_or(air);
        let sugar_cane = registry.get_by_name("sugar_cane").unwrap_or(air);
        let timber_log = registry.get_by_name("timber_log").unwrap_or(air);
        let canopy_leaves = registry.get_by_name("canopy_leaves").unwrap_or(air);
        let sand_block = registry.get_by_name("sand").unwrap_or(dune_sand);
        let hardened_clay = registry.get_by_name("hardened_clay").unwrap_or(granite);
        let clay_deposit = registry.get_by_name("clay_deposit").unwrap_or(loam);
        let tuff_block = registry.get_by_name("tuff").unwrap_or(granite);
        let packed_ice = registry.get_by_name("packed_ice").unwrap_or(snowcap);
        let magma_block = registry.get_by_name("magma_block").unwrap_or(granite);
        let obsidian = registry.get_by_name("obsidian").unwrap_or(granite);

        // Pre-calculate surface heights and biomes for each column
        let mut surface_heights = [[0i32; CHUNK_SIZE]; CHUNK_SIZE];
        let mut biomes = [[Biome::Plains; CHUNK_SIZE]; CHUNK_SIZE];

        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let world_anchor = chunk_to_world(
                    pos,
                    LocalPos {
                        x: x as u8,
                        y: 0,
                        z: z as u8,
                    },
                );

                let world_x = world_anchor.x;
                let world_z = world_anchor.z;

                let (temperature, humidity) =
                    self.sample_climate(&biome_noise, &humidity_noise, world_x, world_z);
                let biome = Self::biome_from_climate(temperature, humidity);
                biomes[z][x] = biome;

                let surface_y = self.calculate_surface_height(
                    &terrain,
                    world_x,
                    world_z,
                    temperature,
                    humidity,
                    biome,
                );
                surface_heights[z][x] = surface_y;
            }
        }

        // Step 1 & 2: Generate terrain layers based on biome
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let world_anchor = chunk_to_world(
                    pos,
                    LocalPos {
                        x: x as u8,
                        y: 0,
                        z: z as u8,
                    },
                );

                let world_x = world_anchor.x;
                let world_z = world_anchor.z;
                let biome = biomes[z][x];
                let surface_y = surface_heights[z][x];
                let badlands_depth = {
                    let hash = self.seed
                        .wrapping_add(91_003)
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add((world_x as i64 as u64).wrapping_mul(1442695040888963407))
                        .wrapping_add((world_z as i64 as u64).wrapping_mul(1103515245));
                    8 + ((hash >> 18) % 5) as i32 // 8-12 blocks
                };
                let volcanic_surface_magma = {
                    let hash = self.seed
                        .wrapping_add(91_777)
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add((world_x as i64 as u64).wrapping_mul(1442695040888963407))
                        .wrapping_add((world_z as i64 as u64).wrapping_mul(1103515245));
                    (hash >> 17) % 7 == 0
                };

                for y in 0..CHUNK_SIZE {
                    let world_y = pos.y * CHUNK_SIZE as i32 + y as i32;

                    let block = if world_y <= BEDSTONE_LEVEL {
                        bedstone
                    } else if world_y < surface_y {
                        // Below surface - determine block type
                        match biome {
                            Biome::Desert => {
                                if world_y >= surface_y - 3 {
                                    sand_block
                                } else if world_y >= surface_y - 8 {
                                    hardened_clay
                                } else {
                                    granite
                                }
                            }
                            Biome::Plains | Biome::Forest => {
                                if world_y == surface_y - 1 {
                                    loam
                                } else if world_y >= surface_y - 6 {
                                    tuff_block
                                } else {
                                    granite
                                }
                            }
                            Biome::Snowy => {
                                if world_y >= surface_y - 3 {
                                    packed_ice
                                } else {
                                    granite
                                }
                            }
                            Biome::IceSpikes => {
                                if world_y >= surface_y - 6 {
                                    packed_ice
                                } else {
                                    granite
                                }
                            }
                            Biome::SnowPeaks => granite,
                            Biome::Jungle | Biome::BambooJungle | Biome::Mangrove => {
                                if world_y >= surface_y - 6 {
                                    loam
                                } else {
                                    granite
                                }
                            }
                            Biome::Swamp => {
                                if world_y >= surface_y - 6 {
                                    loam
                                } else {
                                    granite
                                }
                            }
                            Biome::CherryGrove => {
                                if world_y >= surface_y - 2 {
                                    loam
                                } else {
                                    granite
                                }
                            }
                            Biome::Savanna => {
                                if world_y >= surface_y - 6 {
                                    loam
                                } else {
                                    granite
                                }
                            }
                            Biome::Steppe => {
                                if world_y == surface_y - 1 {
                                    loam
                                } else {
                                    granite
                                }
                            }
                            Biome::Taiga => {
                                if world_y >= surface_y - 4 {
                                    granite
                                } else if world_y >= surface_y - 10 {
                                    tuff_block
                                } else {
                                    granite
                                }
                            }
                            Biome::Badlands => {
                                if world_y >= surface_y - badlands_depth {
                                    hardened_clay
                                } else {
                                    granite
                                }
                            }
                            Biome::Mesa => {
                                if world_y >= surface_y - 12 {
                                    hardened_clay
                                } else {
                                    granite
                                }
                            }
                            Biome::Mushroom => {
                                if world_y >= surface_y - 6 {
                                    clay_deposit
                                } else {
                                    granite
                                }
                            }
                            Biome::Mountain => {
                                if world_y >= surface_y - 6 {
                                    granite
                                } else if world_y >= surface_y - 12 {
                                    tuff_block
                                } else {
                                    granite
                                }
                            }
                            Biome::Ocean => {
                                if world_y >= surface_y - 6 {
                                    sand_block
                                } else {
                                    granite
                                }
                            }
                            Biome::Volcanic => {
                                if world_y >= surface_y - 4 {
                                    obsidian
                                } else {
                                    granite
                                }
                            }
                            Biome::Tundra => {
                                if world_y >= surface_y - 4 {
                                    packed_ice
                                } else {
                                    granite
                                }
                            }
                            Biome::BirchForest => {
                                if world_y == surface_y - 1 {
                                    loam
                                } else if world_y >= surface_y - 6 {
                                    tuff_block
                                } else {
                                    granite
                                }
                            }
                            Biome::DarkForest => {
                                if world_y >= surface_y - 2 {
                                    loam
                                } else if world_y >= surface_y - 6 {
                                    tuff_block
                                } else {
                                    granite
                                }
                            }
                            Biome::FlowerForest => {
                                if world_y == surface_y - 1 {
                                    loam
                                } else if world_y >= surface_y - 6 {
                                    tuff_block
                                } else {
                                    granite
                                }
                            }
                        }
                    } else if world_y == surface_y {
                        // Surface block
                        match biome {
                            Biome::Desert => sand_block,
                            Biome::Plains
                            | Biome::Forest
                            | Biome::BirchForest
                            | Biome::DarkForest
                            | Biome::FlowerForest
                            | Biome::CherryGrove => verdant_turf,
                            Biome::Snowy => snowcap,
                            Biome::IceSpikes => packed_ice,
                            Biome::SnowPeaks => snowcap,
                            Biome::Jungle | Biome::BambooJungle => verdant_turf,
                            Biome::Mangrove => {
                                if world_y <= 24 {
                                    still_water
                                } else {
                                    loam
                                }
                            }
                            Biome::Swamp => {
                                if world_y <= SEA_LEVEL + 1 {
                                    still_water
                                } else {
                                    loam
                                }
                            }
                            Biome::Savanna => dune_sand,
                            Biome::Steppe => verdant_turf,
                            Biome::Taiga => verdant_turf,
                            Biome::Badlands => hardened_clay,
                            Biome::Mesa => hardened_clay,
                            Biome::Mushroom => loam,
                            Biome::Mountain => {
                                if world_y > 40 {
                                    granite
                                } else {
                                    verdant_turf
                                }
                            }
                            Biome::Ocean => sand_block,
                            Biome::Volcanic => {
                                if volcanic_surface_magma {
                                    magma_block
                                } else {
                                    obsidian
                                }
                            }
                            Biome::Tundra => snowcap,
                        }
                    } else {
                        air
                    };

                    chunk.set(
                        LocalPos {
                            x: x as u8,
                            y: y as u8,
                            z: z as u8,
                        },
                        block,
                    );
                }
            }
        }

        // Step 3: Carve caves
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let world_anchor = chunk_to_world(
                    pos,
                    LocalPos {
                        x: x as u8,
                        y: 0,
                        z: z as u8,
                    },
                );

                let world_x = world_anchor.x;
                let world_z = world_anchor.z;
                let surface_y = surface_heights[z][x];

                for y in 0..CHUNK_SIZE {
                    let world_y = pos.y * CHUNK_SIZE as i32 + y as i32;

                    // Never carve bedstone
                    if world_y <= BEDSTONE_LEVEL {
                        continue;
                    }

                    // Never carve surface or above
                    if world_y >= surface_y {
                        continue;
                    }

                    // Only carve at least 3 blocks below surface
                    if world_y > surface_y - 3 {
                        continue;
                    }

                    // Check if this position should be a cave
                    if self.is_cave(&cave_noise, world_x, world_y, world_z) {
                        chunk.set(
                            LocalPos {
                                x: x as u8,
                                y: y as u8,
                                z: z as u8,
                            },
                            air,
                        );
                    }
                }
            }
        }

        // Step 3b: Generate depth-based ore veins in underground solid stone.
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let world_anchor = chunk_to_world(
                    pos,
                    LocalPos {
                        x: x as u8,
                        y: 0,
                        z: z as u8,
                    },
                );

                let world_x = world_anchor.x;
                let world_z = world_anchor.z;
                let surface_y = surface_heights[z][x];

                for y in 0..CHUNK_SIZE {
                    let world_y = pos.y * CHUNK_SIZE as i32 + y as i32;

                    if world_y <= BEDSTONE_LEVEL || world_y >= surface_y {
                        continue;
                    }

                    let local = LocalPos {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                    };

                    let current_block = chunk.get(local);
                    if current_block != granite && current_block != rubblestone {
                        continue;
                    }

                    let ore_block = if (-64..=-20).contains(&world_y)
                        && self.should_place_ore(
                            &diamond_ore_noise,
                            world_x,
                            world_y,
                            world_z,
                            400,
                            0.24,
                            30_005,
                        )
                    {
                        Some(diamond_vein)
                    } else if (-64..=10).contains(&world_y)
                        && self.should_place_ore(
                            &gold_ore_noise,
                            world_x,
                            world_y,
                            world_z,
                            200,
                            0.18,
                            30_004,
                        )
                    {
                        Some(gold_vein)
                    } else if (-64..=30).contains(&world_y)
                        && self.should_place_ore(
                            &iron_ore_noise,
                            world_x,
                            world_y,
                            world_z,
                            100,
                            0.10,
                            30_003,
                        )
                    {
                        Some(iron_vein)
                    } else if (-64..=40).contains(&world_y)
                        && self.should_place_ore(
                            &copper_ore_noise,
                            world_x,
                            world_y,
                            world_z,
                            120,
                            0.12,
                            30_002,
                        )
                    {
                        Some(copper_vein)
                    } else if (-64..=60).contains(&world_y)
                        && self.should_place_ore(
                            &coal_ore_noise,
                            world_x,
                            world_y,
                            world_z,
                            80,
                            0.08,
                            30_001,
                        )
                    {
                        Some(coal_vein)
                    } else if (-64..=-50).contains(&world_y) {
                        let hash = self.seed
                            .wrapping_add(30_006)
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add((world_x as i64 as u64).wrapping_mul(2654435761))
                            .wrapping_add((world_y as i64 as u64).wrapping_mul(22695477))
                            .wrapping_add((world_z as i64 as u64).wrapping_mul(1103515245));
                        if (hash >> 8) % 30 == 0 {
                            Some(magma_block)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(ore_block) = ore_block {
                        chunk.set(local, ore_block);
                    } else if (-64..=-50).contains(&world_y) && current_block == granite {
                        let hash = self.seed
                            .wrapping_add(30_006)
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add((world_x as i64 as u64).wrapping_mul(1442695040888963407))
                            .wrapping_add((world_y as i64 as u64).wrapping_mul(22695477))
                            .wrapping_add((world_z as i64 as u64).wrapping_mul(1103515245));

                        if (hash >> 8) % 30 == 0 {
                            chunk.set(local, magma_block);
                        }
                    }
                }
            }
        }

        // Step 4: Fill water below sea level
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let biome = biomes[z][x];
                let water_fill_level = if biome == Biome::Swamp {
                    SEA_LEVEL + 2
                } else {
                    SEA_LEVEL
                };
                let fill_fluid = if biome == Biome::Volcanic {
                    lava_source
                } else {
                    still_water
                };

                for y in 0..CHUNK_SIZE {
                    let world_y = pos.y * CHUNK_SIZE as i32 + y as i32;

                    if world_y <= water_fill_level {
                        let current_block = chunk.get(LocalPos {
                            x: x as u8,
                            y: y as u8,
                            z: z as u8,
                        });

                        if current_block == air {
                            chunk.set(
                                LocalPos {
                                    x: x as u8,
                                    y: y as u8,
                                    z: z as u8,
                                },
                                fill_fluid,
                            );
                        }
                    }
                }
            }
        }

        // Step 5: Generate trees and cacti based on biome
        for z in 3..29 {  // margin for canopy
            for x in 3..29 {
                let world_x = pos.x * 32 + x as i32;
                let world_z = pos.z * 32 + z as i32;
                let biome = biomes[z][x];

                // Handle cactus placement separately
                if biome == Biome::Desert || biome == Biome::Badlands || biome == Biome::Mesa {
                    if !self.should_place_cactus(world_x, world_z, biome) {
                        continue;
                    }

                    // Find biome-appropriate cactus surface
                    let mut surface_y = None;
                    for y in (0..32).rev() {
                        let block = chunk.get(LocalPos { x: x as u8, y: y as u8, z: z as u8 });
                        let is_surface = match biome {
                            Biome::Desert => block == sand_block || block == dune_sand,
                            Biome::Badlands | Biome::Mesa => block == hardened_clay,
                            _ => false,
                        };
                        if is_surface {
                            surface_y = Some(y);
                            break;
                        }
                    }

                    let Some(surface_y) = surface_y else { continue };

                    // Check if surface is above water
                    let world_surface_y = pos.y * 32 + surface_y as i32;
                    if world_surface_y <= SEA_LEVEL {
                        continue;
                    }

                    // Calculate cactus height
                    let cactus_height = self.calculate_cactus_height(world_x, world_z);

                    // Check if cactus fits in chunk
                    let top_y = surface_y as i32 + cactus_height;
                    if top_y >= 32 { continue; }

                    // Place cactus (single column using timber_log as proxy)
                    for dy in 1..=cactus_height {
                        let y = surface_y as i32 + dy;
                        if y < 32 {
                            chunk.set(
                                LocalPos { x: x as u8, y: y as u8, z: z as u8 },
                                timber_log
                            );
                        }
                    }

                    continue; // Done with this position
                }

                // Handle trees for non-cactus biomes
                if !self.should_place_tree(world_x, world_z, biome) {
                    continue;
                }

                // Find surface: scan from top down for first biome-appropriate surface block
                let mut surface_y = None;
                for y in (0..32).rev() {
                    let block = chunk.get(LocalPos { x: x as u8, y: y as u8, z: z as u8 });
                    let is_surface = match biome {
                        Biome::Plains
                        | Biome::Forest
                        | Biome::Jungle
                        | Biome::BambooJungle
                        | Biome::CherryGrove
                        | Biome::Taiga
                        | Biome::BirchForest
                        | Biome::DarkForest
                        | Biome::FlowerForest => block == verdant_turf,
                        Biome::Snowy => block == snowcap,
                        Biome::Mangrove => block == loam,
                        Biome::Swamp => block == loam,
                        Biome::Savanna => block == dune_sand,
                        Biome::Mountain => block == granite || block == verdant_turf,
                        _ => false,
                    };
                    if is_surface {
                        surface_y = Some(y);
                        break;
                    }
                }

                let Some(surface_y) = surface_y else { continue };

                // Check if surface is above water
                let world_surface_y = pos.y * 32 + surface_y as i32;
                if biome != Biome::Mangrove && world_surface_y <= SEA_LEVEL {
                    continue; // Don't place trees underwater
                }
                if biome == Biome::Mangrove && world_surface_y < SEA_LEVEL - 2 {
                    continue; // Keep mangroves to shallow coastal water.
                }

                // Calculate trunk height and canopy radius based on biome
                let trunk_height = self.calculate_trunk_height(world_x, world_z, biome);
                let canopy_radius = self.calculate_canopy_radius(world_x, world_z, biome);

                // Check if tree fits in chunk (different for cone vs sphere)
                let top_y = if self.is_cone_canopy(biome) {
                    surface_y as i32 + trunk_height + 2 // Cone extends above trunk
                } else {
                    surface_y as i32 + trunk_height + canopy_radius // Sphere canopy
                };
                if top_y >= 32 { continue; }

                // Place trunk
                for dy in 1..=trunk_height {
                    let y = surface_y as i32 + dy;
                    if y < 32 {
                        chunk.set(
                            LocalPos { x: x as u8, y: y as u8, z: z as u8 },
                            timber_log
                        );
                    }
                }

                // Place canopy based on biome
                if self.is_cone_canopy(biome) {
                    // Cone-shaped canopy for spruce trees
                    // Cone starts at 2/3 up the trunk
                    let cone_start_y = surface_y as i32 + (trunk_height * 2 / 3);
                    let cone_height = trunk_height - (trunk_height * 2 / 3) + 2;

                    for layer in 0..cone_height {
                        let layer_y = cone_start_y + layer;
                        if layer_y < 0 || layer_y >= 32 { continue; }

                        // Radius decreases as we go up
                        // Bottom layers: radius 2, middle: radius 1, top: radius 0-1
                        let layer_radius = if layer < cone_height / 2 {
                            2
                        } else if layer < cone_height - 1 {
                            1
                        } else {
                            0
                        };

                        for dz in -layer_radius..=layer_radius {
                            for dx in -layer_radius..=layer_radius {
                                // Circular cross-section at each layer
                                if dx*dx + dz*dz <= layer_radius*layer_radius + 1 {
                                    let lx = x as i32 + dx;
                                    let lz = z as i32 + dz;
                                    if lx >= 0 && lx < 32 && lz >= 0 && lz < 32 {
                                        let current_block = chunk.get(LocalPos {
                                            x: lx as u8,
                                            y: layer_y as u8,
                                            z: lz as u8
                                        });
                                        if current_block == air {
                                            chunk.set(
                                                LocalPos { x: lx as u8, y: layer_y as u8, z: lz as u8 },
                                                canopy_leaves
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Spherical canopy for Forest and Plains trees
                    let canopy_center_y = surface_y as i32 + trunk_height;
                    let radius = canopy_radius;
                    for dy in -radius..=radius {
                        for dz in -radius..=radius {
                            for dx in -radius..=radius {
                                if dx*dx + dy*dy + dz*dz <= radius*radius + 1 {
                                    let lx = x as i32 + dx;
                                    let ly = canopy_center_y + dy;
                                    let lz = z as i32 + dz;
                                    if lx >= 0 && lx < 32 && ly >= 0 && ly < 32 && lz >= 0 && lz < 32 {
                                        let current_block = chunk.get(LocalPos {
                                            x: lx as u8,
                                            y: ly as u8,
                                            z: lz as u8
                                        });
                                        if current_block == air { // only replace air
                                            chunk.set(
                                                LocalPos { x: lx as u8, y: ly as u8, z: lz as u8 },
                                                canopy_leaves
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 6: Add simple vegetation decorations on verdant turf.
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let biome = biomes[z][x];
                if biome != Biome::Plains
                    && biome != Biome::Forest
                    && biome != Biome::FlowerForest
                    && biome != Biome::Steppe
                    && biome != Biome::BambooJungle
                {
                    continue;
                }

                let world_x = pos.x * CHUNK_SIZE as i32 + x as i32;
                let world_z = pos.z * CHUNK_SIZE as i32 + z as i32;

                let mut surface_y = None;
                for y in (0..CHUNK_SIZE).rev() {
                    let block = chunk.get(LocalPos {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                    });
                    if block == verdant_turf {
                        surface_y = Some(y as i32);
                        break;
                    }
                }

                let Some(surface_y) = surface_y else { continue };
                if surface_y + 1 >= CHUNK_SIZE as i32 {
                    continue;
                }

                let world_surface_y = pos.y * CHUNK_SIZE as i32 + surface_y;
                if world_surface_y <= SEA_LEVEL {
                    continue;
                }

                let top_pos = LocalPos {
                    x: x as u8,
                    y: (surface_y + 1) as u8,
                    z: z as u8,
                };
                if chunk.get(top_pos) != air {
                    continue;
                }

                if biome == Biome::Steppe {
                    if self.should_place_surface_decoration(world_x, world_z, 24, 40_103) {
                        chunk.set(top_pos, tall_grass);
                    }
                    continue;
                }

                if biome == Biome::FlowerForest {
                    if self.should_place_surface_decoration(world_x, world_z, 8, 40_101) {
                        chunk.set(top_pos, wildflower);
                    } else if self.should_place_surface_decoration(world_x, world_z, 12, 40_102) {
                        chunk.set(top_pos, tall_grass);
                    }
                    continue;
                }

                if biome == Biome::BambooJungle {
                    if self.should_place_surface_decoration(world_x, world_z, 15, 40_104) {
                        chunk.set(top_pos, sugar_cane);
                    } else if self.should_place_surface_decoration(world_x, world_z, 8, 40_105) {
                        chunk.set(top_pos, tall_grass);
                    }
                    continue;
                }

                if self.should_place_surface_decoration(world_x, world_z, 40, 40_001) {
                    chunk.set(top_pos, wildflower);
                } else if self.should_place_surface_decoration(world_x, world_z, 10, 40_002) {
                    chunk.set(top_pos, tall_grass);
                }
            }
        }

        chunk
    }
}
