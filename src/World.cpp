#include "World.h"

#include <glm/gtc/matrix_transform.hpp>

World::World(uint32_t seed, bool generateTerrain) {
    for (int cz = 0; cz < ChunksZ; ++cz) {
        for (int cx = 0; cx < ChunksX; ++cx) {
            chunks_[static_cast<size_t>(cz) * ChunksX + cx] = std::make_unique<Chunk>();
        }
    }

    if (!generateTerrain) {
        // Caller (WorldIO) will fill every chunk's blocks via
        // loadChunkBlocks() and then call remesh() once itself.
        return;
    }

    // Generate every chunk's blocks first, then mesh them - meshing
    // reads neighbor blocks through World, so a chunk meshed before its
    // neighbor is generated would see un-generated (air) blocks across
    // that border and grow a phantom wall of faces there.
    for (int cz = 0; cz < ChunksZ; ++cz) {
        for (int cx = 0; cx < ChunksX; ++cx) {
            chunkAt(cx, cz).generate(seed, cx * Chunk::SizeX, cz * Chunk::SizeZ);
        }
    }
    remesh();
}

void World::remesh() {
    for (int cz = 0; cz < ChunksZ; ++cz) {
        for (int cx = 0; cx < ChunksX; ++cx) {
            rebuildChunkMesh(cx, cz);
        }
    }
}

BlockType World::getBlock(int x, int y, int z) const {
    if (x < 0 || x >= SizeX || y < 0 || y >= SizeY || z < 0 || z >= SizeZ) {
        return BlockType::Air;
    }
    int cx = x / Chunk::SizeX;
    int cz = z / Chunk::SizeZ;
    return chunkAt(cx, cz).getBlock(x - cx * Chunk::SizeX, y, z - cz * Chunk::SizeZ);
}

void World::setBlock(int x, int y, int z, BlockType type) {
    if (x < 0 || x >= SizeX || y < 0 || y >= SizeY || z < 0 || z >= SizeZ) {
        return;
    }
    int cx = x / Chunk::SizeX;
    int cz = z / Chunk::SizeZ;
    int lx = x - cx * Chunk::SizeX;
    int lz = z - cz * Chunk::SizeZ;

    chunkAt(cx, cz).setBlock(lx, y, lz, type);
    rebuildChunkMesh(cx, cz);

    // An edit right on a chunk seam changes what its neighbor across
    // that seam sees too (their shared faces may now need culling, or
    // un-culling), so that neighbor's mesh needs rebuilding as well.
    if (lx == 0 && cx > 0) rebuildChunkMesh(cx - 1, cz);
    if (lx == Chunk::SizeX - 1 && cx < ChunksX - 1) rebuildChunkMesh(cx + 1, cz);
    if (lz == 0 && cz > 0) rebuildChunkMesh(cx, cz - 1);
    if (lz == Chunk::SizeZ - 1 && cz < ChunksZ - 1) rebuildChunkMesh(cx, cz + 1);
}

void World::rebuildChunkMesh(int cx, int cz) {
    chunkAt(cx, cz).rebuildMesh(atlas_, *this, cx * Chunk::SizeX, cz * Chunk::SizeZ);
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

    for (const auto& chunk : chunks_) {
        chunk->renderOpaque();
    }

    // Translucent faces (glass) are drawn in a second pass with depth
    // writes off: they still depth-*test* against everything opaque
    // (so solid blocks in front of a glass face correctly hide it),
    // but never write their own depth, so a glass face can't
    // incorrectly occlude whatever's behind it once blended.
    glDepthMask(GL_FALSE);
    for (const auto& chunk : chunks_) {
        chunk->renderTranslucent();
    }
    glDepthMask(GL_TRUE);
}

glm::vec3 World::spawnPoint() const {
    int cx = SizeX / 2;
    int cz = SizeZ / 2;
    for (int y = SizeY - 1; y >= 0; --y) {
        if (!isAir(getBlock(cx, y, cz))) {
            return glm::vec3(cx + 0.5f, y + 1.0f, cz + 0.5f); // feet resting on top of that block
        }
    }
    return glm::vec3(cx + 0.5f, SizeY / 2.0f, cz + 0.5f);
}
