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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct InstanceInput {
    @location(2) model_matrix_0: vec4<f32>,
    @location(3) model_matrix_1: vec4<f32>,
    @location(4) model_matrix_2: vec4<f32>,
    @location(5) model_matrix_3: vec4<f32>,
    @location(6) animation_phase: f32,
    @location(7) head_pitch: f32,
    @location(8) attack_animation: f32,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

fn rotate_x_about_pivot(position: vec3<f32>, pivot: vec3<f32>, angle: f32) -> vec3<f32> {
    let translated = position - pivot;
    let s = sin(angle);
    let c = cos(angle);
    let rotated = vec3<f32>(
        translated.x,
        translated.y * c - translated.z * s,
        translated.y * s + translated.z * c,
    );
    return rotated + pivot;
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOut {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    var local_position = vertex.position;
    let swing_angle = sin(instance.animation_phase * 6.28) * 0.6;
    let attack_swing = sin(instance.attack_animation * 3.14159265) * 1.1;

    let is_arm = local_position.y >= 0.75 && local_position.y <= 1.5 && abs(local_position.x) > 0.25;
    let is_head = local_position.y >= 1.5 && abs(local_position.x) <= 0.25;
    let pants_color = vec3<f32>(0.3, 0.3, 0.5);
    let is_pants = all(abs(vertex.color - pants_color) < vec3<f32>(0.001, 0.001, 0.001));
    let is_leg = is_pants && local_position.y >= 0.0 && local_position.y <= 0.75;

    if (is_head) {
        local_position = rotate_x_about_pivot(
            local_position,
            vec3<f32>(0.0, 1.75, 0.0),
            -instance.head_pitch,
        );
    }

    if (is_arm) {
        let arm_sign = select(-1.0, 1.0, local_position.x > 0.0);
        let arm_x = select(-0.375, 0.375, local_position.x > 0.0);
        local_position = rotate_x_about_pivot(
            local_position,
            vec3<f32>(arm_x, 1.5, 0.0),
            swing_angle * arm_sign,
        );
        if (local_position.x > 0.0) {
            local_position = rotate_x_about_pivot(
                local_position,
                vec3<f32>(0.375, 1.5, 0.0),
                -attack_swing,
            );
        }
    }

    if (is_leg) {
        let leg_sign = select(1.0, -1.0, local_position.x > 0.0);
        let leg_x = select(-0.125, 0.125, local_position.x > 0.0);
        local_position = rotate_x_about_pivot(
            local_position,
            vec3<f32>(leg_x, 0.75, 0.0),
            swing_angle * leg_sign,
        );
    }

    let world_position = model_matrix * vec4<f32>(local_position, 1.0);

    var out: VertexOut;
    out.clip_position = camera.view_proj * world_position;
    out.color = vertex.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let ambient = mix(
        0.15,
        1.0,
        clamp(sin(camera.time_of_day * 6.28318 - 1.5708) * 2.0 + 0.5, 0.0, 1.0),
    );
    return vec4<f32>(in.color * ambient, 1.0);
}
