#pragma once

#include <cctype>
#include <cstdint>

// A tiny hand-authored 5x7 monospace bitmap font, just enough for menu
// and HUD text: A-Z, 0-9, space, '-', '_', '.'. Anything else falls
// back to a blank glyph. Input case doesn't matter - everything renders
// uppercase.
struct Glyph {
    uint8_t rows[7]; // bit 0 = leftmost of 5 columns, bit 4 = rightmost
};

namespace font_detail {

constexpr uint8_t rowBits(const char* row5) {
    uint8_t v = 0;
    for (int i = 0; i < 5; ++i) {
        if (row5[i] == '#') {
            v = static_cast<uint8_t>(v | (1 << i));
        }
    }
    return v;
}

constexpr Glyph make(const char* r0, const char* r1, const char* r2, const char* r3,
                      const char* r4, const char* r5, const char* r6) {
    return Glyph{{rowBits(r0), rowBits(r1), rowBits(r2), rowBits(r3), rowBits(r4), rowBits(r5), rowBits(r6)}};
}

} // namespace font_detail

inline const Glyph& glyphFor(char c) {
    using font_detail::make;
    c = static_cast<char>(std::toupper(static_cast<unsigned char>(c)));

    switch (c) {
        case 'A': { static const Glyph g = make(".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"); return g; }
        case 'B': { static const Glyph g = make("####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."); return g; }
        case 'C': { static const Glyph g = make(".####", "#....", "#....", "#....", "#....", "#....", ".####"); return g; }
        case 'D': { static const Glyph g = make("####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."); return g; }
        case 'E': { static const Glyph g = make("#####", "#....", "#....", "####.", "#....", "#....", "#####"); return g; }
        case 'F': { static const Glyph g = make("#####", "#....", "#....", "####.", "#....", "#....", "#...."); return g; }
        case 'G': { static const Glyph g = make(".####", "#....", "#....", "#.###", "#...#", "#...#", ".####"); return g; }
        case 'H': { static const Glyph g = make("#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"); return g; }
        case 'I': { static const Glyph g = make("#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"); return g; }
        case 'J': { static const Glyph g = make("..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."); return g; }
        case 'K': { static const Glyph g = make("#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"); return g; }
        case 'L': { static const Glyph g = make("#....", "#....", "#....", "#....", "#....", "#....", "#####"); return g; }
        case 'M': { static const Glyph g = make("#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#"); return g; }
        case 'N': { static const Glyph g = make("#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"); return g; }
        case 'O': { static const Glyph g = make(".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."); return g; }
        case 'P': { static const Glyph g = make("####.", "#...#", "#...#", "####.", "#....", "#....", "#...."); return g; }
        case 'Q': { static const Glyph g = make(".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"); return g; }
        case 'R': { static const Glyph g = make("####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"); return g; }
        case 'S': { static const Glyph g = make(".####", "#....", "#....", ".###.", "....#", "....#", "####."); return g; }
        case 'T': { static const Glyph g = make("#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."); return g; }
        case 'U': { static const Glyph g = make("#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."); return g; }
        case 'V': { static const Glyph g = make("#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."); return g; }
        case 'W': { static const Glyph g = make("#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"); return g; }
        case 'X': { static const Glyph g = make("#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"); return g; }
        case 'Y': { static const Glyph g = make("#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."); return g; }
        case 'Z': { static const Glyph g = make("#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"); return g; }

        case '0': { static const Glyph g = make(".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."); return g; }
        case '1': { static const Glyph g = make("..#..", ".##..", "..#..", "..#..", "..#..", "..#..", "#####"); return g; }
        case '2': { static const Glyph g = make(".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"); return g; }
        case '3': { static const Glyph g = make("####.", "....#", "....#", ".###.", "....#", "....#", "####."); return g; }
        case '4': { static const Glyph g = make("...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."); return g; }
        case '5': { static const Glyph g = make("#####", "#....", "#....", "####.", "....#", "....#", "####."); return g; }
        case '6': { static const Glyph g = make(".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."); return g; }
        case '7': { static const Glyph g = make("#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."); return g; }
        case '8': { static const Glyph g = make(".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."); return g; }
        case '9': { static const Glyph g = make(".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."); return g; }

        case '-': { static const Glyph g = make(".....", ".....", ".....", "#####", ".....", ".....", "....."); return g; }
        case '_': { static const Glyph g = make(".....", ".....", ".....", ".....", ".....", ".....", "#####"); return g; }
        case '.': { static const Glyph g = make(".....", ".....", ".....", ".....", ".....", ".##..", ".##.."); return g; }

        case ' ':
        default: { static const Glyph g = make(".....", ".....", ".....", ".....", ".....", ".....", "....."); return g; }
    }
}
