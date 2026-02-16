use bitflags::bitflags;
use glam::{IVec3, Vec3};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::block::BlockId;
use crate::coords::{ChunkPos, LocalPos};

pub const PROTOCOL_VERSION: u32 = 1;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct PlayerInputFlags: u8 {
        const SPRINTING = 0b0000_0001;
        const SNEAKING  = 0b0000_0010;
        const JUMPING   = 0b0000_0100;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum C2S {
    Handshake {
        protocol_version: u32,
        username: String,
    },
    RequestChunks {
        positions: Vec<ChunkPos>,
    },
    BlockEdit {
        world_pos: IVec3,
        new_block: BlockId,
    },
    PlayerInput {
        tick: u64,
        position: Vec3,
        yaw: f32,
        pitch: f32,
        flags: u8,
        attack_animation: f32,
        breaking_block: Option<IVec3>,
        break_progress: f32,
    },
    Chat {
        message: String,
    },
    Disconnect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum S2C {
    HandshakeAccept {
        player_id: u64,
        spawn_position: Vec3,
        world_seed: u64,
        tick_rate: u32,
    },
    HandshakeReject {
        reason: String,
    },
    ChunkData {
        pos: ChunkPos,
        data: Vec<u8>,
        format_version: u8,
    },
    ChunkUnload {
        pos: ChunkPos,
    },
    ChunkDelta {
        pos: ChunkPos,
        changes: Vec<(LocalPos, BlockId)>,
    },
    BlockEditConfirm {
        world_pos: IVec3,
        block: BlockId,
    },
    BlockEditReject {
        world_pos: IVec3,
        reason: String,
    },
    PlayerJoined {
        player_id: u64,
        username: String,
        position: Vec3,
    },
    PlayerLeft {
        player_id: u64,
    },
    PlayerStates {
        tick: u64,
        states: Vec<PlayerSnapshot>,
    },
    Chat {
        sender_id: u64,
        sender_name: String,
        message: String,
    },
    TimeSync {
        tick: u64,
        time_of_day: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub player_id: u64,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,
    pub attack_animation: f32,
    pub breaking_block: Option<IVec3>,
    pub break_progress: f32,
}

pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    bincode::serialize(msg).expect("failed to encode protocol payload")
}

pub fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(data)
}

#[cfg(test)]
mod tests {
    use glam::{IVec3, Vec3};

    use super::{decode, encode, C2S, PlayerSnapshot, S2C};
    use crate::block::BlockId;
    use crate::coords::{ChunkPos, LocalPos};

    #[test]
    fn c2s_round_trip_serialization() {
        let msg = C2S::RequestChunks {
            positions: vec![
                ChunkPos { x: 1, y: 0, z: -2 },
                ChunkPos { x: 3, y: -1, z: 4 },
            ],
        };
        let bytes = encode(&msg);
        let decoded: C2S = decode(&bytes).expect("decode C2S request chunks");
        assert_eq!(decoded, msg);

        let edit = C2S::BlockEdit {
            world_pos: IVec3::new(-11, 70, 23),
            new_block: BlockId(10),
        };
        let bytes = encode(&edit);
        let decoded: C2S = decode(&bytes).expect("decode C2S block edit");
        assert_eq!(decoded, edit);

        let input = C2S::PlayerInput {
            tick: 12,
            position: Vec3::new(4.0, 65.2, -7.5),
            yaw: 0.7,
            pitch: -0.3,
            flags: 0b0000_0010,
            attack_animation: 0.9,
            breaking_block: Some(IVec3::new(5, 65, -8)),
            break_progress: 0.25,
        };
        let bytes = encode(&input);
        let decoded: C2S = decode(&bytes).expect("decode C2S player input");
        assert_eq!(decoded, input);
    }

    #[test]
    fn s2c_round_trip_serialization() {
        let delta = S2C::ChunkDelta {
            pos: ChunkPos { x: -2, y: 1, z: 5 },
            changes: vec![
                (LocalPos { x: 1, y: 2, z: 3 }, BlockId(4)),
                (
                    LocalPos {
                        x: 31,
                        y: 31,
                        z: 31,
                    },
                    BlockId(0),
                ),
            ],
        };
        let bytes = encode(&delta);
        let decoded: S2C = decode(&bytes).expect("decode S2C chunk delta");
        assert_eq!(decoded, delta);

        let states = S2C::PlayerStates {
            tick: 42,
            states: vec![PlayerSnapshot {
                player_id: 7,
                position: Vec3::new(1.0, 64.5, -9.25),
                yaw: 1.3,
                pitch: -0.5,
                flags: 0b0000_0011,
                attack_animation: 0.7,
                breaking_block: Some(IVec3::new(12, 64, -3)),
                break_progress: 0.5,
            }],
        };
        let bytes = encode(&states);
        let decoded: S2C = decode(&bytes).expect("decode S2C player states");
        assert_eq!(decoded, states);
    }
}
