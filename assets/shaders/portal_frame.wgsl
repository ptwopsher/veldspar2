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

struct FrameParams {
    color: vec4<f32>,
    glow_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var<uniform> frame_params: FrameParams;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) model_col0: vec4<f32>,
    @location(3) model_col1: vec4<f32>,
    @location(4) model_col2: vec4<f32>,
    @location(5) model_col3: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    let model = mat4x4<f32>(
        input.model_col0,
        input.model_col1,
        input.model_col2,
        input.model_col3,
    );

    let world_position = model * vec4<f32>(input.position, 1.0);
    out.clip_position = camera.view_proj * world_position;
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    let pulse_phase = camera.time_of_day * 6.2831853 * frame_params.glow_params.z + frame_params.glow_params.w;
    let pulse = frame_params.glow_params.x + frame_params.glow_params.y * sin(pulse_phase);
    let emissive = frame_params.color.rgb * pulse;
    return vec4<f32>(emissive, 1.0);
}
