#version 450

layout(set = 0, binding = 1) uniform sampler2DArray u_blocks;
layout(set = 0, binding = 2) uniform sampler2DArray u_ui;

layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;
layout(location = 2) flat in uint v_tex;

layout(location = 0) out vec4 out_color;

void main() {
    // Цвета вершин заданы в sRGB, swapchain — sRGB: переводим в линейное пространство.
    vec4 c = vec4(pow(v_color.rgb, vec3(2.2)), v_color.a);
    if (v_tex == 0xFFFFFFFFu) {
        // сплошной цвет
    } else if ((v_tex & 0x80000000u) != 0u) {
        // атлас интерфейса: шрифт хранится в альфа-канале
        c *= texture(u_ui, vec3(v_uv, float(v_tex & 0x7FFFFFFFu)));
    } else {
        // иконка блока/предмета: без мипов, чётко
        c *= textureLod(u_blocks, vec3(v_uv, float(v_tex)), 0.0);
    }
    if (c.a < 0.003) discard;
    out_color = c;
}
