use glam::Vec3;

#[derive(Debug, Clone)]
pub struct Player {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}
