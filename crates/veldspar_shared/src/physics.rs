use glam::{IVec3, Vec3};

#[derive(Debug, Copy, Clone)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Face {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Face {
    pub fn normal_ivec3(&self) -> IVec3 {
        match self {
            Face::PosX => IVec3::X,
            Face::NegX => IVec3::NEG_X,
            Face::PosY => IVec3::Y,
            Face::NegY => IVec3::NEG_Y,
            Face::PosZ => IVec3::Z,
            Face::NegZ => IVec3::NEG_Z,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RaycastIter {
    current: IVec3,
    step: IVec3,
    t_max: Vec3,
    t_delta: Vec3,
    max_distance: f32,
    started: bool,
    finished: bool,
    last_face: Face,
}

impl RaycastIter {
    fn new(ray: &Ray, max_distance: f32) -> Self {
        let direction = ray.direction;
        let step = IVec3::new(
            if direction.x > 0.0 {
                1
            } else if direction.x < 0.0 {
                -1
            } else {
                0
            },
            if direction.y > 0.0 {
                1
            } else if direction.y < 0.0 {
                -1
            } else {
                0
            },
            if direction.z > 0.0 {
                1
            } else if direction.z < 0.0 {
                -1
            } else {
                0
            },
        );

        let current = ray.origin.floor().as_ivec3();

        let next_x = if step.x > 0 {
            current.x as f32 + 1.0
        } else {
            current.x as f32
        };
        let next_y = if step.y > 0 {
            current.y as f32 + 1.0
        } else {
            current.y as f32
        };
        let next_z = if step.z > 0 {
            current.z as f32 + 1.0
        } else {
            current.z as f32
        };

        let t_max = Vec3::new(
            if direction.x != 0.0 {
                (next_x - ray.origin.x) / direction.x
            } else {
                f32::INFINITY
            },
            if direction.y != 0.0 {
                (next_y - ray.origin.y) / direction.y
            } else {
                f32::INFINITY
            },
            if direction.z != 0.0 {
                (next_z - ray.origin.z) / direction.z
            } else {
                f32::INFINITY
            },
        );

        let t_delta = Vec3::new(
            if direction.x != 0.0 {
                1.0 / direction.x.abs()
            } else {
                f32::INFINITY
            },
            if direction.y != 0.0 {
                1.0 / direction.y.abs()
            } else {
                f32::INFINITY
            },
            if direction.z != 0.0 {
                1.0 / direction.z.abs()
            } else {
                f32::INFINITY
            },
        );

        Self {
            current,
            step,
            t_max,
            t_delta,
            max_distance: max_distance.max(0.0),
            started: false,
            finished: false,
            last_face: Face::NegY,
        }
    }
}

impl Iterator for RaycastIter {
    type Item = (IVec3, Face);

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        if !self.started {
            self.started = true;
            return Some((self.current, self.last_face));
        }

        let (axis, distance) = if self.t_max.x <= self.t_max.y && self.t_max.x <= self.t_max.z {
            (0usize, self.t_max.x)
        } else if self.t_max.y <= self.t_max.z {
            (1usize, self.t_max.y)
        } else {
            (2usize, self.t_max.z)
        };

        if !distance.is_finite() || distance > self.max_distance {
            self.finished = true;
            return None;
        }

        match axis {
            0 => {
                self.current.x += self.step.x;
                self.t_max.x += self.t_delta.x;
                self.last_face = if self.step.x > 0 {
                    Face::NegX
                } else {
                    Face::PosX
                };
            }
            1 => {
                self.current.y += self.step.y;
                self.t_max.y += self.t_delta.y;
                self.last_face = if self.step.y > 0 {
                    Face::NegY
                } else {
                    Face::PosY
                };
            }
            _ => {
                self.current.z += self.step.z;
                self.t_max.z += self.t_delta.z;
                self.last_face = if self.step.z > 0 {
                    Face::NegZ
                } else {
                    Face::PosZ
                };
            }
        }

        Some((self.current, self.last_face))
    }
}

pub fn raycast_blocks(ray: &Ray, max_distance: f32) -> impl Iterator<Item = (IVec3, Face)> {
    RaycastIter::new(ray, max_distance)
}

#[cfg(test)]
mod tests {
    use glam::{IVec3, Vec3};

    use super::{raycast_blocks, AABB, Face, Ray};

    #[test]
    fn aabb_collision_detection() {
        let a = AABB {
            min: Vec3::new(0.0, 0.0, 0.0),
            max: Vec3::new(1.0, 1.0, 1.0),
        };
        let b = AABB {
            min: Vec3::new(0.5, 0.25, 0.5),
            max: Vec3::new(1.5, 1.25, 1.5),
        };
        let c = AABB {
            min: Vec3::new(1.0, 1.0, 1.0),
            max: Vec3::new(2.0, 2.0, 2.0),
        };

        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn raycast_returns_expected_voxels_and_faces() {
        let ray = Ray {
            origin: Vec3::new(0.5, 0.5, 0.5),
            direction: Vec3::X,
        };

        let visited: Vec<(IVec3, Face)> = raycast_blocks(&ray, 2.1).take(5).collect();
        assert_eq!(
            visited,
            vec![
                (IVec3::new(0, 0, 0), Face::NegY),
                (IVec3::new(1, 0, 0), Face::NegX),
                (IVec3::new(2, 0, 0), Face::NegX),
            ]
        );
    }
}
