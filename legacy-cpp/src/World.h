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

    // Light level at (x,y,z) as a 0..1 fraction of MaxLight, for
    // however many of the MaxLight+1 discrete steps that skylight has
    // faded through by the time it reaches this cell (see
    // computeLighting()). 1.0 out in the open, fading down toward 0.0
    // the further a cell is from open sky - e.g. a mined-out pit gets
    // gradually darker as you dig deeper, not an abrupt on/off cutoff.
    // Out-of-x/z-range reads as lit; above the world ceiling reads as
    // lit; below the world floor reads as dark. Used to shade faces
    // that front onto a given cell (see Chunk::rebuildMesh).
    float skylightAt(int x, int y, int z) const;

    // For saving/loading (see WorldIO). Chunk-local coordinates.
    const std::vector<BlockType>& chunkBlocks(int cx, int cz) const { return chunkAt(cx, cz).rawBlocks(); }
    void loadChunkBlocks(int cx, int cz, const std::vector<BlockType>& blocks) { chunkAt(cx, cz).loadRawBlocks(blocks); }
    void remesh();

private:
    Chunk& chunkAt(int cx, int cz) { return *chunks_[static_cast<size_t>(cz) * ChunksX + cx]; }
    const Chunk& chunkAt(int cx, int cz) const { return *chunks_[static_cast<size_t>(cz) * ChunksX + cx]; }
    void rebuildChunkMesh(int cx, int cz);
    void rebuildAllChunkMeshes();

    // Multi-source BFS flood fill: every transparent cell with a clear
    // vertical line to the world ceiling seeds at MaxLight, then light
    // spreads outward through transparent cells losing one step per
    // block traveled (a plain unweighted BFS, since every seed starts
    // equal and every edge costs 1 - so visiting cells in BFS order
    // already assigns each one its correct, first-and-final level).
    // Lighting can in principle change arbitrarily far from an edited
    // block (e.g. breaking through into a sealed cavern floods the
    // whole thing), so this - like every chunk's mesh - is recomputed
    // from scratch on every edit rather than incrementally patched;
    // the world is small enough (16 chunks) for that to be cheap.
    void computeLighting();
    static constexpr int MaxLight = 8;
    static size_t voxelIndex(int x, int y, int z) {
        return (static_cast<size_t>(y) * SizeZ + static_cast<size_t>(z)) * SizeX + static_cast<size_t>(x);
    }

    std::array<std::unique_ptr<Chunk>, ChunksX * ChunksZ> chunks_;
    TextureAtlas atlas_;
    std::vector<uint8_t> lightLevels_;
};
