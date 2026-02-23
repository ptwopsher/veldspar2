use glam::{IVec3, Vec3};
use veldspar_shared::block::BlockId;
use veldspar_shared::coords::{world_to_chunk, ChunkPos};
use veldspar_shared::physics::Face;

use crate::camera::Camera;
use crate::renderer::RenderFrameStats;

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

#[derive(Debug, Clone, Copy)]
pub struct PortalOverlayInfo {
    pub support_lower: IVec3,
    pub face: Face,
    pub linked: bool,
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
    pub portal_a: Option<PortalOverlayInfo>,
    pub portal_b: Option<PortalOverlayInfo>,
    pub portals_linked: bool,
    pub portal_teleport_count: u32,
    pub last_portal_teleport_time_s: Option<f32>,
    pub game_mode: String,
    pub connection_status: String,
    pub render_stats: RenderFrameStats,
    pub upload_bytes: u64,
    pub upload_chunks: u32,
    pub upload_reallocations: u32,
    pub frame_avg_ms: f32,
    pub frame_p95_ms: f32,
    pub frame_p99_ms: f32,
    pub frame_max_ms: f32,
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
        portal_a: Option<PortalOverlayInfo>,
        portal_b: Option<PortalOverlayInfo>,
        portals_linked: bool,
        portal_teleport_count: u32,
        last_portal_teleport_time_s: Option<f32>,
        game_mode: String,
        connection_status: String,
        render_stats: RenderFrameStats,
        upload_bytes: u64,
        upload_chunks: u32,
        upload_reallocations: u32,
        frame_avg_ms: f32,
        frame_p95_ms: f32,
        frame_p99_ms: f32,
        frame_max_ms: f32,
    ) -> Self {
        let position = camera.position;
        let (chunk_pos, _) = world_to_chunk(position.floor().as_ivec3());

        Self {
            fps,
            position,
            chunk_pos,
            yaw: normalize_degrees(camera.yaw.to_degrees()),
            pitch: camera.pitch.to_degrees(),
            selected_block,
            fly_mode,
            loaded_chunks,
            pending_chunks,
            mesh_queue_entries,
            portal_a,
            portal_b,
            portals_linked,
            portal_teleport_count,
            last_portal_teleport_time_s,
            game_mode,
            connection_status,
            render_stats,
            upload_bytes,
            upload_chunks,
            upload_reallocations,
            frame_avg_ms,
            frame_p95_ms,
            frame_p99_ms,
            frame_max_ms,
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
        let position = self.position;
        let chunk_pos = self.chunk_pos;
        let render_stats = self.render_stats;
        let facing = self.facing();
        let selected_block_name = block_display_name(self.selected_block);
        let (mode, _) = movement_mode_labels(self.fly_mode);
        format!(
            "Veldspar | FPS: {:.0} | frame ms avg/p95/p99/max {:.2}/{:.2}/{:.2}/{:.2} | draws O/W/P {}/{}/{} | XYZ: {:.1} / {:.1} / {:.1} | Chunk: {}, {}, {} | Loaded: {} Pending: {} MeshQ: {} | Mode: {} | Server: {} | Portals: {} TP:{} | Facing: {} (yaw: {:.1}, pitch: {:.1}) | Block: {} | {}",
            self.fps.round(),
            self.frame_avg_ms,
            self.frame_p95_ms,
            self.frame_p99_ms,
            self.frame_max_ms,
            render_stats.opaque_draw_calls,
            render_stats.water_draw_calls,
            render_stats.portal_draw_calls,
            position.x,
            position.y,
            position.z,
            chunk_pos.x,
            chunk_pos.y,
            chunk_pos.z,
            self.loaded_chunks,
            self.pending_chunks,
            self.mesh_queue_entries,
            self.game_mode,
            self.connection_status,
            if self.portals_linked { "linked" } else { "unlinked" },
            self.portal_teleport_count,
            facing,
            self.yaw,
            self.pitch,
            selected_block_name,
            mode
        )
    }

    pub fn overlay_lines(&self) -> Vec<String> {
        let position = self.position;
        let chunk_pos = self.chunk_pos;
        let render_stats = self.render_stats;
        let facing = self.facing();
        let selected_block_name = block_display_name(self.selected_block);
        let upload_bytes = format_bytes(self.upload_bytes);
        let (_, movement) = movement_mode_labels(self.fly_mode);
        vec![
            format!("FPS: {:.1}", self.fps),
            format!(
                "FRAME MS (AVG/P95/P99/MAX): {:.2} / {:.2} / {:.2} / {:.2}",
                self.frame_avg_ms, self.frame_p95_ms, self.frame_p99_ms, self.frame_max_ms
            ),
            format!("POS: {:.1} {:.1} {:.1}", position.x, position.y, position.z),
            format!("CHUNK: {} {} {}", chunk_pos.x, chunk_pos.y, chunk_pos.z),
            format!("FACING: {} (YAW {:.1} PITCH {:.1})", facing, self.yaw, self.pitch),
            format!("CHUNKS LOADED: {}", self.loaded_chunks),
            format!("CHUNKS PENDING: {}", self.pending_chunks),
            format!("MESH QUEUE: {}", self.mesh_queue_entries),
            format!(
                "DRAWS (OPAQUE/WATER): {} / {}",
                render_stats.opaque_draw_calls, render_stats.water_draw_calls
            ),
            format!(
                "PORTAL RENDER (DRAWS/VIEWS): {} / {}",
                render_stats.portal_draw_calls, render_stats.portal_view_passes
            ),
            format!("RENDERED CHUNKS: {}", render_stats.rendered_chunks),
            format!("RENDERED INDICES: {}", render_stats.rendered_indices),
            format!("RENDERED VERTICES: {}", render_stats.rendered_vertices),
            format!(
                "UPLOADS: {} in {} chunks (realloc: {})",
                upload_bytes,
                self.upload_chunks,
                self.upload_reallocations
            ),
            format!("PORTAL A: {}", format_portal_overlay_info(self.portal_a)),
            format!("PORTAL B: {}", format_portal_overlay_info(self.portal_b)),
            format!(
                "PORTAL LINKED: {}",
                if self.portals_linked { "yes" } else { "no" }
            ),
            format!("PORTAL TELEPORTS: {}", self.portal_teleport_count),
            format!(
                "LAST PORTAL TELEPORT: {}",
                self.last_portal_teleport_time_s
                    .map_or_else(|| "none".to_string(), |time| format!("{time:.2}s"))
            ),
            format!("MODE: {}", self.game_mode),
            format!("SERVER: {}", self.connection_status),
            format!("SELECTED BLOCK: {} ({})", selected_block_name, self.selected_block.0),
            format!("MOVEMENT: {}", movement),
        ]
    }
}

fn normalize_degrees(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

fn movement_mode_labels(fly_mode: bool) -> (&'static str, &'static str) {
    if fly_mode {
        ("Fly", "FLY")
    } else {
        ("Walk", "WALK")
    }
}

fn format_portal_overlay_info(info: Option<PortalOverlayInfo>) -> String {
    let Some(info) = info else {
        return "none".to_string();
    };

    format!(
        "{} {} {} | {} | linked={}",
        info.support_lower.x,
        info.support_lower.y,
        info.support_lower.z,
        face_label(info.face),
        if info.linked { "yes" } else { "no" }
    )
}

fn face_label(face: Face) -> &'static str {
    match face {
        Face::PosX => "+X",
        Face::NegX => "-X",
        Face::PosY => "+Y",
        Face::NegY => "-Y",
        Face::PosZ => "+Z",
        Face::NegZ => "-Z",
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    let b = bytes as f64;
    if b >= MIB {
        format!("{:.2} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.2} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}
