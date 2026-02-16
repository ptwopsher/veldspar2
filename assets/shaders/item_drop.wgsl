struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    fog_color: vec4<f32>,
    fog_start: f32,
    fog_end: f32,
    time_of_day: f32,
    underwater: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(1) @binding(1)
var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) shade: f32,
    @location(2) uv: vec2<f32>,
}

struct InstanceInput {
    @location(3) model_matrix_0: vec4<f32>,
    @location(4) model_matrix_1: vec4<f32>,
    @location(5) model_matrix_2: vec4<f32>,
    @location(6) model_matrix_3: vec4<f32>,
    @location(7) color: vec4<f32>,
    @location(8) tile_origin: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) tile_origin: vec2<f32>,
    @location(3) use_texture: f32,
}

const TILE_SIZE_UV: f32 = 0.03125;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOut {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let world_position = model_matrix * vec4<f32>(vertex.position, 1.0);

    var out: VertexOut;
    out.clip_position = camera.view_proj * world_position;
    out.color = instance.color.rgb * vertex.shade;
    out.tex_coord = vertex.uv;
    out.tile_origin = instance.tile_origin;
    // If tile_origin is (0,0) and color.a < 1, no texture; otherwise use texture
    // We use color.a as use_texture flag: 1.0 = has texture, 0.0 = flat color
    out.use_texture = instance.color.a;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let ambient = mix(
        0.15,
        1.0,
        clamp(sin(camera.time_of_day * 6.28318 - 1.5708) * 2.0 + 0.5, 0.0, 1.0),
    );

    if in.use_texture > 0.5 {
        let sample_uv = in.tile_origin + in.tex_coord * TILE_SIZE_UV;
        let sampled = textureSample(atlas_texture, atlas_sampler, sample_uv);
        if sampled.a < 0.02 {
            discard;
        }
        let tinted = sampled.rgb * in.color;
        return vec4<f32>(tinted * ambient, 1.0);
    } else {
        return vec4<f32>(in.color * ambient, 1.0);
    }
}
