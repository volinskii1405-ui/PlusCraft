#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

layout(location = 0) in vec3 a_pos;    // относительно камеры
layout(location = 1) in vec2 a_uv;
layout(location = 2) in vec4 a_color;
layout(location = 3) in uvec2 a_extra; // слой текстуры, свет (небо<<8 | блок)

layout(location = 0) out vec3 v_uv;
layout(location = 1) out vec4 v_color;
layout(location = 2) out vec2 v_light;
layout(location = 3) out vec3 v_view;
layout(location = 4) flat out uint v_textured;

void main() {
    gl_Position = g.view_proj * vec4(a_pos, 1.0);
    v_uv = vec3(a_uv, float(a_extra.x));
    v_textured = a_extra.x == 0xFFFFu ? 0u : 1u;
    v_color = vec4(pow(a_color.rgb, vec3(2.2)), a_color.a);
    v_light = vec2(float(a_extra.y >> 8), float(a_extra.y & 255u)) / 15.0;
    v_view = a_pos;
}
