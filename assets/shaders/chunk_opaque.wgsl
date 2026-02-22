struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    fog_color: vec4<f32>,
    fog_start: f32,
    fog_end: f32,
    time_of_day: f32,
    underwater: f32,
    render_time_seconds: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(1) @binding(1)
var atlas_sampler: sampler;

struct ChunkParams {
    spawn_time: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
};
@group(2) @binding(0)
var<uniform> chunk_params: ChunkParams;

const TILE_SIZE_UV: f32 = 0.03125; // 16.0 / 512.0
const CHUNK_FADE_DURATION_SECS: f32 = 0.4;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) ao: f32,
    @location(4) light: f32,
    @location(5) emissive_light: f32,
    @location(6) tile_origin: vec2<f32>,
    @location(7) tint_color: vec3<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) ao: f32,
    @location(3) light: f32,
    @location(4) emissive_light: f32,
    @location(5) tile_origin: vec2<f32>,
    @location(6) world_position: vec3<f32>,
    @location(7) tint_color: vec3<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.normal = input.normal;
    out.tex_coord = input.tex_coord;
    out.ao = input.ao;
    out.light = input.light;
    out.emissive_light = input.emissive_light;
    out.tile_origin = input.tile_origin;
    out.world_position = input.position;
    out.tint_color = input.tint_color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    // Tile the texture: fract() makes the UV repeat per block
    let is_lava = input.tint_color.r > 0.9 && input.tint_color.g < 0.5 && input.tint_color.b < 0.2;
    let flow_time = camera.time_of_day * 24000.0;
    var animated_uv = input.tex_coord;
    if (is_lava) {
        animated_uv = animated_uv + vec2<f32>(flow_time * 0.01, flow_time * 0.006);
        let lava_noise = vec2<f32>(
            sin(input.world_position.x * 1.4 + flow_time * 0.07),
            cos(input.world_position.z * 1.6 + flow_time * 0.08),
        ) * 0.06;
        animated_uv += lava_noise;
    }
    let local_uv = fract(animated_uv);
    let atlas_uv = input.tile_origin + local_uv * TILE_SIZE_UV;

    let sampled = textureSample(atlas_texture, atlas_sampler, atlas_uv);

    // Alpha cutout for cross-plant meshes (flowers, tall grass, etc.)
    if sampled.a < 0.02 {
        discard;
    }

    let up = max(input.normal.y, 0.0);
    let down = max(-input.normal.y, 0.0);
    let side = 1.0 - up - down;

    let sun_angle = camera.time_of_day * 6.283185 - 1.5707963;
    let sun_y = sin(sun_angle);
    let sun_xz = cos(sun_angle);
    let light_dir = normalize(vec3<f32>(0.25 * sun_xz, max(sun_y, 0.0), 0.5 * sun_xz));
    let day_factor = smoothstep(-0.1, 0.15, sun_y);
    let diffuse = max(dot(normalize(input.normal), light_dir), 0.0);
    let hemisphere = up * 1.0 + side * 0.85 + down * 0.7;
    let hemi_ambient = mix(0.15, 0.25, day_factor);
    let lighting = (hemi_ambient + diffuse * 0.75 * day_factor) * hemisphere;
    let ao = clamp(input.ao, 0.0, 1.0);
    let light = clamp(input.light, 0.0, 1.0);
    let block_light = clamp(input.emissive_light, 0.0, 1.0);
    let day_curve = clamp(sin(camera.time_of_day * 6.28318 - 1.5708) * 2.0 + 0.5, 0.0, 1.0);
    let ambient = mix(0.15, 1.0, day_curve);
    let night_factor = 1.0 - day_factor;
    let light_scale = mix(ambient, sqrt(ambient), night_factor);

    let lit_color = sampled.rgb * input.tint_color * lighting * ao;

    // Distance fog
    let dist = length(input.world_position - camera.camera_pos.xyz);
    let underwater_factor = select(0.0, 1.0, camera.underwater > 0.5);
    let fog_color = mix(camera.fog_color.rgb, vec3<f32>(0.05, 0.15, 0.35), underwater_factor);
    let fog_start = mix(camera.fog_start, 0.0, underwater_factor);
    let fog_end = mix(camera.fog_end, 48.0, underwater_factor);
    let fog_range = max(fog_end - fog_start, 0.001);
    var fog_factor = clamp((fog_end - dist) / fog_range, 0.0, 1.0);
    if (fog_end <= fog_start + 0.01) {
        fog_factor = 1.0;
    }

    // Keep chunks fully visible even if chunk fade timing data is out of sync.
    let visibility = fog_factor;
    let lava_pulse = 1.0 + 0.12 * sin(flow_time * 0.08 + input.world_position.x * 0.4 + input.world_position.z * 0.4);
    let base_light_factor = light * light_scale;
    let local_light_factor = block_light * (0.72 + 0.28 * night_factor);
    let lit_factor = max(base_light_factor, local_light_factor);
    let lava_light_factor = max(max(light, block_light), 0.75) * mix(ambient, 1.0, 0.7) * lava_pulse;
    let light_factor = select(lit_factor, lava_light_factor, is_lava);

    let warm_luma = dot(sampled.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let warm_texel =
        smoothstep(0.72, 0.92, warm_luma) * smoothstep(0.18, 0.4, sampled.r - sampled.b);
    let emissive_mask = max(
        select(0.0, 1.0, is_lava),
        warm_texel * smoothstep(0.75, 1.0, max(light, block_light)),
    );
    let emissive_strength = night_factor * emissive_mask * select(0.55, 0.95 * lava_pulse, is_lava)
        + block_light * 0.2;
    let emissive = sampled.rgb * input.tint_color * emissive_strength;

    let lit_with_light = lit_color * light_factor + emissive;
    let final_color = mix(fog_color, lit_with_light, visibility);

    return vec4<f32>(final_color, 1.0);
}
