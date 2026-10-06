#version 450

layout(push_constant) uniform Push { vec4 screen; vec4 extra; } pc;

layout(location = 0) in vec2 a_pos;  // пиксели
layout(location = 1) in vec2 a_uv;
layout(location = 2) in vec4 a_color;
layout(location = 3) in uint a_tex;

layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;
layout(location = 2) flat out uint v_tex;

void main() {
    vec2 ndc = a_pos / pc.screen.xy * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
    v_tex = a_tex;
}
