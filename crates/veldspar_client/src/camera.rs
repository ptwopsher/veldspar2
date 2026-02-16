use glam::{Mat4, Vec3};
use winit::keyboard::KeyCode;

use crate::input::InputState;

#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 64.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            fov: 70.0_f32.to_radians(),
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl Camera {
    pub fn update_look(&mut self, input: &InputState, look_sensitivity: f32) {
        const MAX_PITCH: f32 = 89.0_f32.to_radians();

        self.yaw += input.mouse_delta.x * look_sensitivity;
        self.pitch -= input.mouse_delta.y * look_sensitivity;
        self.pitch = self.pitch.clamp(-MAX_PITCH, MAX_PITCH);
    }

    pub fn horizontal_movement_dir(&self, input: &InputState) -> Vec3 {
        let forward = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin()).normalize_or_zero();
        let right = Vec3::new(-forward.z, 0.0, forward.x);

        let mut dir = Vec3::ZERO;
        if input.is_pressed(KeyCode::KeyW) {
            dir += forward;
        }
        if input.is_pressed(KeyCode::KeyS) {
            dir -= forward;
        }
        if input.is_pressed(KeyCode::KeyD) {
            dir += right;
        }
        if input.is_pressed(KeyCode::KeyA) {
            dir -= right;
        }

        if dir.length_squared() > 0.0 {
            dir.normalize()
        } else {
            Vec3::ZERO
        }
    }

    pub fn view_projection_matrix(&self) -> Mat4 {
        let forward = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize_or_zero();

        let view = Mat4::look_to_rh(self.position, forward, Vec3::Y);
        let projection = Mat4::perspective_rh(
            self.fov,
            self.aspect.max(0.0001),
            self.near.max(0.0001),
            self.far.max(self.near + 0.0001),
        );

        projection * view
    }

    pub fn forward_direction(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize_or_zero()
    }
}
