#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

layout(push_constant) uniform Push { vec4 origin; vec4 extra; } pc;

layout(location = 0) in uvec4 a_pos;   // xyz в 1/16 блока, w — слой текстуры
layout(location = 1) in uvec2 a_uv;    // в 1/16 тайла
layout(location = 2) in uvec4 a_info;  // нормаль|AO, небесный свет×4, блочный свет×4, флаги
layout(location = 3) in vec4 a_color;  // тинт биома

layout(location = 0) out vec3 v_uv;
layout(location = 1) out vec4 v_color;
layout(location = 2) out vec2 v_light;
layout(location = 3) out vec3 v_view;
layout(location = 4) flat out uint v_flags;

const float FACE_SHADE[6] = float[6](1.0, 0.55, 0.8, 0.8, 0.68, 0.68); // +Y -Y +X -X +Z -Z
const float AO_CURVE[4] = float[4](0.42, 0.62, 0.81, 1.0);

void main() {
    vec3 p = vec3(a_pos.xyz) / 16.0 + pc.origin.xyz;
    uint flags = a_info.w;
    float t = g.cam_pos.w;
    vec3 wp = p + g.cam_pos.xyz;
    if ((flags & 1u) != 0u) {
        // Колыхание листвы/травы.
        p.x += sin(t * 1.7 + wp.z * 0.9 + wp.y * 0.4) * 0.035;
        p.z += cos(t * 1.3 + wp.x * 0.7 + wp.y * 0.3) * 0.035;
    }
    if ((flags & 2u) != 0u) {
        // Лёгкая волна на поверхности жидкости.
        p.y += sin(t * 1.4 + wp.x * 0.9 + wp.z * 0.7) * 0.025 - 0.025;
    }
    gl_Position = g.view_proj * vec4(p, 1.0);

    uint n = a_info.x & 7u;
    uint ao = (a_info.x >> 3) & 3u;
    float shade = FACE_SHADE[min(n, 5u)] * AO_CURVE[ao];
    v_color = vec4(pow(a_color.rgb, vec3(2.2)) * shade, a_color.a);
    v_light = vec2(float(a_info.y), float(a_info.z)) / 60.0;
    v_uv = vec3(vec2(a_uv) / 16.0, float(a_pos.w));
    v_view = p;
    v_flags = flags;
}
