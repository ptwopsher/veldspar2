use glam::Vec3;
use veldspar_shared::block::BlockId;
use veldspar_shared::coords::{world_to_chunk, ChunkPos};

use crate::camera::Camera;

fn block_display_name(id: BlockId) -> &'static str {
    match id.0 {
        1 => "Bedstone",
        2 => "Granite",
        3 => "Loam",
        4 => "Verdant Turf",
        5 => "Dune Sand",
        6 => "Timber Log",
        7 => "Hewn Plank",
        8 => "Canopy Leaves",
        9 => "Still Water",
        10 => "Rubblestone",
        11 => "Iron Vein",
        12 => "Crystal Pane",
        13 => "Kiln Brick",
        14 => "Gravel Bed",
        15 => "Snowcap",
        _ => "Unknown",
    }
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub fps: f32,
    pub position: Vec3,
    pub chunk_pos: ChunkPos,
    pub yaw: f32,
    pub pitch: f32,
    pub selected_block: BlockId,
    pub fly_mode: bool,
    pub loaded_chunks: usize,
    pub pending_chunks: usize,
    pub mesh_queue_entries: usize,
    pub game_mode: String,
    pub connection_status: String,
}

impl DebugInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn from_camera(
        camera: &Camera,
        fps: f32,
        selected_block: BlockId,
        fly_mode: bool,
        loaded_chunks: usize,
        pending_chunks: usize,
        mesh_queue_entries: usize,
        game_mode: String,
        connection_status: String,
    ) -> Self {
        let world_pos = camera.position.floor().as_ivec3();
        let (chunk_pos, _) = world_to_chunk(world_pos);

        Self {
            fps,
            position: camera.position,
            chunk_pos,
            yaw: normalize_degrees(camera.yaw.to_degrees()),
            pitch: camera.pitch.to_degrees(),
            selected_block,
            fly_mode,
            loaded_chunks,
            pending_chunks,
            mesh_queue_entries,
            game_mode,
            connection_status,
        }
    }

    pub fn facing(&self) -> &'static str {
        let yaw = self.yaw.rem_euclid(360.0);
        if yaw < 45.0 || yaw >= 315.0 {
            "N"
        } else if yaw < 135.0 {
            "E"
        } else if yaw < 225.0 {
            "S"
        } else {
            "W"
        }
    }

    pub fn window_title(&self) -> String {
        let mode = if self.fly_mode { "Fly" } else { "Walk" };
        format!(
            "Veldspar | FPS: {:.0} | XYZ: {:.1} / {:.1} / {:.1} | Chunk: {}, {}, {} | Loaded: {} Pending: {} MeshQ: {} | Mode: {} | Server: {} | Facing: {} (yaw: {:.1}, pitch: {:.1}) | Block: {} | {}",
            self.fps.round(),
            self.position.x,
            self.position.y,
            self.position.z,
            self.chunk_pos.x,
            self.chunk_pos.y,
            self.chunk_pos.z,
            self.loaded_chunks,
            self.pending_chunks,
            self.mesh_queue_entries,
            self.game_mode,
            self.connection_status,
            self.facing(),
            self.yaw,
            self.pitch,
            block_display_name(self.selected_block),
            mode
        )
    }

    pub fn overlay_lines(&self) -> Vec<String> {
        vec![
            format!("FPS: {:.1}", self.fps),
            format!(
                "POS: {:.1} {:.1} {:.1}",
                self.position.x, self.position.y, self.position.z
            ),
            format!(
                "CHUNK: {} {} {}",
                self.chunk_pos.x, self.chunk_pos.y, self.chunk_pos.z
            ),
            format!("FACING: {} (YAW {:.1} PITCH {:.1})", self.facing(), self.yaw, self.pitch),
            format!("CHUNKS LOADED: {}", self.loaded_chunks),
            format!("CHUNKS PENDING: {}", self.pending_chunks),
            format!("MESH QUEUE: {}", self.mesh_queue_entries),
            format!("MODE: {}", self.game_mode),
            format!("SERVER: {}", self.connection_status),
            format!(
                "SELECTED BLOCK: {} ({})",
                block_display_name(self.selected_block),
                self.selected_block.0
            ),
            format!("MOVEMENT: {}", if self.fly_mode { "FLY" } else { "WALK" }),
        ]
    }
}

fn normalize_degrees(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}
