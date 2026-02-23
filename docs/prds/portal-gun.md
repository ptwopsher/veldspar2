# Portal Gun System Implementation Plan (Veldspar)

## Context Anchors From Current Code

1. Main gameplay loop is `ClientApp::update_and_render` in `app.rs:6611`.
2. Block targeting/placement hooks via `raycast_blocks` and click handling in `app.rs:7136` and `app.rs:7211`.
3. Collision and AABB utilities in `app.rs:9039`, `app.rs:9416`, `app.rs:9465`.
4. Render pass order in `Renderer::render_frame` in `renderer/mod.rs:770`.
5. Chunk/water draw: `render_chunks` and `render_visible_transparent_chunks` in `chunk_renderer.rs:151` and `chunk_renderer.rs:197`.
6. Frustum culling currently disabled: `FRUSTUM_CULLING_ENABLED: false` in `chunk_renderer.rs:12`.
7. Depth format: `Depth32Float` in `renderer/mod.rs:53`.
8. Shared world/block/physics types in `crates/veldspar_shared`.

---

## A. Data Structures

### portal.rs (new file)

```rust
use glam::{IVec3, Mat3, Vec2, Vec3};
use veldspar_shared::physics::Face;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalColor { Orange, Blue }

impl PortalColor {
    pub fn index(self) -> usize {
        match self { Self::Orange => 0, Self::Blue => 1 }
    }
    pub fn other(self) -> Self {
        match self { Self::Orange => Self::Blue, Self::Blue => Self::Orange }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PortalGunState {
    pub mode: PortalColor,
    pub last_shot_time_s: f32,
}

#[derive(Debug, Clone)]
pub struct Portal {
    pub color: PortalColor,
    pub support_lower: IVec3,       // solid block behind bottom half
    pub support_upper: IVec3,       // solid block behind top half
    pub face: Face,                 // source placement face
    pub normal: IVec3,              // outward from support surface
    pub up: IVec3,                  // portal local up axis
    pub right: IVec3,               // portal local right axis
    pub center: Vec3,               // world center of 1x2 opening
    pub half_extents: Vec2,         // x=0.5, y=1.0
    pub linked_to: Option<PortalColor>,
}

#[derive(Debug, Clone, Copy)]
pub struct TeleportDebounce {
    pub blocked_until_s: f32,
    pub exit_portal: PortalColor,
    pub min_exit_distance: f32,
}

#[derive(Debug, Default)]
pub struct PortalManager {
    pub portals: [Option<Portal>; 2],
    pub debounce: Option<TeleportDebounce>,
}
```

### Add to ClientApp:
- `portal_gun: PortalGunState`
- `portal_manager: PortalManager`
- `prev_eye_pos: Vec3`

---

## B. Placement System

1. **Input**: Left click = Blue, Right click = Orange (when holding portal gun)
2. **Item**: Add `ItemId::PORTAL_GUN` in `veldspar_shared/src/inventory.rs`
3. **Raycast**: Reuse `Ray`, `raycast_blocks`, `Face::normal_ivec3` — range 32.0
4. **Validation**:
   - 2 solid blocks behind opening (`is_block_solid`)
   - 2 air blocks in front (`block_at`)
   - Reject water/lava/non-solid/unloaded
5. **Orientation**:
   - Wall faces: `up = IVec3::Y`
   - Floor/ceiling: `up` from cardinalized camera yaw
6. **Replacement**: Same color always replaced, link when both exist
7. **Invalidation**: Remove portal when support blocks change (hook `remesh_for_block_change`, chunk unload)

---

## C. Rendering (Critical)

### Technique: Render-to-Texture

1. Two offscreen color+depth textures (Orange view, Blue view)
2. Format: same as `surface_config.format`, usage: `RENDER_ATTACHMENT | TEXTURE_BINDING`
3. Per-view camera uniform buffers (slot 0=main, 1=orange, 2=blue)

### Shaders:
- `portal_surface.wgsl` — samples linked portal RTT, fallback color if unlinked
- `portal_frame.wgsl` — emissive orange/blue frame glow

### Pipeline state:
- `depth_write_enabled: false`, `depth_compare: LessEqual`, alpha blending

### Render order:
Sky → Clouds → Opaque → **Portal** → Water → Particles → FirstPersonHand → UI

### Offscreen portal-view rendering:
- Compute camera transformed through source→destination portal
- Build frustum, render sky/clouds/opaque/water into RTT
- Visibility gate: portal faces camera + quad intersects frustum

### Oblique near-plane clipping:
```rust
fn apply_oblique_clip(mut proj: Mat4, clip_plane_camera: Vec4) -> Mat4 {
    let q = proj.inverse() * Vec4::new(
        clip_plane_camera.x.signum(),
        clip_plane_camera.y.signum(),
        1.0, 1.0,
    );
    let c = clip_plane_camera * (2.0 / clip_plane_camera.dot(q));
    let mut m = proj.to_cols_array_2d();
    m[0][2] = c.x - m[0][3];
    m[1][2] = c.y - m[1][3];
    m[2][2] = c.z - m[2][3];
    m[3][2] = c.w - m[3][3];
    Mat4::from_cols_array_2d(&m)
}
```

### Recursion: depth 0=flat color, 1=single RTT, 2=optional recursive

---

## D. Teleportation Physics

### Plane crossing:
- `d = dot((p - portal.center), portal.normal)`
- Crossing: `d_prev > eps && d_curr <= -eps`

### Transform math:
```rust
let a_basis = Mat3::from_cols(a.right, a.up, a.normal);
let b_basis = Mat3::from_cols(b.right, b.up, b.normal);
let rot = b_basis * Mat3::from_rotation_y(PI) * a_basis.transpose();

let rel = player_pos - a.center;
let new_pos = b.center + rot * rel + b.normal * 0.35;
let new_vel = rot * velocity;
```

### Orientation: Rotate camera forward, rebuild yaw/pitch
### Debounce: Time gate (~0.15s) + min signed-distance from exit (~0.25)
### Post-teleport: Run collision resolve, fallback revert if stuck

---

## E. Edge Cases

1. **Partial intersection**: Swept crossing + local bounds expansion by player half-width
2. **Chunk boundaries**: Cross-chunk checks via `world_to_chunk` + `block_at`, reject if unloaded
3. **Destroyed support**: Invalidate portal on block change
4. **Overlap**: Same color replaced, opposite-color overlap rejected
5. **A→B→A recursion**: Hard cap, flat fallback at depth limit
6. **World edge**: Disable teleport if destination chunk unloaded
7. **Invalid targets**: Sky/water/non-solid = no portal
8. **Multiplayer**: Local-only initially, future C2S/S2C portal packets

---

## F. File Structure

### New files:
1. `crates/veldspar_client/src/portal.rs`
2. `crates/veldspar_client/src/renderer/portal_renderer.rs`
3. `assets/shaders/portal_surface.wgsl`
4. `assets/shaders/portal_frame.wgsl`

### Modified files:
1. `crates/veldspar_client/src/main.rs` — module wiring
2. `crates/veldspar_client/src/app.rs` — input, placement, teleport
3. `crates/veldspar_client/src/renderer/mod.rs` — portal renderer integration
4. `crates/veldspar_client/src/renderer/chunk_renderer.rs` — runtime frustum culling
5. `crates/veldspar_shared/src/inventory.rs` — ItemId::PORTAL_GUN

---

## G. Implementation Phases

### Phase 1: Data + Placement (no rendering)
- PortalManager, portal item, input interception
- Placement validation, lifecycle invalidation
- Debug wireframes via existing overlay

### Phase 2: Portal Rendering (RTT + compositing)
- Portal renderer, RTT targets, shaders
- Insert portal pass between opaque and water
- Oblique clipping, recursion depth 0/1

### Phase 3: Teleportation
- Swept plane crossing, transform, cooldown
- Orientation/velocity preservation
- Collision-safe teleport

### Phase 4: Polish
- Frame glow, particles/sound placeholders
- Floor/ceiling placement refinements
- Debug/perf counters, quality fallbacks

---

## H. Performance Budget

| Metric | Value |
|--------|-------|
| Extra draw calls | ~2x world geometry (2 visible portals, recursion 1) |
| Texture memory (full-res) | ~31.6 MiB (two 1920x1080 color+depth) |
| Texture memory (0.5x scale) | ~8 MiB |
| GPU time (0.5x, recursion 1) | +2 to +5.5 ms/frame |
| GPU time (full, recursion 1) | +5 to +12 ms/frame |

### Mitigations:
- Always frustum-cull portal passes
- Skip offscreen render for non-visible portals
- Default recursion = 1
- Dynamic quality fallback (reduce RTT scale → drop recursion to 0)
