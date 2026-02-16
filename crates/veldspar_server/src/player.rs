use glam::{IVec3, Vec3};

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub player_id: u64,
    pub username: String,
    pub position: Vec3,
    pub last_position_tick: u64,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,
    pub attack_animation: f32,
    pub breaking_block: Option<IVec3>,
    pub break_progress: f32,
}

impl PlayerState {
    pub fn new(player_id: u64, username: impl Into<String>) -> Self {
        Self {
            player_id,
            username: username.into(),
            position: Vec3::ZERO,
            last_position_tick: 0,
            yaw: 0.0,
            pitch: 0.0,
            flags: 0,
            attack_animation: 0.0,
            breaking_block: None,
            break_progress: 0.0,
        }
    }
}
