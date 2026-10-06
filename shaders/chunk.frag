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
layout(location = 4) flat in uint v_flags;

layout(location = 0) out vec4 out_color;

void main() {
    vec3 uv = v_uv;
    if ((v_flags & 2u) != 0u) {
        // Течение текстуры жидкости.
        uv.xy += vec2(g.cam_pos.w * 0.03, g.cam_pos.w * 0.02);
    }
    vec4 tex = sample_pixel_art(u_blocks, uv);
    if (tex.a < ALPHA_CUT) discard;
    vec3 albedo = tex.rgb * v_color.rgb;
    vec3 col;
    if ((v_flags & 4u) != 0u) {
        col = albedo * 1.1; // эмиссивные блоки не темнеют
    } else {
        col = apply_lighting(albedo, v_light.x, v_light.y);
    }
    col = apply_fog(col, v_view);
    out_color = vec4(col, tex.a * v_color.a);
}
