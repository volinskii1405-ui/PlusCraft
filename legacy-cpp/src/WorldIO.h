#pragma once

#include "World.h"

#include <cstdint>
#include <glm/glm.hpp>
#include <memory>
#include <optional>
#include <string>
#include <vector>

// Save/load of .wrld files: a small binary format holding every
// chunk's raw block data plus enough metadata (seed, player position,
// selected hotbar slot) to drop the player back where they left off.
// Also lists what's in the worlds/ save directory for the menu.
namespace worldio {

// Ensures worlds/ exists (relative to the current working directory)
// and returns its path.
std::string worldsDirectory();

// worlds/<sanitized name>.wrld - strips anything that isn't a letter,
// digit, space, '-', '_' or '.', and falls back to "world" if that
// leaves nothing.
std::string pathForName(const std::string& worldName);

// Display names (file stem, unsanitized path aside) of every *.wrld
// file in the worlds directory, alphabetical.
std::vector<std::string> listWorldNames();

bool save(const std::string& filePath, const World& world, uint32_t seed, const glm::vec3& playerFeet, int selectedHotbar);

struct LoadedWorld {
    std::unique_ptr<World> world;
    uint32_t seed = 0;
    glm::vec3 playerFeet{0.0f};
    int selectedHotbar = 0;
};
std::optional<LoadedWorld> load(const std::string& filePath);

} // namespace worldio
