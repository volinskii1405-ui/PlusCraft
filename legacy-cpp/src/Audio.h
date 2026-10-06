#pragma once

#include "Block.h"

// Every sound effect is synthesized in memory at startup (no bundled
// .wav/.ogg assets, same "zero external assets" approach as
// TextureAtlas) and played back through a tiny cross-platform mixer
// (miniaudio). If no audio device is available - e.g. a headless CI
// box - initialization degrades gracefully and every play* call is
// simply a no-op instead of crashing the game.
class Audio {
public:
    Audio();
    ~Audio();
    Audio(const Audio&) = delete;
    Audio& operator=(const Audio&) = delete;

    void playDig(BlockType type);
    void playPlace(BlockType type);
    void playStep(BlockType type);
    void playJump();

private:
    struct Impl;
    Impl* impl_;
};
