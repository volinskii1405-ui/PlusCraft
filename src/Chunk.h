#pragma once

#include "Block.h"
#include "Texture.h"
#include "gl_core33.h"

#include <cstdint>
#include <vector>

class World;

// A single, fixed-size column of blocks with its own GPU mesh. World
// tiles several of these together into the full playable area.
class Chunk {
public:
    static constexpr int SizeX = 32;
    static constexpr int SizeY = 48;
    static constexpr int SizeZ = 32;

    Chunk();
    ~Chunk();

    Chunk(const Chunk&) = delete;
    Chunk& operator=(const Chunk&) = delete;

    // worldOffsetX/Z is this chunk's origin in world-space block
    // coordinates, so terrain noise (and thus the heightmap) is
    // continuous across chunk boundaries instead of repeating per chunk.
    void generate(uint32_t seed, int worldOffsetX, int worldOffsetZ);

    BlockType getBlock(int x, int y, int z) const;
    void setBlock(int x, int y, int z, BlockType type);

    // For saving/loading: the flat block array in the same order
    // index(x,y,z) produces. loadRawBlocks() is a no-op if the size
    // doesn't match (e.g. a save file from a build with different
    // chunk dimensions).
    const std::vector<BlockType>& rawBlocks() const { return blocks_; }
    void loadRawBlocks(const std::vector<BlockType>& blocks);

    // Rebuilds the CPU-side face mesh (culling hidden faces) and
    // re-uploads it to the GPU, baking worldOffsetX/Z into the vertex
    // positions so the mesh renders directly in world space with no
    // per-chunk model matrix needed. Neighbor lookups at this chunk's
    // own edges go through `world` (in world-space coordinates) so
    // faces are culled correctly against whatever chunk is next door.
    // Call once after generate(), and again after any setBlock() that
    // should become visible.
    void rebuildMesh(const TextureAtlas& atlas, const World& world, int worldOffsetX, int worldOffsetZ);
    void render() const;

    static bool inBounds(int x, int y, int z);

private:
    std::vector<BlockType> blocks_;
    GLuint vao_ = 0;
    GLuint vbo_ = 0;
    GLsizei vertexCount_ = 0;

    static size_t index(int x, int y, int z);
};
