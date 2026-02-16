struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    fog_color: vec4<f32>,
    fog_start: f32,
    fog_end: f32,
    time_of_day: f32,
    underwater: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(1) @binding(1)
var atlas_sampler: sampler;

struct ChunkParams {
    fade: f32,
};
@group(2) @binding(0)
var<uniform> chunk_params: ChunkParams;

const TILE_SIZE_UV: f32 = 0.03125; // 16.0 / 512.0

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
    var world_pos = input.position;
    if (abs(input.normal.y) > 0.5) {
        let wave = sin(world_pos.x * 1.5 + camera.time_of_day * 50.0) * 0.04
            + cos(world_pos.z * 2.0 + camera.time_of_day * 40.0) * 0.03;
        world_pos.y = world_pos.y + wave;
    }

    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.normal = input.normal;
    out.tex_coord = input.tex_coord;
    out.ao = input.ao;
    out.light = input.light;
    out.emissive_light = input.emissive_light;
    out.tile_origin = input.tile_origin;
    out.world_position = world_pos;
    out.tint_color = input.tint_color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    // Tile the texture: fract() makes the UV repeat per block
    let flow_time = camera.time_of_day * 24000.0;
    var animated_uv = input.tex_coord + vec2<f32>(flow_time * 0.015, -flow_time * 0.012);
    let micro_wave = vec2<f32>(
        sin((input.world_position.x + input.world_position.z) * 1.6 + flow_time * 0.11),
        cos((input.world_position.z - input.world_position.x) * 1.3 + flow_time * 0.09),
    ) * 0.05;
    animated_uv += micro_wave;
    let local_uv = fract(animated_uv);
    let atlas_uv = input.tile_origin + local_uv * TILE_SIZE_UV;

    let sampled = textureSample(atlas_texture, atlas_sampler, atlas_uv);

    let normal = normalize(input.normal);
    let up = max(normal.y, 0.0);
    let down = max(-normal.y, 0.0);
    let side = 1.0 - up - down;

    let sun_angle = camera.time_of_day * 6.283185 - 1.5707963;
    let sun_y = sin(sun_angle);
    let sun_xz = cos(sun_angle);
    let light_dir = normalize(vec3<f32>(0.25 * sun_xz, max(sun_y, 0.0), 0.5 * sun_xz));
    let day_factor = smoothstep(-0.1, 0.15, sun_y);
    let diffuse = max(dot(normal, light_dir), 0.0);
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

    // Subtle wave-based tint variation in the blue channel
    let wave_pattern = sin(input.world_position.x * 1.5 + camera.time_of_day * 50.0) * 0.5
        + cos(input.world_position.z * 2.0 + camera.time_of_day * 40.0) * 0.5;
    let blue_shift = 0.95 + wave_pattern * 0.05;
    let water_tint = vec3<f32>(0.4, 0.7 + wave_pattern * 0.015, blue_shift);

    // Subtle sun specular highlight
    let view_dir = normalize(camera.camera_pos.xyz - input.world_position);
    let reflected = reflect(-light_dir, normal);
    let specular_strength = pow(max(dot(view_dir, reflected), 0.0), 48.0) * 0.2 * day_factor * up;
    let specular = vec3<f32>(1.0, 1.0, 0.95) * specular_strength;

    let lit_color = sampled.rgb * water_tint * lighting * ao + specular;

    // Distance fog
    let dist = length(input.world_position - camera.camera_pos.xyz);
    let underwater_factor = select(0.0, 1.0, camera.underwater > 0.5);
    let fog_color = mix(camera.fog_color.rgb, vec3<f32>(0.05, 0.15, 0.35), underwater_factor);
    let fog_start = mix(camera.fog_start, 0.0, underwater_factor);
    let fog_end = mix(camera.fog_end, 48.0, underwater_factor);
    let fog_range = max(fog_end - fog_start, 0.001);
    let fog_factor = clamp((fog_end - dist) / fog_range, 0.0, 1.0);

    // Chunk fade-in
    let visibility = fog_factor * chunk_params.fade;
    let sky_light_factor = light * light_scale;
    let local_light_factor = block_light * (0.72 + 0.28 * night_factor);
    let light_factor = max(sky_light_factor, local_light_factor);
    let lit_with_light = lit_color * light_factor;
    let final_color = mix(fog_color, lit_with_light, visibility);

    // Fresnel-like transparency: clearer when looking straight down
    let ndotv = clamp(dot(normal, view_dir), 0.0, 1.0);
    let fresnel = pow(1.0 - ndotv, 3.0);
    let top_alpha = mix(0.5, 0.68, fresnel);
    let alpha = mix(0.6, top_alpha, up);

    return vec4<f32>(final_color, alpha);
}
