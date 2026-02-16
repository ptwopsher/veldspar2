use std::collections::HashSet;

use glam::Vec2;
use winit::keyboard::KeyCode;

#[derive(Debug, Default)]
pub struct InputState {
    pressed_keys: HashSet<KeyCode>,
    pub mouse_delta: Vec2,
    pub left_click: bool,
    pub right_click: bool,
}

impl InputState {
    pub fn press_key(&mut self, key: KeyCode) {
        self.pressed_keys.insert(key);
    }

    pub fn release_key(&mut self, key: KeyCode) {
        self.pressed_keys.remove(&key);
    }

    pub fn is_pressed(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    pub fn add_mouse_delta(&mut self, delta: Vec2) {
        self.mouse_delta += delta;
    }

    pub fn clear_frame(&mut self) {
        self.mouse_delta = Vec2::ZERO;
    }

    pub fn consume_right_click(&mut self) -> bool {
        let value = self.right_click;
        self.right_click = false;
        value
    }
}
