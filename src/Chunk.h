#pragma once

#include "Block.h"
#include "Texture.h"
#include "gl_core33.h"

#include <cstdint>
#include <vector>

// A single, fixed-size column of blocks with its own GPU mesh. The MVP
// world is exactly one Chunk (see World).
class Chunk {
public:
    static constexpr int SizeX = 32;
    static constexpr int SizeY = 48;
    static constexpr int SizeZ = 32;

    Chunk();
    ~Chunk();

    Chunk(const Chunk&) = delete;
    Chunk& operator=(const Chunk&) = delete;

    void generate(uint32_t seed);

    BlockType getBlock(int x, int y, int z) const;
    void setBlock(int x, int y, int z, BlockType type);

    // Rebuilds the CPU-side face mesh (culling hidden faces) and
    // re-uploads it to the GPU. Call once after generate(), and again
    // after any setBlock() that should become visible.
    void rebuildMesh(const TextureAtlas& atlas);
    void render() const;

    static bool inBounds(int x, int y, int z);

private:
    std::vector<BlockType> blocks_;
    GLuint vao_ = 0;
    GLuint vbo_ = 0;
    GLsizei vertexCount_ = 0;

    static size_t index(int x, int y, int z);
};
