#include "World.h"

#include <glm/gtc/matrix_transform.hpp>

World::World(uint32_t seed) {
    chunk_.generate(seed);
    chunk_.rebuildMesh(atlas_);
}

BlockType World::getBlock(int x, int y, int z) const {
    return chunk_.getBlock(x, y, z);
}

void World::setBlock(int x, int y, int z, BlockType type) {
    if (!Chunk::inBounds(x, y, z)) {
        return;
    }
    chunk_.setBlock(x, y, z, type);
    chunk_.rebuildMesh(atlas_);
}

World::RaycastHit World::raycast(glm::vec3 origin, glm::vec3 direction, float maxDistance) const {
    const float step = 0.05f;
    glm::ivec3 lastEmpty = glm::ivec3(glm::floor(origin));

    for (float t = 0.0f; t < maxDistance; t += step) {
        glm::vec3 p = origin + direction * t;
        glm::ivec3 block = glm::ivec3(glm::floor(p));
        BlockType type = getBlock(block.x, block.y, block.z);
        if (!isAir(type)) {
            RaycastHit hit;
            hit.hit = true;
            hit.blockPos = block;
            hit.placePos = lastEmpty;
            return hit;
        }
        lastEmpty = block;
    }
    return {};
}

void World::render(const Shader& shader) const {
    atlas_.bind(GL_TEXTURE0);
    shader.setInt("uAtlas", 0);
    chunk_.render();
}

glm::vec3 World::spawnPoint() const {
    int cx = Chunk::SizeX / 2;
    int cz = Chunk::SizeZ / 2;
    for (int y = Chunk::SizeY - 1; y >= 0; --y) {
        if (!isAir(getBlock(cx, y, cz))) {
            return glm::vec3(cx + 0.5f, y + 2.0f, cz + 0.5f);
        }
    }
    return glm::vec3(cx + 0.5f, Chunk::SizeY / 2.0f, cz + 0.5f);
}
