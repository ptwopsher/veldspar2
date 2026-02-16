struct SkyUniforms {
    inv_view_proj: mat4x4<f32>,
    horizon_color: vec4<f32>,
    zenith_color: vec4<f32>,
    time_of_day: f32,
    _pad: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> sky: SkyUniforms;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc_position: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var out: VertexOut;

    // Generate fullscreen triangle from vertex index
    // Triangle covers entire NDC space (-1 to 1 in x,y)
    let x = f32((vertex_index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vertex_index & 2u) * 2.0 - 1.0;

    out.clip_position = vec4<f32>(x, y, 1.0, 1.0);
    out.ndc_position = vec2<f32>(x, y);

    return out;
}

fn hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn star_intensity(view_direction: vec3<f32>, night_factor: f32) -> f32 {
    let above_horizon = step(0.0, view_direction.y);
    let horizon_fade = smoothstep(0.0, 0.12, view_direction.y);

    // Project hemisphere direction to a 2D domain for procedural star placement.
    let projected = view_direction.xz / max(view_direction.y + 1.0, 0.05);
    let star_space = projected * 280.0;

    let cell = floor(star_space);
    let local = fract(star_space);

    let star_pick = hash12(cell + vec2<f32>(13.1, 71.7));
    let has_star = step(0.995, star_pick);

    let center = vec2<f32>(
        hash12(cell + vec2<f32>(31.4, 17.9)),
        hash12(cell + vec2<f32>(97.3, 53.8)),
    );
    let dist = length(local - center);

    let radius = mix(0.025, 0.09, hash12(cell + vec2<f32>(7.7, 19.2)));
    let core = 1.0 - smoothstep(radius * 0.5, radius, dist);
    let brightness = mix(0.45, 1.0, hash12(cell + vec2<f32>(111.2, 201.5)));

    return has_star * core * brightness * above_horizon * horizon_fade * night_factor;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    // Reconstruct view direction from NDC position
    // Create a point in clip space (z=1 for far plane, w=1)
    let clip_pos = vec4<f32>(input.ndc_position, 1.0, 1.0);

    // Transform to world space using inverse view-projection matrix
    let world_pos = sky.inv_view_proj * clip_pos;

    // Perspective divide and create direction vector
    let world_dir = world_pos.xyz / world_pos.w;
    let view_direction = normalize(world_dir);

    // Use the Y component (vertical direction) for gradient interpolation
    // Clamp to [0,1] range and apply power for smoother gradient
    let vertical_factor = clamp(abs(view_direction.y), 0.0, 1.0);
    let smooth_factor = pow(vertical_factor, 0.4);

    // Interpolate between horizon and zenith colors
    let sky_color = mix(sky.horizon_color, sky.zenith_color, smooth_factor);
    let sun_y = sin(sky.time_of_day * 6.28318530718 - 1.57079632679);
    let night_factor = smoothstep(0.3, 0.0, sun_y);
    let stars = star_intensity(view_direction, night_factor);
    let final_rgb = sky_color.rgb + vec3<f32>(stars);

    return vec4<f32>(final_rgb, sky_color.a);
}
