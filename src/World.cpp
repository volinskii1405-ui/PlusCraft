#include "World.h"

#include <algorithm>
#include <glm/gtc/matrix_transform.hpp>
#include <queue>

World::World(uint32_t seed, bool generateTerrain) {
    for (int cz = 0; cz < ChunksZ; ++cz) {
        for (int cx = 0; cx < ChunksX; ++cx) {
            chunks_[static_cast<size_t>(cz) * ChunksX + cx] = std::make_unique<Chunk>();
        }
    }
    lightLevels_.assign(static_cast<size_t>(SizeX) * SizeY * SizeZ, 0);

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
    computeLighting();
    rebuildAllChunkMeshes();
}

void World::rebuildAllChunkMeshes() {
    for (int cz = 0; cz < ChunksZ; ++cz) {
        for (int cx = 0; cx < ChunksX; ++cx) {
            rebuildChunkMesh(cx, cz);
        }
    }
}

void World::computeLighting() {
    std::fill(lightLevels_.begin(), lightLevels_.end(), 0);
    std::vector<bool> visited(lightLevels_.size(), false);
    std::queue<glm::ivec3> queue;

    // Seed every column from the top: light stays at full strength
    // straight down through open sky until it hits the first solid
    // (non-transparent) block, which is as far as this initial pass
    // goes - anything below that (a roofed-over gap, the far side of
    // an opening) only gets lit by the BFS spreading sideways/upward
    // from these seeds afterward.
    for (int z = 0; z < SizeZ; ++z) {
        for (int x = 0; x < SizeX; ++x) {
            for (int y = SizeY - 1; y >= 0; --y) {
                if (!isTransparent(getBlock(x, y, z))) {
                    break;
                }
                size_t idx = voxelIndex(x, y, z);
                lightLevels_[idx] = static_cast<uint8_t>(MaxLight);
                visited[idx] = true;
                queue.push(glm::ivec3(x, y, z));
            }
        }
    }

    static const glm::ivec3 kNeighbors[6] = {
        {1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0}, {0, 0, 1}, {0, 0, -1},
    };

    while (!queue.empty()) {
        glm::ivec3 cell = queue.front();
        queue.pop();
        int level = lightLevels_[voxelIndex(cell.x, cell.y, cell.z)];
        if (level <= 1) {
            continue; // neighbors would only get level 0, nothing to spread
        }
        for (const glm::ivec3& off : kNeighbors) {
            glm::ivec3 n = cell + off;
            if (n.x < 0 || n.x >= SizeX || n.y < 0 || n.y >= SizeY || n.z < 0 || n.z >= SizeZ) {
                continue;
            }
            size_t nIdx = voxelIndex(n.x, n.y, n.z);
            if (visited[nIdx] || !isTransparent(getBlock(n.x, n.y, n.z))) {
                continue;
            }
            visited[nIdx] = true;
            lightLevels_[nIdx] = static_cast<uint8_t>(level - 1);
            queue.push(n);
        }
    }
}

float World::skylightAt(int x, int y, int z) const {
    if (x < 0 || x >= SizeX || z < 0 || z >= SizeZ) {
        return 1.0f;
    }
    if (y >= SizeY) {
        return 1.0f; // above the world ceiling: open sky
    }
    if (y < 0) {
        return 0.0f; // below the world floor
    }
    return static_cast<float>(lightLevels_[voxelIndex(x, y, z)]) / static_cast<float>(MaxLight);
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

    // Light can spread arbitrarily far from a single edit (breaking
    // into a sealed cavern floods the whole thing), so every chunk's
    // lighting and mesh needs a fresh look, not just the edited one
    // and its immediate seam neighbors.
    computeLighting();
    rebuildAllChunkMeshes();
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
