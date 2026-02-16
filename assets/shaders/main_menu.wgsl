struct MainMenuUniform {
    resolution: vec2<f32>,
    time: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> menu: MainMenuUniform;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) ndc_position: vec2<f32>,
    @location(2) effect: f32,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) effect: f32,
) -> VertexOut {
    var out: VertexOut;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.ndc_position = position;
    out.effect = effect;
    return out;
}

fn hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let s = local * local * (3.0 - 2.0 * local);

    let a = hash12(cell);
    let b = hash12(cell + vec2<f32>(1.0, 0.0));
    let c = hash12(cell + vec2<f32>(0.0, 1.0));
    let d = hash12(cell + vec2<f32>(1.0, 1.0));

    let ab = mix(a, b, s.x);
    let cd = mix(c, d, s.x);
    return mix(ab, cd, s.y);
}

fn menu_background(ndc_position: vec2<f32>) -> vec3<f32> {
    let uv = ndc_position * 0.5 + vec2<f32>(0.5, 0.5);
    let top = vec3<f32>(0.06, 0.2, 0.22);
    let bottom = vec3<f32>(0.01, 0.06, 0.08);
    let gradient_t = smoothstep(0.0, 1.0, uv.y);
    var color = mix(bottom, top, gradient_t);

    let aspect = menu.resolution.x / max(menu.resolution.y, 1.0);
    let drift = vec2<f32>(menu.time * 0.004, menu.time * 0.003);
    let noise_uv = vec2<f32>(uv.x * aspect, uv.y) * 6.0 + drift;
    let n0 = value_noise(noise_uv);
    let n1 = value_noise(noise_uv * 2.11 + vec2<f32>(17.3, 9.7)) * 0.5;
    let pattern = (n0 + n1) / 1.5;
    color += vec3<f32>(0.04, 0.08, 0.08) * (pattern - 0.5) * 0.75;

    let horizon_band = exp(-pow((uv.y - 0.35) * 3.5, 2.0)) * 0.14;
    color += vec3<f32>(0.02, 0.08, 0.07) * horizon_band;

    let scan = sin((uv.y * 28.0 + menu.time * 0.35) * 6.283185);
    color += vec3<f32>(0.0, 0.03, 0.025) * (scan * 0.5 + 0.5) * 0.15;

    let centered = uv * 2.0 - vec2<f32>(1.0, 1.0);
    let vignette = 1.0 - clamp(dot(centered, centered) * 0.2, 0.0, 0.3);
    return color * vignette;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if in.effect > 0.5 {
        let bg = menu_background(in.ndc_position);
        return vec4<f32>(bg, in.color.a);
    }
    return in.color;
}
