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

struct ParticleParams {
    size_and_pad: vec4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> particle_params: ParticleParams;

struct VertexIn {
    @location(0) quad_pos: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) age: f32,
    @location(4) lifetime: f32,
    @location(5) size: vec2<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) age_ratio: f32,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;

    let camera_right = particle_params.camera_right.xyz;
    let camera_up = particle_params.camera_up.xyz;
    let world_size = input.size * particle_params.size_and_pad.x;
    let billboard_offset =
        camera_right * (input.quad_pos.x * world_size.x)
        + camera_up * (input.quad_pos.y * world_size.y);

    let world_pos = input.world_pos + billboard_offset;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = input.color;
    out.age_ratio = clamp(input.age / max(input.lifetime, 0.0001), 0.0, 1.0);

    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let alpha = (1.0 - input.age_ratio) * input.color.a;
    return vec4<f32>(input.color.rgb, alpha);
}
