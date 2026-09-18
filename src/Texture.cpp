#include "Texture.h"
#include "Noise.h"

#include <array>
#include <vector>

namespace {

enum Tile {
    TileGrassTop = 0,
    TileGrassSide = 1,
    TileDirt = 2,
    TileStone = 3,
    TileSand = 4,
    TileWoodSide = 5,
    TileWoodTop = 6,
    TileLeaves = 7,
    TilePlanks = 8,
    TileWool = 9,
    TileGlass = 10,
};

struct RGB {
    int r, g, b;
};

void putPixel(std::vector<uint8_t>& pixels, int atlasW, int x, int y, RGB c, uint8_t a) {
    size_t i = (static_cast<size_t>(y) * atlasW + x) * 4;
    pixels[i + 0] = static_cast<uint8_t>(c.r);
    pixels[i + 1] = static_cast<uint8_t>(c.g);
    pixels[i + 2] = static_cast<uint8_t>(c.b);
    pixels[i + 3] = a;
}

int clampByte(float v) {
    if (v < 0.0f) return 0;
    if (v > 255.0f) return 255;
    return static_cast<int>(v);
}

RGB shade(RGB base, float noise01, float strength) {
    float factor = 1.0f - strength + noise01 * (2.0f * strength);
    return {clampByte(base.r * factor), clampByte(base.g * factor), clampByte(base.b * factor)};
}

void paintTile(std::vector<uint8_t>& pixels, int atlasW, int tile, RGB base, float strength, uint32_t seed) {
    for (int y = 0; y < TextureAtlas::TileSize; ++y) {
        for (int x = 0; x < TextureAtlas::TileSize; ++x) {
            float n = noise::rand01(x, y, seed);
            RGB c = shade(base, n, strength);
            putPixel(pixels, atlasW, tile * TextureAtlas::TileSize + x, y, c, 255);
        }
    }
}

void paintGrassSide(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB grassColor{92, 150, 63};
    const RGB dirtColor{121, 85, 58};
    // Pixel row y is uploaded as texture row y, which is v = y / TileSize
    // in our UV mapping (local block y=0 -> v=0 -> row 0). So the grass
    // strip needs to live in the *high* rows to end up at the top (v=1)
    // of the side face, not the low ones.
    for (int y = 0; y < TextureAtlas::TileSize; ++y) {
        for (int x = 0; x < TextureAtlas::TileSize; ++x) {
            float n = noise::rand01(x, y, seed);
            bool grassRow = y >= TextureAtlas::TileSize - 4;
            RGB c = grassRow ? shade(grassColor, n, 0.18f) : shade(dirtColor, n, 0.18f);
            putPixel(pixels, atlasW, TileGrassSide * TextureAtlas::TileSize + x, y, c, 255);
        }
    }
}

void paintWoodSide(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB bark{92, 64, 39};
    const RGB darkBark{66, 45, 27};
    for (int y = 0; y < TextureAtlas::TileSize; ++y) {
        for (int x = 0; x < TextureAtlas::TileSize; ++x) {
            float n = noise::rand01(x, y, seed);
            bool stripe = ((x + (y / 4)) % 4) == 0;
            RGB c = shade(stripe ? darkBark : bark, n, 0.12f);
            putPixel(pixels, atlasW, TileWoodSide * TextureAtlas::TileSize + x, y, c, 255);
        }
    }
}

void paintWoodTop(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB light{176, 140, 92};
    const RGB dark{140, 104, 64};
    float cx = TextureAtlas::TileSize / 2.0f - 0.5f;
    float cy = TextureAtlas::TileSize / 2.0f - 0.5f;
    for (int y = 0; y < TextureAtlas::TileSize; ++y) {
        for (int x = 0; x < TextureAtlas::TileSize; ++x) {
            float dist = std::sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
            bool ring = static_cast<int>(dist) % 3 == 0;
            float n = noise::rand01(x, y, seed);
            RGB c = shade(ring ? dark : light, n, 0.1f);
            putPixel(pixels, atlasW, TileWoodTop * TextureAtlas::TileSize + x, y, c, 255);
        }
    }
}

void paintStone(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB base{125, 126, 129};
    for (int y = 0; y < TextureAtlas::TileSize; ++y) {
        for (int x = 0; x < TextureAtlas::TileSize; ++x) {
            // Low-frequency blotches (sampled on a coarser grid than one
            // noise value per pixel) read as mottled stone; per-pixel
            // noise at uniform strength is what made this look like
            // gravel instead.
            float blotch = noise::value2D(x * 0.3f, y * 0.3f, seed);
            RGB c = shade(base, blotch, 0.14f);

            // Sparse dark pits and light flecks for a bit of rock
            // texture without every pixel flickering independently.
            float fleck = noise::rand01(x, y, seed + 500);
            if (fleck > 0.92f) {
                c = shade(c, 0.0f, 0.30f);
            } else if (fleck < 0.05f) {
                c = shade(c, 1.0f, 0.10f);
            }

            putPixel(pixels, atlasW, TileStone * TextureAtlas::TileSize + x, y, c, 255);
        }
    }
}

void paintPlanks(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB planks[3] = {{176, 138, 84}, {186, 147, 91}, {166, 129, 78}};
    for (int y = 0; y < TextureAtlas::TileSize; ++y) {
        int plank = y / 4; // 4 horizontal planks, 4px tall each
        RGB base = planks[plank % 3];
        bool seam = (y % 4) == 0; // a seam line between planks
        for (int x = 0; x < TextureAtlas::TileSize; ++x) {
            float n = noise::rand01(x, y, seed);
            RGB c = shade(base, n, 0.08f);
            if (seam) {
                c = shade(c, 0.0f, 0.35f);
            } else if (((x + plank * 3) % 8) == 0) {
                c = shade(c, 0.0f, 0.15f); // a faint grain line down each plank
            }
            putPixel(pixels, atlasW, TilePlanks * TextureAtlas::TileSize + x, y, c, 255);
        }
    }
}

void paintWool(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB base{218, 218, 210};
    const int size = TextureAtlas::TileSize;
    for (int y = 0; y < size; ++y) {
        for (int x = 0; x < size; ++x) {
            // Soft, coarse blotches for a felted look, not the tighter
            // per-pixel grain a mineral block like stone or sand wants.
            float blotch = noise::value2D(x * 0.4f, y * 0.4f, seed);
            RGB c = shade(base, blotch, 0.10f);

            // A faint woven grid, like strands of yarn.
            if (x % 4 == 0 || y % 4 == 0) {
                c = shade(c, 0.0f, 0.06f);
            }

            // A few small, darker fiber flecks scattered around.
            float fleck = noise::rand01(x, y, seed + 700);
            if (fleck > 0.94f) {
                c = shade(c, 0.0f, 0.22f);
            }

            putPixel(pixels, atlasW, TileWool * size + x, y, c, 255);
        }
    }
}

void paintGlass(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB pane{215, 233, 235};
    const RGB frame{238, 247, 248};
    const int size = TextureAtlas::TileSize;
    for (int y = 0; y < size; ++y) {
        for (int x = 0; x < size; ++x) {
            float n = noise::rand01(x, y, seed);
            bool border = x == 0 || y == 0 || x == size - 1 || y == size - 1;
            RGB c = shade(border ? frame : pane, n, 0.05f);
            // Still clearly see-through, but enough tint that a pane
            // reads as "a block is there" rather than "nothing here" -
            // a brighter, less transparent frame around the edge helps
            // that too.
            uint8_t alpha = border ? 205 : 90;
            putPixel(pixels, atlasW, TileGlass * size + x, y, c, alpha);
        }
    }
}

void paintLeaves(std::vector<uint8_t>& pixels, int atlasW, uint32_t seed) {
    const RGB leaf{58, 110, 42};
    for (int y = 0; y < TextureAtlas::TileSize; ++y) {
        for (int x = 0; x < TextureAtlas::TileSize; ++x) {
            float n = noise::rand01(x, y, seed);
            RGB c = shade(leaf, n, 0.25f);
            uint8_t alpha = (n < 0.14f) ? 0 : 255;
            putPixel(pixels, atlasW, TileLeaves * TextureAtlas::TileSize + x, y, c, alpha);
        }
    }
}

} // namespace

TextureAtlas::TextureAtlas() {
    const int atlasW = TileSize * TileCount;
    const int atlasH = TileSize;
    std::vector<uint8_t> pixels(static_cast<size_t>(atlasW) * atlasH * 4, 255);

    paintTile(pixels, atlasW, TileGrassTop, {92, 150, 63}, 0.18f, 11);
    paintGrassSide(pixels, atlasW, 22);
    paintTile(pixels, atlasW, TileDirt, {121, 85, 58}, 0.18f, 33);
    paintStone(pixels, atlasW, 44);
    paintTile(pixels, atlasW, TileSand, {219, 205, 157}, 0.12f, 55);
    paintWoodSide(pixels, atlasW, 66);
    paintWoodTop(pixels, atlasW, 77);
    paintLeaves(pixels, atlasW, 88);
    paintPlanks(pixels, atlasW, 99);
    paintWool(pixels, atlasW, 110);
    paintGlass(pixels, atlasW, 121);

    glGenTextures(1, &textureId_);
    glBindTexture(GL_TEXTURE_2D, textureId_);
    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, atlasW, atlasH, 0, GL_RGBA, GL_UNSIGNED_BYTE, pixels.data());
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glBindTexture(GL_TEXTURE_2D, 0);
}

TextureAtlas::~TextureAtlas() {
    if (textureId_ != 0) {
        glDeleteTextures(1, &textureId_);
    }
}

void TextureAtlas::bind(GLenum unit) const {
    glActiveTexture(unit);
    glBindTexture(GL_TEXTURE_2D, textureId_);
}

TextureAtlas::UV TextureAtlas::uvFor(BlockType type, Face face) const {
    int tile = TileStone;
    switch (type) {
        case BlockType::Grass:
            tile = (face == Face::PosY) ? TileGrassTop : (face == Face::NegY) ? TileDirt : TileGrassSide;
            break;
        case BlockType::Dirt:
            tile = TileDirt;
            break;
        case BlockType::Stone:
            tile = TileStone;
            break;
        case BlockType::Sand:
            tile = TileSand;
            break;
        case BlockType::Wood:
            tile = (face == Face::PosY || face == Face::NegY) ? TileWoodTop : TileWoodSide;
            break;
        case BlockType::Leaves:
            tile = TileLeaves;
            break;
        case BlockType::Planks:
            tile = TilePlanks;
            break;
        case BlockType::Wool:
            tile = TileWool;
            break;
        case BlockType::Glass:
            tile = TileGlass;
            break;
        default:
            tile = TileStone;
            break;
    }

    float u0 = static_cast<float>(tile) / TileCount;
    float u1 = static_cast<float>(tile + 1) / TileCount;
    return {u0, 0.0f, u1, 1.0f};
}
