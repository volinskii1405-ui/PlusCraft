#define MINIAUDIO_IMPLEMENTATION
#include "miniaudio.h"

#include "Audio.h"

#include <array>
#include <cmath>
#include <cstdint>
#include <random>
#include <vector>

namespace {

constexpr ma_uint32 kSampleRate = 44100;

// Which block types share a timbre. Everything not called out
// explicitly (dirt, sand, grass, air) falls into Earthy.
enum class Category { Stone = 0, Wood, Earthy, Soft, Glass, Count };

Category categoryFor(BlockType type) {
    switch (type) {
        case BlockType::Stone: return Category::Stone;
        case BlockType::Wood:
        case BlockType::Planks: return Category::Wood;
        case BlockType::Leaves:
        case BlockType::Wool: return Category::Soft;
        case BlockType::Glass: return Category::Glass;
        default: return Category::Earthy;
    }
}

// A small deterministic PRNG (xorshift32) so clip generation doesn't
// depend on global std::rand state.
struct Rng {
    uint32_t state;
    explicit Rng(uint32_t seed) : state(seed ? seed : 1) {}
    float next() {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        return (static_cast<float>(state) / 4294967295.0f) * 2.0f - 1.0f; // [-1, 1]
    }
};

// Every effect is an exponentially-decaying mix of filtered noise (a
// "thud"/"crack" transient, duller the lower lowpassAlpha is) plus an
// optional sine tone (a resonant "knock"/"ring"). Varying just these
// few knobs per material is enough to tell stone/wood/dirt/cloth/glass
// apart without needing per-block recordings.
std::vector<float> synthesize(float durationSec, float noiseAmount, float lowpassAlpha,
                               float toneFreqHz, float toneAmount, float decayRate, uint32_t seed) {
    size_t n = static_cast<size_t>(durationSec * kSampleRate);
    std::vector<float> samples(n, 0.0f);
    Rng rng(seed);
    float filtered = 0.0f;
    for (size_t i = 0; i < n; ++i) {
        float t = static_cast<float>(i) / static_cast<float>(kSampleRate);
        float envelope = std::exp(-decayRate * t);

        filtered += lowpassAlpha * (rng.next() - filtered);
        float tone = (toneAmount > 0.0f) ? std::sin(2.0f * 3.14159265f * toneFreqHz * t) : 0.0f;

        samples[i] = envelope * (noiseAmount * filtered + toneAmount * tone);
    }
    return samples;
}

struct ClipParams {
    float durationSec;
    float noiseAmount;
    float lowpassAlpha; // 1 = raw noise (bright/crisp), smaller = duller/softer
    float toneFreqHz;   // 0 = no tonal component
    float toneAmount;
    float decayRate;
};

// [category][kind]; kind order matches Kind below.
constexpr int kKindCount = 3; // Dig, Place, Step
const ClipParams kParams[static_cast<int>(Category::Count)][kKindCount] = {
    // Stone
    {{0.18f, 0.9f, 0.90f, 0.0f, 0.0f, 18.0f},
     {0.22f, 0.6f, 0.60f, 130.0f, 0.5f, 14.0f},
     {0.10f, 0.5f, 0.80f, 0.0f, 0.0f, 30.0f}},
    // Wood
    {{0.18f, 0.6f, 0.35f, 220.0f, 0.5f, 16.0f},
     {0.20f, 0.4f, 0.30f, 180.0f, 0.6f, 12.0f},
     {0.10f, 0.35f, 0.30f, 200.0f, 0.3f, 28.0f}},
    // Earthy (dirt/sand/grass)
    {{0.20f, 0.8f, 0.18f, 0.0f, 0.0f, 14.0f},
     {0.18f, 0.6f, 0.15f, 90.0f, 0.3f, 16.0f},
     {0.12f, 0.5f, 0.20f, 0.0f, 0.0f, 24.0f}},
    // Soft (leaves/wool)
    {{0.25f, 0.5f, 0.10f, 0.0f, 0.0f, 10.0f},
     {0.20f, 0.35f, 0.12f, 0.0f, 0.0f, 14.0f},
     {0.14f, 0.3f, 0.12f, 0.0f, 0.0f, 20.0f}},
    // Glass
    {{0.30f, 0.6f, 0.90f, 1500.0f, 0.5f, 9.0f},
     {0.25f, 0.15f, 0.90f, 1100.0f, 0.7f, 11.0f},
     {0.10f, 0.2f, 0.80f, 900.0f, 0.4f, 30.0f}},
};

const ClipParams kJumpParams = {0.12f, 0.7f, 0.25f, 0.0f, 0.0f, 20.0f};

} // namespace

struct Audio::Impl {
    ma_engine engine{};
    bool engineReady = false;
    std::mt19937 rng{std::random_device{}()};
    std::uniform_real_distribution<float> pitchJitter{0.94f, 1.06f};
    std::uniform_real_distribution<float> volumeJitter{0.85f, 1.0f};

    struct Clip {
        std::vector<float> samples;
        ma_audio_buffer buffer{};
        ma_sound sound{};
        bool ready = false;
    };

    std::array<Clip, static_cast<size_t>(Category::Count)> digClips;
    std::array<Clip, static_cast<size_t>(Category::Count)> placeClips;
    std::array<Clip, static_cast<size_t>(Category::Count)> stepClips;
    Clip jumpClip;

    ~Impl() {
        auto teardown = [](Clip& clip) {
            if (clip.ready) {
                ma_sound_uninit(&clip.sound);
                ma_audio_buffer_uninit(&clip.buffer);
            }
        };
        for (Clip& c : digClips) teardown(c);
        for (Clip& c : placeClips) teardown(c);
        for (Clip& c : stepClips) teardown(c);
        teardown(jumpClip);
        if (engineReady) {
            ma_engine_uninit(&engine);
        }
    }

    void initClip(Clip& clip, const ClipParams& p, uint32_t seed) {
        if (!engineReady) {
            return;
        }
        clip.samples = synthesize(p.durationSec, p.noiseAmount, p.lowpassAlpha, p.toneFreqHz, p.toneAmount, p.decayRate, seed);

        ma_audio_buffer_config cfg = ma_audio_buffer_config_init(
            ma_format_f32, 1, clip.samples.size(), clip.samples.data(), nullptr);
        cfg.sampleRate = kSampleRate;
        if (ma_audio_buffer_init(&cfg, &clip.buffer) != MA_SUCCESS) {
            return;
        }
        if (ma_sound_init_from_data_source(&engine, &clip.buffer, 0, nullptr, &clip.sound) != MA_SUCCESS) {
            ma_audio_buffer_uninit(&clip.buffer);
            return;
        }
        clip.ready = true;
    }

    void play(Clip& clip) {
        if (!engineReady || !clip.ready) {
            return;
        }
        ma_sound_stop(&clip.sound);
        ma_sound_seek_to_pcm_frame(&clip.sound, 0);
        ma_sound_set_pitch(&clip.sound, pitchJitter(rng));
        ma_sound_set_volume(&clip.sound, volumeJitter(rng));
        ma_sound_start(&clip.sound);
    }
};

Audio::Audio() : impl_(new Impl()) {
    impl_->engineReady = (ma_engine_init(nullptr, &impl_->engine) == MA_SUCCESS);

    for (int cat = 0; cat < static_cast<int>(Category::Count); ++cat) {
        // Distinct seeds per clip so materials/kinds don't all reuse
        // the exact same noise pattern.
        impl_->initClip(impl_->digClips[cat], kParams[cat][0], 1000u + cat);
        impl_->initClip(impl_->placeClips[cat], kParams[cat][1], 2000u + cat);
        impl_->initClip(impl_->stepClips[cat], kParams[cat][2], 3000u + cat);
    }
    impl_->initClip(impl_->jumpClip, kJumpParams, 9000u);
}

Audio::~Audio() {
    delete impl_;
}

void Audio::playDig(BlockType type) {
    impl_->play(impl_->digClips[static_cast<size_t>(categoryFor(type))]);
}

void Audio::playPlace(BlockType type) {
    impl_->play(impl_->placeClips[static_cast<size_t>(categoryFor(type))]);
}

void Audio::playStep(BlockType type) {
    impl_->play(impl_->stepClips[static_cast<size_t>(categoryFor(type))]);
}

void Audio::playJump() {
    impl_->play(impl_->jumpClip);
}
