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

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) alpha: f32,
}

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) alpha: f32,
) -> VertexOut {
    var out: VertexOut;
    out.clip_position = camera.view_proj * vec4<f32>(position, 1.0);
    out.alpha = alpha;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, input.alpha);
}
