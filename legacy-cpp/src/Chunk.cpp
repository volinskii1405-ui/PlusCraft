#include "Chunk.h"
#include "Noise.h"
#include "World.h"

#include <cstdlib>
#include <glm/glm.hpp>

namespace {

struct FaceDef {
    glm::vec3 corners[4];
    glm::vec3 normal;
};

// Corners are listed walking the quad's perimeter (winding doesn't
// matter since the renderer never culls back faces).
const FaceDef kFaceDefs[6] = {
    {{{1, 0, 0}, {1, 0, 1}, {1, 1, 1}, {1, 1, 0}}, {1, 0, 0}},   // +X
    {{{0, 0, 1}, {0, 0, 0}, {0, 1, 0}, {0, 1, 1}}, {-1, 0, 0}},  // -X
    {{{0, 1, 1}, {1, 1, 1}, {1, 1, 0}, {0, 1, 0}}, {0, 1, 0}},   // +Y
    {{{0, 0, 0}, {1, 0, 0}, {1, 0, 1}, {0, 0, 1}}, {0, -1, 0}},  // -Y
    {{{1, 0, 1}, {0, 0, 1}, {0, 1, 1}, {1, 1, 1}}, {0, 0, 1}},   // +Z
    {{{0, 0, 0}, {1, 0, 0}, {1, 1, 0}, {0, 1, 0}}, {0, 0, -1}},  // -Z
};

const glm::vec2 kCornerUV[4] = {{0, 0}, {1, 0}, {1, 1}, {0, 1}};

struct Offset {
    int dx, dy, dz;
};
const Offset kNeighborOffsets[6] = {
    {1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0}, {0, 0, 1}, {0, 0, -1},
};

Face faceFromIndex(int i) {
    switch (i) {
        case 0: return Face::PosX;
        case 1: return Face::NegX;
        case 2: return Face::PosY;
        case 3: return Face::NegY;
        case 4: return Face::PosZ;
        default: return Face::NegZ;
    }
}

void pushVertex(std::vector<float>& verts, glm::vec3 pos, glm::vec3 normal, glm::vec2 uv, float light) {
    verts.push_back(pos.x);
    verts.push_back(pos.y);
    verts.push_back(pos.z);
    verts.push_back(normal.x);
    verts.push_back(normal.y);
    verts.push_back(normal.z);
    verts.push_back(uv.x);
    verts.push_back(uv.y);
    verts.push_back(light);
}

void setUpMeshBuffers(GLuint& vao, GLuint& vbo) {
    glGenVertexArrays(1, &vao);
    glGenBuffers(1, &vbo);

    glBindVertexArray(vao);
    glBindBuffer(GL_ARRAY_BUFFER, vbo);

    const GLsizei stride = 9 * sizeof(float);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, stride, reinterpret_cast<void*>(0));
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, stride, reinterpret_cast<void*>(3 * sizeof(float)));
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(2, 2, GL_FLOAT, GL_FALSE, stride, reinterpret_cast<void*>(6 * sizeof(float)));
    glEnableVertexAttribArray(2);
    glVertexAttribPointer(3, 1, GL_FLOAT, GL_FALSE, stride, reinterpret_cast<void*>(8 * sizeof(float)));
    glEnableVertexAttribArray(3);

    glBindVertexArray(0);
}

} // namespace

Chunk::Chunk() : blocks_(static_cast<size_t>(SizeX) * SizeY * SizeZ, BlockType::Air) {
    setUpMeshBuffers(vao_, vbo_);
    setUpMeshBuffers(vaoTranslucent_, vboTranslucent_);
}

Chunk::~Chunk() {
    glDeleteBuffers(1, &vbo_);
    glDeleteVertexArrays(1, &vao_);
    glDeleteBuffers(1, &vboTranslucent_);
    glDeleteVertexArrays(1, &vaoTranslucent_);
}

bool Chunk::inBounds(int x, int y, int z) {
    return x >= 0 && x < SizeX && y >= 0 && y < SizeY && z >= 0 && z < SizeZ;
}

size_t Chunk::index(int x, int y, int z) {
    return static_cast<size_t>(y) * SizeX * SizeZ + static_cast<size_t>(z) * SizeX + static_cast<size_t>(x);
}

BlockType Chunk::getBlock(int x, int y, int z) const {
    if (!inBounds(x, y, z)) {
        return BlockType::Air;
    }
    return blocks_[index(x, y, z)];
}

void Chunk::setBlock(int x, int y, int z, BlockType type) {
    if (!inBounds(x, y, z)) {
        return;
    }
    blocks_[index(x, y, z)] = type;
}

void Chunk::loadRawBlocks(const std::vector<BlockType>& blocks) {
    if (blocks.size() == blocks_.size()) {
        blocks_ = blocks;
    }
}

void Chunk::generate(uint32_t seed, int worldOffsetX, int worldOffsetZ) {
    std::vector<int> heights(static_cast<size_t>(SizeX) * SizeZ);

    for (int z = 0; z < SizeZ; ++z) {
        for (int x = 0; x < SizeX; ++x) {
            // Sampled in world-space coordinates so the heightmap is one
            // continuous field across chunk boundaries, not a repeating
            // per-chunk pattern.
            float n = noise::fractal2D((worldOffsetX + x) * 0.07f, (worldOffsetZ + z) * 0.07f, seed);
            int height = 18 + static_cast<int>(n * 16.0f);
            heights[static_cast<size_t>(z) * SizeX + x] = height;
        }
    }

    for (int z = 0; z < SizeZ; ++z) {
        for (int x = 0; x < SizeX; ++x) {
            int height = heights[static_cast<size_t>(z) * SizeX + x];
            bool beach = height <= 20;

            for (int y = 0; y < height && y < SizeY; ++y) {
                BlockType type;
                if (y < height - 4) {
                    type = BlockType::Stone;
                } else if (y < height - 1) {
                    type = beach ? BlockType::Sand : BlockType::Dirt;
                } else {
                    type = beach ? BlockType::Sand : BlockType::Grass;
                }
                setBlock(x, y, z, type);
            }
        }
    }

    // Scatter a few trees on grass, away from the chunk edges so their
    // canopy always fits.
    for (int z = 3; z < SizeZ - 3; ++z) {
        for (int x = 3; x < SizeX - 3; ++x) {
            int height = heights[static_cast<size_t>(z) * SizeX + x];
            if (height <= 20 || height + 6 >= SizeY) {
                continue; // no trees on the beach, or too close to the world ceiling
            }
            if (noise::rand01(worldOffsetX + x, worldOffsetZ + z, seed + 999) > 0.985f) {
                int trunkTop = height + 3;
                for (int y = height; y < trunkTop; ++y) {
                    setBlock(x, y, z, BlockType::Wood);
                }
                for (int ly = trunkTop - 1; ly <= trunkTop + 1; ++ly) {
                    int radius = (ly >= trunkTop + 1) ? 1 : 2;
                    for (int lx = -radius; lx <= radius; ++lx) {
                        for (int lz = -radius; lz <= radius; ++lz) {
                            if (lx == 0 && lz == 0 && ly < trunkTop + 1) {
                                continue; // keep the trunk itself as wood
                            }
                            if (std::abs(lx) == radius && std::abs(lz) == radius) {
                                continue; // round the canopy's corners off
                            }
                            setBlock(x + lx, ly, z + lz, BlockType::Leaves);
                        }
                    }
                }
            }
        }
    }
}

void Chunk::rebuildMesh(const TextureAtlas& atlas, const World& world, int worldOffsetX, int worldOffsetZ) {
    std::vector<float> verts;
    verts.reserve(4096);
    std::vector<float> translucentVerts;

    for (int y = 0; y < SizeY; ++y) {
        for (int z = 0; z < SizeZ; ++z) {
            for (int x = 0; x < SizeX; ++x) {
                BlockType type = getBlock(x, y, z);
                if (isAir(type)) {
                    continue;
                }
                std::vector<float>& dest = isTranslucent(type) ? translucentVerts : verts;

                for (int f = 0; f < 6; ++f) {
                    const Offset& off = kNeighborOffsets[f];
                    // Always go through World (in world-space coordinates)
                    // rather than this chunk's own getBlock: at this
                    // chunk's edges, the neighbor lives in an adjacent
                    // chunk, and World is the only thing that knows
                    // about those.
                    int nx = worldOffsetX + x + off.dx;
                    int ny = y + off.dy;
                    int nz = worldOffsetZ + z + off.dz;
                    BlockType neighbor = world.getBlock(nx, ny, nz);
                    if (!(isTransparent(neighbor) && neighbor != type)) {
                        continue;
                    }

                    // Lit by whatever the face actually opens onto, not
                    // the solid block behind it - a cave wall next to an
                    // unlit air pocket should read dark even though the
                    // block itself sits under open sky elsewhere.
                    float light = world.skylightAt(nx, ny, nz);

                    Face face = faceFromIndex(f);
                    TextureAtlas::UV uv = atlas.uvFor(type, face);
                    const FaceDef& def = kFaceDefs[f];
                    glm::vec3 base(static_cast<float>(worldOffsetX + x), static_cast<float>(y), static_cast<float>(worldOffsetZ + z));

                    glm::vec3 quadPos[4];
                    glm::vec2 quadUV[4];
                    for (int c = 0; c < 4; ++c) {
                        quadPos[c] = base + def.corners[c];
                        quadUV[c] = {
                            uv.u0 + kCornerUV[c].x * (uv.u1 - uv.u0),
                            uv.v0 + kCornerUV[c].y * (uv.v1 - uv.v0),
                        };
                    }

                    const int triOrder[6] = {0, 1, 2, 0, 2, 3};
                    for (int t : triOrder) {
                        pushVertex(dest, quadPos[t], def.normal, quadUV[t], light);
                    }
                }
            }
        }
    }

    vertexCount_ = static_cast<GLsizei>(verts.size() / 9);
    glBindBuffer(GL_ARRAY_BUFFER, vbo_);
    glBufferData(GL_ARRAY_BUFFER, static_cast<GLsizeiptr>(verts.size() * sizeof(float)), verts.data(), GL_DYNAMIC_DRAW);

    vertexCountTranslucent_ = static_cast<GLsizei>(translucentVerts.size() / 9);
    glBindBuffer(GL_ARRAY_BUFFER, vboTranslucent_);
    glBufferData(GL_ARRAY_BUFFER, static_cast<GLsizeiptr>(translucentVerts.size() * sizeof(float)), translucentVerts.data(), GL_DYNAMIC_DRAW);
}

void Chunk::renderOpaque() const {
    if (vertexCount_ == 0) {
        return;
    }
    glBindVertexArray(vao_);
    glDrawArrays(GL_TRIANGLES, 0, vertexCount_);
    glBindVertexArray(0);
}

void Chunk::renderTranslucent() const {
    if (vertexCountTranslucent_ == 0) {
        return;
    }
    glBindVertexArray(vaoTranslucent_);
    glDrawArrays(GL_TRIANGLES, 0, vertexCountTranslucent_);
    glBindVertexArray(0);
}
