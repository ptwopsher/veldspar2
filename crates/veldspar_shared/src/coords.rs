use std::ops::{Add, AddAssign, Sub, SubAssign};

use glam::IVec3;
use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalPos {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl Add for ChunkPos {
    type Output = ChunkPos;

    fn add(self, rhs: Self) -> Self::Output {
        ChunkPos {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl AddAssign for ChunkPos {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for ChunkPos {
    type Output = ChunkPos;

    fn sub(self, rhs: Self) -> Self::Output {
        ChunkPos {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl SubAssign for ChunkPos {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

fn div_rem_floor(value: i32, divisor: i32) -> (i32, i32) {
    let mut q = value / divisor;
    let mut r = value % divisor;
    if r < 0 {
        q -= 1;
        r += divisor;
    }
    (q, r)
}

pub fn world_to_chunk(world_pos: IVec3) -> (ChunkPos, LocalPos) {
    let size = CHUNK_SIZE as i32;

    let (chunk_x, local_x) = div_rem_floor(world_pos.x, size);
    let (chunk_y, local_y) = div_rem_floor(world_pos.y, size);
    let (chunk_z, local_z) = div_rem_floor(world_pos.z, size);

    (
        ChunkPos {
            x: chunk_x,
            y: chunk_y,
            z: chunk_z,
        },
        LocalPos {
            x: local_x as u8,
            y: local_y as u8,
            z: local_z as u8,
        },
    )
}

pub fn chunk_to_world(chunk_pos: ChunkPos, local: LocalPos) -> IVec3 {
    let size = CHUNK_SIZE as i32;
    IVec3::new(
        chunk_pos.x * size + i32::from(local.x),
        chunk_pos.y * size + i32::from(local.y),
        chunk_pos.z * size + i32::from(local.z),
    )
}

pub fn local_to_index(local: LocalPos) -> usize {
    usize::from(local.x)
        + usize::from(local.z) * CHUNK_SIZE
        + usize::from(local.y) * CHUNK_SIZE * CHUNK_SIZE
}

pub fn index_to_local(index: usize) -> LocalPos {
    assert!(index < CHUNK_VOLUME, "chunk index out of bounds: {index}");

    let y = index / (CHUNK_SIZE * CHUNK_SIZE);
    let rem = index % (CHUNK_SIZE * CHUNK_SIZE);
    let z = rem / CHUNK_SIZE;
    let x = rem % CHUNK_SIZE;

    LocalPos {
        x: x as u8,
        y: y as u8,
        z: z as u8,
    }
}

#[cfg(test)]
mod tests {
    use glam::IVec3;

    use super::{
        chunk_to_world, index_to_local, local_to_index, world_to_chunk, ChunkPos, LocalPos,
        CHUNK_SIZE,
    };

    #[test]
    fn local_to_index_round_trips_back_to_local_coords() {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let local = LocalPos {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                    };
                    let index = local_to_index(local);
                    assert_eq!(index_to_local(index), local);
                }
            }
        }
    }

    #[test]
    fn chunk_pos_arithmetic_is_component_wise() {
        let a = ChunkPos { x: 10, y: -2, z: 4 };
        let b = ChunkPos { x: -3, y: 8, z: 1 };

        assert_eq!(a + b, ChunkPos { x: 7, y: 6, z: 5 });
        assert_eq!(a - b, ChunkPos { x: 13, y: -10, z: 3 });

        let mut c = a;
        c += b;
        assert_eq!(c, ChunkPos { x: 7, y: 6, z: 5 });
        c -= b;
        assert_eq!(c, a);
    }

    #[test]
    fn world_to_chunk_handles_negative_and_positive_coordinates() {
        let (chunk0, local0) = world_to_chunk(IVec3::new(-1, -1, -1));
        assert_eq!(chunk0, ChunkPos { x: -1, y: -1, z: -1 });
        assert_eq!(
            local0,
            LocalPos {
                x: (CHUNK_SIZE - 1) as u8,
                y: (CHUNK_SIZE - 1) as u8,
                z: (CHUNK_SIZE - 1) as u8,
            }
        );

        let (chunk1, local1) = world_to_chunk(IVec3::new(32, 64, 0));
        assert_eq!(chunk1, ChunkPos { x: 1, y: 2, z: 0 });
        assert_eq!(local1, LocalPos { x: 0, y: 0, z: 0 });

        let world = IVec3::new(-33, 95, 66);
        let (chunk2, local2) = world_to_chunk(world);
        assert_eq!(chunk_to_world(chunk2, local2), world);
    }
}
