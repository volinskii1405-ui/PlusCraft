#version 450
#extension GL_GOOGLE_include_directive : require
#define FRAGMENT_SHADER
#include "common.glsl"

layout(constant_id = 0) const float ALPHA_CUT = 0.5;
layout(set = 0, binding = 1) uniform sampler2DArray u_blocks;

layout(location = 0) in vec3 v_uv;
layout(location = 1) in vec4 v_color;
layout(location = 2) in vec2 v_light;
layout(location = 3) in vec3 v_view;
layout(location = 4) flat in uint v_textured;

layout(location = 0) out vec4 out_color;

void main() {
    vec4 c = v_color;
    if (v_textured != 0u) c *= sample_pixel_art(u_blocks, v_uv);
    if (c.a < ALPHA_CUT) discard;
    vec3 col = c.rgb;
    // Свет > 1.0 (значение 16+) означает «без освещения» (солнце, рамка).
    if (v_light.x <= 1.0) col = apply_lighting(col, v_light.x, v_light.y, 1.0);
    col = apply_fog(col, v_view);
    out_color = vec4(col, c.a);
}
