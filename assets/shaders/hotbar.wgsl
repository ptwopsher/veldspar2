struct ScreenUniform {
    dimensions: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;

    // Convert pixel coordinates to NDC
    let ndc_x = position.x / screen.dimensions.x * 2.0 - 1.0;
    let ndc_y = 1.0 - position.y / screen.dimensions.y * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
