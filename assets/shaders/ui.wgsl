@group(0) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(0) @binding(1)
var atlas_sampler: sampler;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) tile_origin: vec2<f32>,
    @location(3) use_texture: f32,
}

const TILE_SIZE_UV: f32 = 0.03125; // 16.0 / 512.0

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) tile_origin: vec2<f32>,
    @location(4) use_texture: f32,
) -> VertexOut {
    var out: VertexOut;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.tex_coord = tex_coord;
    out.tile_origin = tile_origin;
    out.use_texture = use_texture;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if in.use_texture > 0.5 {
        let atlas_uv = in.tile_origin + in.tex_coord * TILE_SIZE_UV;
        let sampled = textureSample(atlas_texture, atlas_sampler, atlas_uv);
        if sampled.a < 0.5 {
            discard;
        }
        return vec4<f32>(sampled.rgb * in.color.rgb, in.color.a);
    }
    return in.color;
}
