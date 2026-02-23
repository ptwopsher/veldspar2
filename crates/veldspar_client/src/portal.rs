use std::collections::HashMap;
use std::f32::consts::PI;

use glam::{IVec3, Mat3, Vec2, Vec3};
use veldspar_shared::block::{is_lava_block, is_water_block, BlockId};
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::{world_to_chunk, ChunkPos};
use veldspar_shared::physics::Face;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalColor {
    Orange,
    Blue,
}

impl PortalColor {
    pub fn index(self) -> usize {
        match self {
            Self::Orange => 0,
            Self::Blue => 1,
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::Orange => Self::Blue,
            Self::Blue => Self::Orange,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PortalGunState {
    pub mode: PortalColor,
    pub last_shot_time_s: f32,
}

impl Default for PortalGunState {
    fn default() -> Self {
        Self {
            mode: PortalColor::Blue,
            last_shot_time_s: -999.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Portal {
    pub color: PortalColor,
    pub support_lower: IVec3,
    pub support_upper: IVec3,
    pub face: Face,
    pub normal: IVec3,
    pub up: IVec3,
    pub right: IVec3,
    pub center: Vec3,
    pub half_extents: Vec2,
    pub linked_to: Option<PortalColor>,
}

impl Portal {
    pub fn normal_f32(&self) -> Vec3 {
        self.normal.as_vec3()
    }

    pub fn up_f32(&self) -> Vec3 {
        self.up.as_vec3()
    }

    pub fn right_f32(&self) -> Vec3 {
        self.right.as_vec3()
    }

    pub fn frame_cells(&self) -> Vec<IVec3> {
        let up_offset = self.up;
        [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (-1, 2),
            (1, -1),
            (1, 0),
            (1, 1),
            (1, 2),
            (0, -1),
            (0, 2),
        ]
        .into_iter()
        .map(|(x, y)| self.support_lower + self.right * x + up_offset * y)
        .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TeleportDebounce {
    pub blocked_until_s: f32,
    pub exit_portal: PortalColor,
    pub min_exit_distance: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TeleportResult {
    pub new_pos: Vec3,
    pub new_vel: Vec3,
    pub new_yaw: f32,
    pub new_pitch: f32,
}

pub trait PortalChunksAccessor {
    fn block_at(&self, pos: IVec3) -> Option<BlockId>;
    fn is_block_solid(&self, pos: IVec3) -> bool;
}

#[derive(Debug, Default)]
pub struct PortalManager {
    pub portals: [Option<Portal>; 2],
    pub debounce: Option<TeleportDebounce>,
    pub camera_forward: Vec3,
}

impl PortalManager {
    pub fn set_camera_forward(&mut self, forward: Vec3) {
        self.camera_forward = forward.normalize_or_zero();
    }

    pub fn place_portal<A: PortalChunksAccessor>(
        &mut self,
        color: PortalColor,
        support_lower: IVec3,
        face: Face,
        chunks_accessor: &A,
    ) -> bool {
        let normal = face.normal_ivec3();
        let up = Self::up_axis_for_face(face);
        let right = up.cross(normal);
        if right == IVec3::ZERO {
            return false;
        }

        let support_upper = support_lower + up;
        let support_blocks = [support_lower, support_upper];
        let opening_blocks = [support_lower + normal, support_upper + normal];

        for support in support_blocks {
            let Some(block) = chunks_accessor.block_at(support) else {
                return false;
            };
            if block == BlockId::AIR
                || is_water_block(block)
                || is_lava_block(block)
                || !chunks_accessor.is_block_solid(support)
            {
                return false;
            }
        }

        for opening in opening_blocks {
            let Some(block) = chunks_accessor.block_at(opening) else {
                return false;
            };
            if block != BlockId::AIR || is_water_block(block) || is_lava_block(block) {
                return false;
            }
        }

        if let Some(other) = self.get_portal(color.other()) {
            if Self::candidate_overlaps_portal(support_lower, face, other) {
                return false;
            }
        }

        let center = Self::compute_portal_center(support_lower, normal, up);
        self.portals[color.index()] = Some(Portal {
            color,
            support_lower,
            support_upper,
            face,
            normal,
            up,
            right,
            center,
            half_extents: Vec2::new(0.5, 1.0),
            linked_to: None,
        });

        if self
            .debounce
            .is_some_and(|debounce| debounce.exit_portal == color)
        {
            self.debounce = None;
        }
        self.refresh_links();
        true
    }

    pub fn remove_portal(&mut self, color: PortalColor) {
        self.portals[color.index()] = None;
        if self
            .debounce
            .is_some_and(|debounce| debounce.exit_portal == color)
        {
            self.debounce = None;
        }
        self.refresh_links();
    }

    pub fn get_portal(&self, color: PortalColor) -> Option<&Portal> {
        self.portals[color.index()].as_ref()
    }

    pub fn get_linked_pair(&self) -> Option<(&Portal, &Portal)> {
        let orange = self.get_portal(PortalColor::Orange)?;
        let blue = self.get_portal(PortalColor::Blue)?;
        Some((orange, blue))
    }

    pub fn is_destination_chunk_loaded(
        &self,
        destination: PortalColor,
        chunks: &HashMap<ChunkPos, ChunkData>,
    ) -> bool {
        let Some(portal) = self.get_portal(destination) else {
            return false;
        };

        Self::portal_blocks(portal)
            .into_iter()
            .all(|block_pos| chunks.contains_key(&world_to_chunk(block_pos).0))
    }

    pub fn invalidate_for_block_change<A: PortalChunksAccessor>(
        &mut self,
        pos: IVec3,
        chunks_accessor: &A,
    ) {
        let mut removed_any = false;
        for color in [PortalColor::Orange, PortalColor::Blue] {
            let should_remove = self
                .get_portal(color)
                .is_some_and(|portal| {
                    Self::portal_should_invalidate_for_change(portal, pos, chunks_accessor)
                });
            if should_remove {
                self.portals[color.index()] = None;
                removed_any = true;
            }
        }

        if removed_any {
            self.refresh_links();
            if self.debounce.is_some_and(|debounce| {
                self.portals[debounce.exit_portal.index()].is_none()
            }) {
                self.debounce = None;
            }
        }
    }

    pub fn check_and_teleport(
        &mut self,
        prev_eye: Vec3,
        curr_eye: Vec3,
        velocity: Vec3,
        time_s: f32,
        chunks: &HashMap<ChunkPos, ChunkData>,
    ) -> Option<TeleportResult> {
        const PORTAL_CROSS_EPS: f32 = 0.001;
        const PORTAL_BOUNDS_MARGIN: f32 = 0.3;
        const EXIT_PUSH_DISTANCE: f32 = 0.6;
        const TELEPORT_DEBOUNCE_SECS: f32 = 0.15;
        const MIN_EXIT_DISTANCE: f32 = 0.25;

        if let Some(debounce) = self.debounce {
            if time_s >= debounce.blocked_until_s {
                let clear_block = match self.get_portal(debounce.exit_portal) {
                    Some(exit_portal) => {
                        let signed_dist =
                            (curr_eye - exit_portal.center).dot(exit_portal.normal.as_vec3());
                        signed_dist.abs() >= debounce.min_exit_distance
                    }
                    None => true,
                };
                if clear_block {
                    self.debounce = None;
                }
            }
        }

        if self.debounce.is_some() {
            return None;
        }

        let Some((orange, blue)) = self.get_linked_pair() else {
            return None;
        };
        let orange = orange.clone();
        let blue = blue.clone();

        for (entry, exit) in [(&orange, &blue), (&blue, &orange)] {
            if !self.is_destination_chunk_loaded(exit.color, chunks) {
                continue;
            }

            let Some(result) = Self::teleport_between(
                entry,
                exit,
                prev_eye,
                curr_eye,
                velocity,
                self.camera_forward,
                PORTAL_CROSS_EPS,
                PORTAL_BOUNDS_MARGIN,
                EXIT_PUSH_DISTANCE,
            ) else {
                continue;
            };

            self.debounce = Some(TeleportDebounce {
                blocked_until_s: time_s + TELEPORT_DEBOUNCE_SECS,
                exit_portal: exit.color,
                min_exit_distance: MIN_EXIT_DISTANCE,
            });
            return Some(result);
        }

        None
    }

    fn teleport_between(
        entry: &Portal,
        exit: &Portal,
        prev_eye: Vec3,
        curr_eye: Vec3,
        velocity: Vec3,
        camera_forward: Vec3,
        portal_cross_eps: f32,
        portal_bounds_margin: f32,
        exit_push_distance: f32,
    ) -> Option<TeleportResult> {
        let entry_normal = entry.normal.as_vec3();
        let d_prev = (prev_eye - entry.center).dot(entry_normal);
        let d_curr = (curr_eye - entry.center).dot(entry_normal);
        if !(d_prev > portal_cross_eps && d_curr <= portal_cross_eps) {
            return None;
        }

        let denom = d_prev - d_curr;
        if denom.abs() <= f32::EPSILON {
            return None;
        }

        let t = (d_prev / denom).clamp(0.0, 1.0);
        let hit_point = prev_eye.lerp(curr_eye, t);
        let hit_local = hit_point - entry.center;
        let local_x = hit_local.dot(entry.right_f32());
        let local_y = hit_local.dot(entry.up_f32());
        if local_x.abs() > entry.half_extents.x + portal_bounds_margin
            || local_y.abs() > entry.half_extents.y + portal_bounds_margin
        {
            return None;
        }

        let a_basis = Mat3::from_cols(entry.right_f32(), entry.up_f32(), entry.normal_f32());
        let b_basis = Mat3::from_cols(exit.right_f32(), exit.up_f32(), exit.normal_f32());
        let rot = b_basis * Mat3::from_rotation_y(PI) * a_basis.transpose();

        let new_pos =
            exit.center + rot * (curr_eye - entry.center) + exit.normal_f32() * exit_push_distance;
        let new_vel = rot * velocity;

        let travel_forward = (curr_eye - prev_eye).normalize_or_zero();
        let pre_forward = if camera_forward.length_squared() > 1.0e-6 {
            camera_forward.normalize()
        } else if velocity.length_squared() > 1.0e-6 {
            velocity.normalize()
        } else if travel_forward.length_squared() > 1.0e-6 {
            travel_forward
        } else {
            -entry.normal_f32()
        };
        let new_forward = (rot * pre_forward).normalize_or_zero();
        let new_yaw = new_forward.z.atan2(new_forward.x);
        let new_pitch = new_forward.y.clamp(-1.0, 1.0).asin();

        Some(TeleportResult {
            new_pos,
            new_vel,
            new_yaw,
            new_pitch,
        })
    }

    fn refresh_links(&mut self) {
        let orange_idx = PortalColor::Orange.index();
        let blue_idx = PortalColor::Blue.index();

        if self.portals[orange_idx].is_some() && self.portals[blue_idx].is_some() {
            if let Some(orange) = self.portals[orange_idx].as_mut() {
                orange.linked_to = Some(PortalColor::Blue);
            }
            if let Some(blue) = self.portals[blue_idx].as_mut() {
                blue.linked_to = Some(PortalColor::Orange);
            }
            return;
        }

        if let Some(orange) = self.portals[orange_idx].as_mut() {
            orange.linked_to = None;
        }
        if let Some(blue) = self.portals[blue_idx].as_mut() {
            blue.linked_to = None;
        }
    }

    fn up_axis_for_face(face: Face) -> IVec3 {
        match face {
            Face::PosY | Face::NegY => IVec3::Z,
            _ => IVec3::Y,
        }
    }

    fn compute_portal_center(support_lower: IVec3, normal: IVec3, up: IVec3) -> Vec3 {
        support_lower.as_vec3()
            + Vec3::splat(0.5)
            + normal.as_vec3() * 0.5
            + up.as_vec3() * 0.5
    }

    fn candidate_overlaps_portal(
        support_lower: IVec3,
        face: Face,
        other: &Portal,
    ) -> bool {
        const PLANE_EPSILON: f32 = 0.001;
        const OVERLAP_EPSILON: f32 = 0.0001;

        if face != other.face {
            return false;
        }

        let normal = face.normal_ivec3();
        let up = Self::up_axis_for_face(face);
        let right = up.cross(normal);
        if right == IVec3::ZERO {
            return false;
        }

        let candidate_center = Self::compute_portal_center(support_lower, normal, up);
        let delta = other.center - candidate_center;

        if delta.dot(normal.as_vec3()).abs() > PLANE_EPSILON {
            return false;
        }

        let sep_right = delta.dot(right.as_vec3()).abs();
        let sep_up = delta.dot(up.as_vec3()).abs();
        let candidate_half_extents = Vec2::new(0.5, 1.0);

        sep_right + OVERLAP_EPSILON < candidate_half_extents.x + other.half_extents.x
            && sep_up + OVERLAP_EPSILON < candidate_half_extents.y + other.half_extents.y
    }

    fn portal_should_invalidate_for_change<A: PortalChunksAccessor>(
        portal: &Portal,
        pos: IVec3,
        chunks_accessor: &A,
    ) -> bool {
        if portal.support_lower == pos || portal.support_upper == pos {
            return true;
        }

        if !Self::portal_opening_blocks(portal).contains(&pos) {
            return false;
        }

        let Some(block) = chunks_accessor.block_at(pos) else {
            return true;
        };
        block != BlockId::AIR || is_water_block(block) || is_lava_block(block)
    }

    fn portal_blocks(portal: &Portal) -> [IVec3; 4] {
        let opening = Self::portal_opening_blocks(portal);
        [
            portal.support_lower,
            portal.support_upper,
            opening[0],
            opening[1],
        ]
    }

    fn portal_opening_blocks(portal: &Portal) -> [IVec3; 2] {
        [
            portal.support_lower + portal.normal,
            portal.support_upper + portal.normal,
        ]
    }
}
