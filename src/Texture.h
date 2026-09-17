#pragma once

#include "Block.h"
#include "gl_core33.h"

// Procedurally generates a small texture atlas (8 tiles of TileSize x
// TileSize, laid out in a single row) so the game needs no external
// image assets. Tiles get per-pixel noise variation so they read as
// "textured" rather than flat-shaded.
class TextureAtlas {
public:
    static constexpr int TileSize = 16;
    static constexpr int TileCount = 8;

    TextureAtlas();
    ~TextureAtlas();

    TextureAtlas(const TextureAtlas&) = delete;
    TextureAtlas& operator=(const TextureAtlas&) = delete;

    void bind(GLenum unit = GL_TEXTURE0) const;
    GLuint id() const { return textureId_; }

    struct UV {
        float u0, v0, u1, v1;
    };
    UV uvFor(BlockType type, Face face) const;

private:
    GLuint textureId_ = 0;
};
