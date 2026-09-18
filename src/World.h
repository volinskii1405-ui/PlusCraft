#pragma once

#include "Block.h"
#include "Chunk.h"
#include "Shader.h"
#include "Texture.h"

#include <array>
#include <cstdint>
#include <glm/glm.hpp>
#include <memory>
#include <vector>

// The world is a fixed ChunksX x ChunksZ grid of Chunks (currently
// 4x4 = 16), tiled seamlessly: terrain noise and face culling both go
// through world-space coordinates, so nothing seams at chunk borders.
class World {
public:
    static constexpr int ChunksX = 4;
    static constexpr int ChunksZ = 4;
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

    // 1.0 if (x,y,z) has a clear vertical line to the sky (only air,
    // leaves and/or glass above it, all the way to the world ceiling),
    // 0.0 otherwise - e.g. a mined-out pit with a roof over it, or the
    // bottom of a tunnel. Out-of-range x/z reads as lit. Used to darken
    // faces that front onto an unlit cell (see Chunk::rebuildMesh).
    float skylightAt(int x, int y, int z) const;

    // For saving/loading (see WorldIO). Chunk-local coordinates.
    const std::vector<BlockType>& chunkBlocks(int cx, int cz) const { return chunkAt(cx, cz).rawBlocks(); }
    void loadChunkBlocks(int cx, int cz, const std::vector<BlockType>& blocks) { chunkAt(cx, cz).loadRawBlocks(blocks); }
    void remesh();

private:
    Chunk& chunkAt(int cx, int cz) { return *chunks_[static_cast<size_t>(cz) * ChunksX + cx]; }
    const Chunk& chunkAt(int cx, int cz) const { return *chunks_[static_cast<size_t>(cz) * ChunksX + cx]; }
    void rebuildChunkMesh(int cx, int cz);

    // Highest block in world-space column (x,z) that blocks skylight
    // (i.e. isn't air/leaves/glass), or -1 if the column is clear all
    // the way down. A column is entirely within one chunk (chunks only
    // tile in X/Z, not Y), so editing a block only ever needs to
    // recompute its own column and rebuild its own chunk.
    void recomputeColumnLight(int x, int z);
    void recomputeAllColumnLight();
    static size_t columnIndex(int x, int z) { return static_cast<size_t>(z) * SizeX + x; }

    std::array<std::unique_ptr<Chunk>, ChunksX * ChunksZ> chunks_;
    TextureAtlas atlas_;
    std::vector<int16_t> topOpaqueY_;
};
