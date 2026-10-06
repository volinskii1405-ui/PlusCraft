#include "WorldIO.h"

#include <algorithm>
#include <cctype>
#include <cstring>
#include <filesystem>
#include <fstream>

namespace fs = std::filesystem;

namespace worldio {

namespace {

constexpr char kMagic[4] = {'P', 'C', 'W', 'D'};
constexpr uint32_t kVersion = 1;

struct Header {
    char magic[4];
    uint32_t version;
    uint32_t seed;
    int32_t chunksX;
    int32_t chunksZ;
    int32_t chunkSizeX;
    int32_t chunkSizeY;
    int32_t chunkSizeZ;
    float playerX, playerY, playerZ;
    int32_t selectedHotbar;
};

} // namespace

std::string worldsDirectory() {
    fs::path dir = "worlds";
    std::error_code ec;
    fs::create_directories(dir, ec);
    return dir.string();
}

std::string pathForName(const std::string& worldName) {
    std::string clean;
    clean.reserve(worldName.size());
    for (char c : worldName) {
        if (std::isalnum(static_cast<unsigned char>(c)) || c == ' ' || c == '-' || c == '_' || c == '.') {
            clean += c;
        }
    }
    // Trim leading/trailing spaces left over from stripped characters.
    size_t begin = clean.find_first_not_of(' ');
    size_t end = clean.find_last_not_of(' ');
    clean = (begin == std::string::npos) ? "" : clean.substr(begin, end - begin + 1);
    if (clean.empty()) {
        clean = "world";
    }

    fs::path path = fs::path(worldsDirectory()) / (clean + ".wrld");
    return path.string();
}

std::vector<std::string> listWorldNames() {
    std::vector<std::string> names;
    std::error_code ec;
    for (const auto& entry : fs::directory_iterator(worldsDirectory(), ec)) {
        if (entry.is_regular_file() && entry.path().extension() == ".wrld") {
            names.push_back(entry.path().stem().string());
        }
    }
    std::sort(names.begin(), names.end());
    return names;
}

bool save(const std::string& filePath, const World& world, uint32_t seed, const glm::vec3& playerFeet, int selectedHotbar) {
    std::ofstream out(filePath, std::ios::binary | std::ios::trunc);
    if (!out) {
        return false;
    }

    Header header{};
    header.magic[0] = kMagic[0];
    header.magic[1] = kMagic[1];
    header.magic[2] = kMagic[2];
    header.magic[3] = kMagic[3];
    header.version = kVersion;
    header.seed = seed;
    header.chunksX = World::ChunksX;
    header.chunksZ = World::ChunksZ;
    header.chunkSizeX = Chunk::SizeX;
    header.chunkSizeY = Chunk::SizeY;
    header.chunkSizeZ = Chunk::SizeZ;
    header.playerX = playerFeet.x;
    header.playerY = playerFeet.y;
    header.playerZ = playerFeet.z;
    header.selectedHotbar = selectedHotbar;

    out.write(reinterpret_cast<const char*>(&header), sizeof(header));

    for (int cz = 0; cz < World::ChunksZ; ++cz) {
        for (int cx = 0; cx < World::ChunksX; ++cx) {
            const std::vector<BlockType>& blocks = world.chunkBlocks(cx, cz);
            out.write(reinterpret_cast<const char*>(blocks.data()), static_cast<std::streamsize>(blocks.size() * sizeof(BlockType)));
        }
    }

    return static_cast<bool>(out);
}

std::optional<LoadedWorld> load(const std::string& filePath) {
    std::ifstream in(filePath, std::ios::binary);
    if (!in) {
        return std::nullopt;
    }

    Header header{};
    in.read(reinterpret_cast<char*>(&header), sizeof(header));
    if (!in || std::memcmp(header.magic, kMagic, sizeof(kMagic)) != 0 || header.version != kVersion) {
        return std::nullopt;
    }
    // Reject saves from a build with different world/chunk dimensions
    // rather than misreading their block data.
    if (header.chunksX != World::ChunksX || header.chunksZ != World::ChunksZ ||
        header.chunkSizeX != Chunk::SizeX || header.chunkSizeY != Chunk::SizeY || header.chunkSizeZ != Chunk::SizeZ) {
        return std::nullopt;
    }

    LoadedWorld result;
    result.seed = header.seed;
    result.playerFeet = glm::vec3(header.playerX, header.playerY, header.playerZ);
    result.selectedHotbar = header.selectedHotbar;
    result.world = std::make_unique<World>(header.seed, /*generateTerrain=*/false);

    const size_t chunkBlockCount = static_cast<size_t>(Chunk::SizeX) * Chunk::SizeY * Chunk::SizeZ;
    std::vector<BlockType> buffer(chunkBlockCount);

    for (int cz = 0; cz < World::ChunksZ; ++cz) {
        for (int cx = 0; cx < World::ChunksX; ++cx) {
            in.read(reinterpret_cast<char*>(buffer.data()), static_cast<std::streamsize>(chunkBlockCount * sizeof(BlockType)));
            if (!in) {
                return std::nullopt;
            }
            result.world->loadChunkBlocks(cx, cz, buffer);
        }
    }

    result.world->remesh();
    return result;
}

} // namespace worldio
