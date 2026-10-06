// Общие объявления для всех шейдеров PlusCraft.
layout(set = 0, binding = 0) uniform Globals {
    mat4 view_proj;
    mat4 inv_view_proj;
    vec4 cam_pos;      // xyz — мировая позиция камеры, w — время (с)
    vec4 fog_color;    // rgb, a — начало тумана
    vec4 params;       // x — конец тумана, y — дневной свет, z — под водой, w — мин. яркость
    vec4 sun_dir;      // xyz — на солнце, w — время суток 0..1
    vec4 sky_top;
    vec4 sky_horizon;
    vec4 block_light;  // rgb — цвет факелов, w — гамма/яркость
    vec4 sky_light;    // rgb — цвет небесного света, w — сила дебаффа темноты
} g;

// Кривая яркости: уровень света 0..1 -> множитель освещённости.
float light_curve(float l) {
    float b = l / (4.0 - 3.0 * l);
    // Настройка яркости смешивает «тёмную» кривую с линейной.
    return mix(b, sqrt(l), g.block_light.w * 0.6);
}

vec3 apply_lighting(vec3 albedo, float sky, float blk) {
    float s = light_curve(sky) * g.params.y;
    float b = light_curve(blk);
    vec3 light = max(vec3(s) * g.sky_light.rgb, vec3(b) * g.block_light.rgb);
    light = max(light, vec3(g.params.w));
    return albedo * light;
}

vec3 apply_fog(vec3 color, vec3 view_pos) {
    float d = length(view_pos);
    float f = clamp((d - g.fog_color.a) / max(g.params.x - g.fog_color.a, 0.001), 0.0, 1.0);
    f = f * f * (3.0 - 2.0 * f);
    return mix(color, g.fog_color.rgb, f);
}

#ifdef FRAGMENT_SHADER
// Чёткая «пиксельная» выборка: тексели остаются квадратными при увеличении,
// а их края сглаживаются на ширину одного пикселя экрана. Выбор mip-уровня —
// по исходным производным UV, поэтому вдали работает обычная фильтрация.
vec4 sample_pixel_art(sampler2DArray tex, vec3 uv) {
    vec2 size = vec2(textureSize(tex, 0).xy);
    vec2 px = uv.xy * size;
    vec2 seam = floor(px + 0.5);
    vec2 dpx = max(fwidth(px), vec2(1e-5));
    vec2 snapped = (seam + clamp((px - seam) / dpx, -0.5, 0.5)) / size;
    return textureGrad(tex, vec3(snapped, uv.z), dFdx(uv.xy), dFdy(uv.xy));
}
#endif
