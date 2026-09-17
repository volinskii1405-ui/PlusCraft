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
    return type == BlockType::Air || type == BlockType::Leaves;
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
        default: return "Unknown";
    }
}
