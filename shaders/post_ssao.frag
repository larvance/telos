#version 460

layout(location = 0) in vec2 v_uv;
layout(location = 0) out float out_occlusion;

layout(set = 0, binding = 0) uniform sampler2D u_depth;

layout(push_constant) uniform SsaorPushConstants {
    mat4 u_inv_view_proj;
    mat4 u_view_proj;
    vec2 u_screen_size;
    float u_radius;
    float u_bias;
};

// 16 Poisson/spiral hemisphere sample points (z > 0 in hemisphere)
const vec3 SAMPLES[16] = vec3[16](
    vec3( 0.23,  0.18, 0.42), vec3(-0.31,  0.22, 0.51),
    vec3( 0.15, -0.38, 0.63), vec3(-0.25, -0.19, 0.35),
    vec3( 0.45,  0.42, 0.71), vec3(-0.52,  0.35, 0.62),
    vec3( 0.38, -0.49, 0.58), vec3(-0.41, -0.45, 0.73),
    vec3( 0.12,  0.15, 0.88), vec3(-0.18,  0.11, 0.92),
    vec3( 0.09, -0.14, 0.85), vec3(-0.11, -0.16, 0.95),
    vec3( 0.58,  0.12, 0.31), vec3(-0.62,  0.08, 0.28),
    vec3( 0.11,  0.64, 0.29), vec3(-0.09, -0.59, 0.33)
);

vec3 get_world_pos(vec2 uv, float depth) {
    vec4 clip = vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    vec4 world_pos = u_inv_view_proj * clip;
    return world_pos.xyz / max(world_pos.w, 0.00001);
}

void main() {
    float depth = texture(u_depth, v_uv).r;

    // Reversed-Z: depth <= 0.00001 is sky/infinite background
    if (depth <= 0.00001) {
        out_occlusion = 1.0;
        return;
    }

    vec3 origin = get_world_pos(v_uv, depth);

    // Bilateral edge-aware reconstruction of world surface normal to avoid depth discontinuities
    vec2 texel = 1.0 / u_screen_size;
    float d_l = texture(u_depth, v_uv - vec2(texel.x, 0.0)).r;
    float d_r = texture(u_depth, v_uv + vec2(texel.x, 0.0)).r;
    float d_t = texture(u_depth, v_uv - vec2(0.0, texel.y)).r;
    float d_b = texture(u_depth, v_uv + vec2(0.0, texel.y)).r;

    vec3 p_l = get_world_pos(v_uv - vec2(texel.x, 0.0), d_l);
    vec3 p_r = get_world_pos(v_uv + vec2(texel.x, 0.0), d_r);
    vec3 p_t = get_world_pos(v_uv - vec2(0.0, texel.y), d_t);
    vec3 p_b = get_world_pos(v_uv + vec2(0.0, texel.y), d_b);

    vec3 dx = (abs(d_l - depth) < abs(d_r - depth)) ? (origin - p_l) : (p_r - origin);
    vec3 dy = (abs(d_t - depth) < abs(d_b - depth)) ? (origin - p_t) : (p_b - origin);

    vec3 normal = cross(dy, dx);
    float normal_len = length(normal);
    if (normal_len < 0.00001) {
        out_occlusion = 1.0;
        return;
    }
    normal /= normal_len;

    // In reversed-Z, depth = 1.0 is on the near plane towards the camera eye.
    // Ensure the normal always faces towards the camera (into empty space, not into solid block).
    vec3 to_camera = get_world_pos(v_uv, 1.0) - origin;
    if (dot(normal, to_camera) < 0.0) {
        normal = -normal;
    }

    // Cardinal snap for voxel faces
    vec3 abs_n = abs(normal);
    vec3 cardinal_n;
    if (abs_n.y >= abs_n.x && abs_n.y >= abs_n.z) {
        cardinal_n = vec3(0.0, normal.y >= 0.0 ? 1.0 : -1.0, 0.0);
    } else if (abs_n.x >= abs_n.z) {
        cardinal_n = vec3(normal.x >= 0.0 ? 1.0 : -1.0, 0.0, 0.0);
    } else {
        cardinal_n = vec3(0.0, 0.0, normal.z >= 0.0 ? 1.0 : -1.0);
    }
    if (dot(cardinal_n, to_camera) < 0.0) {
        cardinal_n = -cardinal_n;
    }

    // World-space tangent and bitangent orthogonal to cardinal face
    vec3 helper = (abs(cardinal_n.y) > 0.9) ? vec3(0.0, 0.0, 1.0) : vec3(0.0, 1.0, 0.0);
    vec3 tangent = normalize(cross(cardinal_n, helper));
    vec3 bitangent = cross(cardinal_n, tangent);

    // Screen-space 4x4 Bayer dither pattern for low-frequency, smoothly blurrable rotations
    int bx = int(gl_FragCoord.x) & 3;
    int by = int(gl_FragCoord.y) & 3;
    const float bayer4x4[16] = float[16](
         0.0/16.0,  8.0/16.0,  2.0/16.0, 10.0/16.0,
        12.0/16.0,  4.0/16.0, 14.0/16.0,  6.0/16.0,
         3.0/16.0, 11.0/16.0,  1.0/16.0,  9.0/16.0,
        15.0/16.0,  7.0/16.0, 13.0/16.0,  5.0/16.0
    );
    float angle = bayer4x4[by * 4 + bx] * 6.2831853;
    float cos_a = cos(angle);
    float sin_a = sin(angle);
    vec3 rot_tangent = tangent * cos_a + bitangent * sin_a;
    vec3 rot_bitangent = -tangent * sin_a + bitangent * cos_a;

    float occlusion = 0.0;
    float valid_samples = 0.0;

    for (int i = 0; i < 16; i++) {
        vec3 s = SAMPLES[i];
        vec3 sample_dir = rot_tangent * s.x + rot_bitangent * s.y + cardinal_n * s.z;
        vec3 sample_pos = origin + sample_dir * u_radius;

        // Project world-space sample point back to screen UV
        vec4 sample_clip = u_view_proj * vec4(sample_pos, 1.0);
        vec2 sample_ndc = sample_clip.xy / max(sample_clip.w, 0.00001);
        vec2 sample_uv = vec2(sample_ndc.x * 0.5 + 0.5, 0.5 - sample_ndc.y * 0.5);

        // Boundary check
        if (sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0) {
            continue;
        }

        float sample_depth = texture(u_depth, sample_uv).r;
        vec3 actual_pos = get_world_pos(sample_uv, sample_depth);

        // True 3D Euclidean distance range check (rotation-invariant)
        float dist = length(actual_pos - origin);
        float range_check = smoothstep(0.0, 1.0, u_radius / (dist + 0.001));

        // Occlusion check along the face normal: occluding geometry must be above surface but near sample
        float actual_height = dot(actual_pos - origin, cardinal_n);
        float sample_height = dot(sample_pos - origin, cardinal_n);

        if (actual_height >= u_bias && actual_height <= sample_height + 0.08) {
            occlusion += range_check;
        }
        valid_samples += 1.0;
    }

    if (valid_samples > 0.0) {
        out_occlusion = clamp(1.0 - (occlusion / valid_samples) * 1.1, 0.0, 1.0);
    } else {
        out_occlusion = 1.0;
    }
}
