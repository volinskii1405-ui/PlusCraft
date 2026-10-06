#pragma once

#include <cstdint>

enum class BlockType : uint8_t {
    Air = 0,
    Grass,
    Dirt,
    Stone,
    Sand,
    Wood,
    Leaves,
    Planks,
    Wool,
    Glass,
    Count
};

enum class Face {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
};

inline bool isAir(BlockType type) {
    return type == BlockType::Air;
}

// Blocks a neighboring face is hidden behind unless the neighbor itself
// lets light/visibility through.
inline bool isTransparent(BlockType type) {
    return type == BlockType::Air || type == BlockType::Leaves || type == BlockType::Glass;
}

// True for blocks that need real alpha blending (as opposed to fully
// opaque blocks, or leaves' binary alpha-cutout where every surviving
// texel is fully opaque). Chunk meshes these into a separate buffer
// rendered after - and without writing to - the depth buffer, so glass
// can never incorrectly occlude something drawn behind it.
inline bool isTranslucent(BlockType type) {
    return type == BlockType::Glass;
}

inline const char* blockName(BlockType type) {
    switch (type) {
        case BlockType::Air: return "Air";
        case BlockType::Grass: return "Grass";
        case BlockType::Dirt: return "Dirt";
        case BlockType::Stone: return "Stone";
        case BlockType::Sand: return "Sand";
        case BlockType::Wood: return "Wood";
        case BlockType::Leaves: return "Leaves";
        case BlockType::Planks: return "Planks";
        case BlockType::Wool: return "Wool";
        case BlockType::Glass: return "Glass";
        default: return "Unknown";
    }
}
