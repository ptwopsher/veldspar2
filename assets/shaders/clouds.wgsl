struct CloudUniform {
    camera_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    time: f32,
    cloud_height: f32,
    _pad: vec2<f32>,
    _pad1: vec4<f32>,
    _pad2: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> cloud: CloudUniform;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc_position: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var out: VertexOut;

    let x = f32((vertex_index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vertex_index & 2u) * 2.0 - 1.0;

    out.clip_position = vec4<f32>(x, y, 1.0, 1.0);
    out.ndc_position = vec2<f32>(x, y);
    return out;
}

fn hash_2d(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash_2d(i);
    let b = hash_2d(i + vec2<f32>(1.0, 0.0));
    let c = hash_2d(i + vec2<f32>(0.0, 1.0));
    let d = hash_2d(i + vec2<f32>(1.0, 1.0));

    let ab = mix(a, b, u.x);
    let cd = mix(c, d, u.x);
    return mix(ab, cd, u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    let n0 = value_noise(p);
    let n1 = value_noise(p * 2.03 + vec2<f32>(37.2, 17.9)) * 0.5;
    let n2 = value_noise(p * 4.01 + vec2<f32>(11.3, 53.8)) * 0.25;
    return (n0 + n1 + n2) / 1.75;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let clip_pos = vec4<f32>(input.ndc_position, 1.0, 1.0);
    let world_far = cloud.inv_view_proj * clip_pos;
    let world_far_pos = world_far.xyz / world_far.w;

    let camera_pos = cloud.camera_pos.xyz;
    let ray_dir = normalize(world_far_pos - camera_pos);

    if ray_dir.y <= 0.0 {
        return vec4<f32>(0.0);
    }

    let t = (cloud.cloud_height - camera_pos.y) / ray_dir.y;
    if t <= 0.0 {
        return vec4<f32>(0.0);
    }

    let hit = camera_pos + ray_dir * t;
    let drift = vec2<f32>(cloud.time * 0.006, cloud.time * 0.004);
    let uv = hit.xz * 0.005 + drift;

    let base = fbm(uv);
    let detail = value_noise(uv * 3.0 + vec2<f32>(19.7, 7.3));
    let cloud_density = smoothstep(0.52, 0.78, base + detail * 0.15);

    let coverage = smoothstep(0.02, 0.22, ray_dir.y);
    let alpha = clamp(cloud_density * coverage * 0.85, 0.0, 1.0);

    return vec4<f32>(1.0, 1.0, 1.0, alpha);
}
