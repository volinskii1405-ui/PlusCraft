#version 450
#extension GL_GOOGLE_include_directive : require
#define FRAGMENT_SHADER
#include "common.glsl"

layout(location = 0) in vec2 v_ndc;
layout(location = 0) out vec4 out_color;

float hash(vec3 p) {
    p = fract(p * 0.3183099 + 0.1);
    p *= 17.0;
    return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

void main() {
    // Направление луча из обратной матрицы (камера в начале координат).
    vec4 far = g.inv_view_proj * vec4(v_ndc, 0.0001, 1.0);
    vec3 dir = normalize(far.xyz / far.w);

    float h = dir.y;
    vec3 col = mix(g.sky_horizon.rgb, g.sky_top.rgb, smoothstep(-0.05, 0.6, h));
    // Ниже горизонта — темнее (бездна под миром).
    col = mix(col, g.sky_horizon.rgb * 0.35, smoothstep(0.0, -0.4, h));

    vec3 sun = normalize(g.sun_dir.xyz);
    float day = g.params.y;

    // Ореол заката/рассвета у горизонта со стороны солнца.
    float sunset = pow(max(dot(dir, normalize(vec3(sun.x, 0.0, sun.z) + 1e-4)), 0.0), 6.0)
        * (1.0 - abs(sun.y)) * smoothstep(-0.25, 0.1, sun.y) * (1.0 - smoothstep(0.0, 0.5, abs(h)));
    col += vec3(1.0, 0.45, 0.15) * sunset * 0.7;

    // Звёзды ночью.
    if (h > 0.0) {
        vec3 sd = dir * 180.0;
        float s = hash(floor(sd));
        float star = step(0.9965, s) * (1.0 - day) * smoothstep(0.0, 0.25, h);
        col += vec3(star * 0.9);
    }

    // Солнце — квадратный диск (в духе стиля), луна — с противоположной стороны.
    vec3 up = abs(sun.y) > 0.99 ? vec3(1, 0, 0) : vec3(0, 1, 0);
    vec3 right = normalize(cross(up, sun));
    vec3 up2 = cross(sun, right);
    float ds = dot(dir, sun);
    if (ds > 0.0) {
        vec2 q = vec2(dot(dir, right), dot(dir, up2)) / ds;
        float m = max(abs(q.x), abs(q.y));
        col = mix(col, vec3(1.0, 0.95, 0.75) * 1.6, 1.0 - smoothstep(0.075, 0.082, m));
        col += vec3(1.0, 0.8, 0.5) * 0.25 * exp(-m * 9.0) * smoothstep(-0.2, 0.1, sun.y);
    }
    float dm = dot(dir, -sun);
    if (dm > 0.0) {
        vec2 q = vec2(dot(dir, -right), dot(dir, up2)) / dm;
        float m = max(abs(q.x), abs(q.y));
        float moon = 1.0 - smoothstep(0.05, 0.055, m);
        // Кратеры на луне.
        float crater = step(0.75, hash(floor(vec3(q * 60.0, 1.0)))) * 0.25;
        col = mix(col, vec3(0.85, 0.88, 0.95) * (1.0 - crater), moon);
    }

    // Под водой — сплошной цвет тумана.
    if (g.params.z > 0.5) col = g.fog_color.rgb;
    out_color = vec4(col, 1.0);
}
