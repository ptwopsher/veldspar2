use serde::{Deserialize, Serialize};

use crate::inventory::ItemId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobType {
    Chicken,
    Pig,
    Cow,
    Zombie,
    Skeleton,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobAiState {
    Idle,
    Wandering,
    Chasing,
    Fleeing,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MobData {
    pub mob_type: MobType,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub ai_state: MobAiState,
    pub ai_timer: f32,
    pub attack_cooldown: f32,
    pub wander_target: Option<[f32; 3]>,
    pub hurt_timer: f32,
}

impl MobData {
    pub fn new(mob_type: MobType, position: [f32; 3]) -> Self {
        let props = mob_properties(mob_type);
        Self {
            mob_type,
            position,
            velocity: [0.0; 3],
            yaw: 0.0,
            health: props.max_health,
            max_health: props.max_health,
            ai_state: MobAiState::Idle,
            ai_timer: 0.0,
            attack_cooldown: 0.0,
            wander_target: None,
            hurt_timer: 0.0,
        }
    }

    pub fn is_hostile(&self) -> bool {
        mob_properties(self.mob_type).hostile
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }
}

pub struct MobProperties {
    pub max_health: f32,
    pub speed: f32,
    pub hostile: bool,
    pub attack_damage: f32,
    pub attack_range: f32,
    pub detection_range: f32,
    pub drops: &'static [(ItemId, u8, u8)], // (item, min, max)
    pub width: f32,                          // hitbox
    pub height: f32,                         // hitbox
}

pub fn mob_properties(mob_type: MobType) -> MobProperties {
    match mob_type {
        MobType::Chicken => MobProperties {
            max_health: 4.0,
            speed: 1.5,
            hostile: false,
            attack_damage: 0.0,
            attack_range: 0.0,
            detection_range: 0.0,
            drops: &[], // no drops for now
            width: 0.4,
            height: 0.7,
        },
        MobType::Pig => MobProperties {
            max_health: 10.0,
            speed: 1.2,
            hostile: false,
            attack_damage: 0.0,
            attack_range: 0.0,
            detection_range: 0.0,
            drops: &[],
            width: 0.6,
            height: 0.9,
        },
        MobType::Cow => MobProperties {
            max_health: 10.0,
            speed: 1.0,
            hostile: false,
            attack_damage: 0.0,
            attack_range: 0.0,
            detection_range: 0.0,
            drops: &[],
            width: 0.7,
            height: 1.4,
        },
        MobType::Zombie => MobProperties {
            max_health: 20.0,
            speed: 1.8,
            hostile: true,
            attack_damage: 3.0,
            attack_range: 1.5,
            detection_range: 24.0,
            drops: &[],
            width: 0.6,
            height: 1.8,
        },
        MobType::Skeleton => MobProperties {
            max_health: 20.0,
            speed: 1.8,
            hostile: true,
            attack_damage: 2.0,
            attack_range: 12.0,
            detection_range: 24.0,
            drops: &[],
            width: 0.6,
            height: 1.8,
        },
    }
}

pub fn mob_color(mob_type: MobType) -> [f32; 3] {
    match mob_type {
        MobType::Chicken => [1.0, 1.0, 1.0],   // white
        MobType::Pig => [0.95, 0.7, 0.7],      // pink
        MobType::Cow => [0.55, 0.35, 0.2],     // brown
        MobType::Zombie => [0.3, 0.6, 0.3],    // green
        MobType::Skeleton => [0.85, 0.85, 0.8], // bone white
    }
}
