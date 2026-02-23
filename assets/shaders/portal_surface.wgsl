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

struct PortalParams {
    model: mat4x4<f32>,
    color: vec4<f32>,
    linked: f32,
    recursion: f32,
    _padding0: f32,
    _padding1: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var portal_texture: texture_2d<f32>;
@group(1) @binding(1)
var portal_sampler: sampler;

@group(2) @binding(0)
var<uniform> portal_params: PortalParams;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    let world_position = portal_params.model * vec4<f32>(input.position, 1.0);
    out.clip_position = camera.view_proj * world_position;
    out.uv = input.uv;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    if (portal_params.linked < 0.5 || portal_params.recursion < 0.5) {
        return vec4<f32>(portal_params.color.rgb, 0.92);
    }

    let sampled = textureSample(portal_texture, portal_sampler, input.uv);
    let tint = mix(vec3<f32>(1.0, 1.0, 1.0), portal_params.color.rgb, 0.08);
    return vec4<f32>(sampled.rgb * tint, 1.0);
}
