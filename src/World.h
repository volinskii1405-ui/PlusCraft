#pragma once

#include "Block.h"
#include "Chunk.h"
#include "Shader.h"
#include "Texture.h"

#include <glm/glm.hpp>

// The MVP world is a single generated Chunk plus the texture atlas it's
// rendered with. A multi-chunk world would wrap several Chunks behind
// the same getBlock/setBlock/raycast interface.
class World {
public:
    struct RaycastHit {
        bool hit = false;
        glm::ivec3 blockPos{0};  // the solid block that was hit
        glm::ivec3 placePos{0};  // the empty block just before it, for placing
    };

    explicit World(uint32_t seed);

    BlockType getBlock(int x, int y, int z) const;
    void setBlock(int x, int y, int z, BlockType type);

    RaycastHit raycast(glm::vec3 origin, glm::vec3 direction, float maxDistance) const;

    void render(const Shader& shader) const;

    glm::vec3 spawnPoint() const;
    const TextureAtlas& atlas() const { return atlas_; }

private:
    Chunk chunk_;
    TextureAtlas atlas_;
};
