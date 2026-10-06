#pragma once

#include <cmath>
#include <cstdint>

// Small deterministic hash + value-noise helpers, shared by the
// procedural texture atlas (per-pixel variation) and the terrain
// generator (heightmap). Not real Perlin noise, but smooth and cheap
// enough for a single chunk.
namespace noise {

inline uint32_t hash2(int x, int y, uint32_t seed = 0) {
    uint32_t h = static_cast<uint32_t>(x) * 374761393u + static_cast<uint32_t>(y) * 668265263u + seed * 2246822519u;
    h = (h ^ (h >> 13)) * 1274126177u;
    h ^= h >> 16;
    return h;
}

// Deterministic pseudo-random float in [0, 1) for an integer lattice point.
inline float rand01(int x, int y, uint32_t seed = 0) {
    return (hash2(x, y, seed) & 0xFFFFFFu) / static_cast<float>(0x1000000u);
}

inline float smoothstep(float t) {
    return t * t * (3.0f - 2.0f * t);
}

inline float lerp(float a, float b, float t) {
    return a + (b - a) * t;
}

// Smooth 2D value noise in [0, 1), sampled at continuous (x, y).
inline float value2D(float x, float y, uint32_t seed = 0) {
    int x0 = static_cast<int>(std::floor(x));
    int y0 = static_cast<int>(std::floor(y));
    int x1 = x0 + 1;
    int y1 = y0 + 1;

    float sx = smoothstep(x - x0);
    float sy = smoothstep(y - y0);

    float n00 = rand01(x0, y0, seed);
    float n10 = rand01(x1, y0, seed);
    float n01 = rand01(x0, y1, seed);
    float n11 = rand01(x1, y1, seed);

    float ix0 = lerp(n00, n10, sx);
    float ix1 = lerp(n01, n11, sx);
    return lerp(ix0, ix1, sy);
}

// Two-octave fractal sum, still in roughly [0, 1).
inline float fractal2D(float x, float y, uint32_t seed = 0) {
    float total = value2D(x, y, seed) * 0.65f;
    total += value2D(x * 2.3f, y * 2.3f, seed + 1) * 0.35f;
    return total;
}

} // namespace noise
