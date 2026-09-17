#pragma once

#include "Block.h"
#include "Chunk.h"
#include "Shader.h"
#include "Texture.h"

#include <array>
#include <glm/glm.hpp>
#include <memory>
#include <vector>

// The world is a fixed ChunksX x ChunksZ grid of Chunks (currently
// 2x2 = 4), tiled seamlessly: terrain noise and face culling both go
// through world-space coordinates, so nothing seams at chunk borders.
class World {
public:
    static constexpr int ChunksX = 2;
    static constexpr int ChunksZ = 2;
    static constexpr int SizeX = Chunk::SizeX * ChunksX;
    static constexpr int SizeY = Chunk::SizeY;
    static constexpr int SizeZ = Chunk::SizeZ * ChunksZ;

    struct RaycastHit {
        bool hit = false;
        glm::ivec3 blockPos{0};  // the solid block that was hit
        glm::ivec3 placePos{0};  // the empty block just before it, for placing
    };

    // generateTerrain=false makes an all-air grid instead (its chunks
    // still exist and have GPU buffers, they're just empty) - used
    // when loading a save, which fills every chunk's blocks itself via
    // loadChunkBlocks() and then calls remesh() once.
    explicit World(uint32_t seed, bool generateTerrain = true);

    BlockType getBlock(int x, int y, int z) const;
    void setBlock(int x, int y, int z, BlockType type);

    RaycastHit raycast(glm::vec3 origin, glm::vec3 direction, float maxDistance) const;

    void render(const Shader& shader) const;

    glm::vec3 spawnPoint() const;
    const TextureAtlas& atlas() const { return atlas_; }

    // For saving/loading (see WorldIO). Chunk-local coordinates.
    const std::vector<BlockType>& chunkBlocks(int cx, int cz) const { return chunkAt(cx, cz).rawBlocks(); }
    void loadChunkBlocks(int cx, int cz, const std::vector<BlockType>& blocks) { chunkAt(cx, cz).loadRawBlocks(blocks); }
    void remesh();

private:
    Chunk& chunkAt(int cx, int cz) { return *chunks_[static_cast<size_t>(cz) * ChunksX + cx]; }
    const Chunk& chunkAt(int cx, int cz) const { return *chunks_[static_cast<size_t>(cz) * ChunksX + cx]; }
    void rebuildChunkMesh(int cx, int cz);

    std::array<std::unique_ptr<Chunk>, ChunksX * ChunksZ> chunks_;
    TextureAtlas atlas_;
};
