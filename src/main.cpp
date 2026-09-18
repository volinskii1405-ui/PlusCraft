#include "gl_core33.h"

#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>

#include "Block.h"
#include "Camera.h"
#include "Highlight.h"
#include "Menu.h"
#include "Player.h"
#include "Shader.h"
#include "Shaders.h"
#include "Ui.h"
#include "World.h"
#include "WorldIO.h"

#include <glm/gtc/matrix_transform.hpp>
#include <algorithm>
#include <array>
#include <cmath>
#include <iostream>
#include <memory>
#include <random>

namespace {

int gWindowWidth = 1280;
int gWindowHeight = 720;
bool gFirstMouse = true;
float gLastX = gWindowWidth / 2.0f;
float gLastY = gWindowHeight / 2.0f;

// GLFW only gives a window a single user pointer; both callbacks below
// need to reach different things depending on which screen is active,
// so they share this instead.
struct AppState {
    Camera* camera = nullptr; // set only once in-game
    Menu* menu = nullptr;     // set for the whole session
    int* selected = nullptr;  // hotbar index; scroll wheel steps it, in-game only
    int hotbarCount = 0;
};

void framebufferSizeCallback(GLFWwindow*, int width, int height) {
    gWindowWidth = width;
    gWindowHeight = height;
    glViewport(0, 0, width, height);
}

void mouseCallback(GLFWwindow* window, double xpos, double ypos) {
    auto* app = static_cast<AppState*>(glfwGetWindowUserPointer(window));
    if (!app || !app->camera) {
        return;
    }

    if (gFirstMouse) {
        gLastX = static_cast<float>(xpos);
        gLastY = static_cast<float>(ypos);
        gFirstMouse = false;
    }

    float xOffset = static_cast<float>(xpos) - gLastX;
    float yOffset = gLastY - static_cast<float>(ypos); // screen y grows downward
    gLastX = static_cast<float>(xpos);
    gLastY = static_cast<float>(ypos);

    app->camera->processMouseMovement(xOffset, yOffset);
}

void charCallback(GLFWwindow* window, unsigned int codepoint) {
    auto* app = static_cast<AppState*>(glfwGetWindowUserPointer(window));
    if (app && app->menu) {
        app->menu->onChar(codepoint);
    }
}

void scrollCallback(GLFWwindow* window, double /*xoffset*/, double yoffset) {
    auto* app = static_cast<AppState*>(glfwGetWindowUserPointer(window));
    if (!app || !app->camera || !app->selected || app->hotbarCount <= 0) {
        return; // camera is only set once in-game
    }
    int delta = (yoffset > 0.0) ? -1 : (yoffset < 0.0 ? 1 : 0);
    if (delta == 0) {
        return;
    }
    int n = app->hotbarCount;
    *app->selected = ((*app->selected + delta) % n + n) % n;
}

} // namespace

int main() {
    if (!glfwInit()) {
        std::cerr << "Failed to initialize GLFW\n";
        return 1;
    }

    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
#ifdef __APPLE__
    glfwWindowHint(GLFW_OPENGL_FORWARD_COMPAT, GLFW_TRUE);
#endif

    GLFWwindow* window = glfwCreateWindow(gWindowWidth, gWindowHeight, "PlusCraft", nullptr, nullptr);
    if (!window) {
        std::cerr << "Failed to create GLFW window\n";
        glfwTerminate();
        return 1;
    }

    glfwMakeContextCurrent(window);
    glfwSwapInterval(1);
    glfwSetFramebufferSizeCallback(window, framebufferSizeCallback);
    glfwSetCursorPosCallback(window, mouseCallback);
    glfwSetCharCallback(window, charCallback);
    glfwSetScrollCallback(window, scrollCallback);

    if (!glCore33Init()) {
        std::cerr << "Failed to load required OpenGL functions (need an OpenGL 3.3 capable driver)\n";
        glfwTerminate();
        return 1;
    }

    glDepthFunc(GL_LESS);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    glClearColor(0.53f, 0.80f, 0.92f, 1.0f);

    Shader blockShader(kBlockVertexShader, kBlockFragmentShader);
    Ui ui;
    Highlight highlight;
    Menu menu;

    AppState appState;
    appState.menu = &menu;
    glfwSetWindowUserPointer(window, &appState);
    glfwSetInputMode(window, GLFW_CURSOR, GLFW_CURSOR_NORMAL);

    enum class Screen { MainMenu, InGame };
    Screen screen = Screen::MainMenu;

    std::unique_ptr<World> world;
    std::unique_ptr<Player> player;
    std::unique_ptr<Camera> camera;
    std::string currentWorldPath;
    uint32_t currentSeed = 0;

    const std::array<BlockType, 9> hotbar = {
        BlockType::Dirt, BlockType::Stone, BlockType::Sand, BlockType::Wood,
        BlockType::Planks, BlockType::Wool, BlockType::Glass, BlockType::Leaves, BlockType::Grass,
    };
    int selected = 1; // Stone
    int lastSelected = -1;
    appState.selected = &selected;
    appState.hotbarCount = static_cast<int>(hotbar.size());

    // Holding a mouse button repeats the action every breakInterval /
    // placeInterval seconds; a fresh press always fires immediately
    // (cooldown starts at 0 and is reset to 0 on release).
    const float breakInterval = 0.2f;
    const float placeInterval = 0.25f;
    float breakCooldown = 0.0f;
    float placeCooldown = 0.0f;

    float lastFrame = static_cast<float>(glfwGetTime());
    const float reach = 5.0f;
    bool prevRDown = false;

    auto enterGame = [&](std::unique_ptr<World> newWorld, const glm::vec3& feet, int hotbarIndex, uint32_t seed, const std::string& path) {
        world = std::move(newWorld);
        player = std::make_unique<Player>(feet);
        camera = std::make_unique<Camera>(player->eyePosition());
        appState.camera = camera.get();
        selected = hotbarIndex;
        lastSelected = -1;
        currentSeed = seed;
        currentWorldPath = path;
        breakCooldown = 0.0f;
        placeCooldown = 0.0f;
        prevRDown = false;
        gFirstMouse = true;
        screen = Screen::InGame;
        glfwSetInputMode(window, GLFW_CURSOR, GLFW_CURSOR_DISABLED);

        std::cout << "WASD move, mouse look, Space to jump, Left Shift to sneak, Left Ctrl to sprint\n";
        std::cout << "Left click (hold to repeat): break block, Right click (hold to repeat): place block\n";
        std::cout << "1-9 or mouse wheel: select block, R: respawn, Esc: save and quit to desktop\n";
    };

    std::cout << "PlusCraft\n";

    while (!glfwWindowShouldClose(window)) {
        float currentFrame = static_cast<float>(glfwGetTime());
        float deltaTime = std::min(currentFrame - lastFrame, 0.05f); // clamp so a stall can't blow past a block in one physics step
        lastFrame = currentFrame;

        if (screen == Screen::MainMenu) {
            glClearColor(0.09f, 0.10f, 0.13f, 1.0f);
            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
            glDisable(GL_DEPTH_TEST);
            ui.resize(gWindowWidth, gWindowHeight);

            Menu::Result result = menu.tick(window, ui, gWindowWidth, gWindowHeight, currentFrame);
            if (result.action == Menu::Action::Quit) {
                glfwSetWindowShouldClose(window, true);
            } else if (result.action == Menu::Action::StartNewWorld) {
                std::random_device rd;
                uint32_t seed = rd();
                auto newWorld = std::make_unique<World>(seed);
                glm::vec3 feet = newWorld->spawnPoint();
                std::string path = worldio::pathForName(result.value);
                worldio::save(path, *newWorld, seed, feet, 1);
                enterGame(std::move(newWorld), feet, 1, seed, path);
            } else if (result.action == Menu::Action::LoadWorld) {
                auto loaded = worldio::load(result.value);
                if (loaded) {
                    enterGame(std::move(loaded->world), loaded->playerFeet, loaded->selectedHotbar, loaded->seed, result.value);
                } else {
                    std::cerr << "Failed to load world: " << result.value << "\n";
                    menu.refreshWorldList();
                }
            }

            glfwSwapBuffers(window);
            glfwPollEvents();
            continue;
        }

        // --- In game ---
        if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS) {
            glfwSetWindowShouldClose(window, true);
        }

        bool rDown = glfwGetKey(window, GLFW_KEY_R) == GLFW_PRESS;
        if (rDown && !prevRDown) {
            player->teleport(world->spawnPoint());
        }
        prevRDown = rDown;

        glm::vec3 forwardFlat(camera->front().x, 0.0f, camera->front().z);
        glm::vec3 rightFlat(camera->right().x, 0.0f, camera->right().z);
        if (glm::length(forwardFlat) > 1e-4f) forwardFlat = glm::normalize(forwardFlat);
        if (glm::length(rightFlat) > 1e-4f) rightFlat = glm::normalize(rightFlat);

        glm::vec3 wishDir(0.0f);
        if (glfwGetKey(window, GLFW_KEY_W) == GLFW_PRESS) wishDir += forwardFlat;
        if (glfwGetKey(window, GLFW_KEY_S) == GLFW_PRESS) wishDir -= forwardFlat;
        if (glfwGetKey(window, GLFW_KEY_D) == GLFW_PRESS) wishDir += rightFlat;
        if (glfwGetKey(window, GLFW_KEY_A) == GLFW_PRESS) wishDir -= rightFlat;
        if (glm::length(wishDir) > 1e-4f) wishDir = glm::normalize(wishDir);

        bool jumpPressed = glfwGetKey(window, GLFW_KEY_SPACE) == GLFW_PRESS;
        bool sneaking = glfwGetKey(window, GLFW_KEY_LEFT_SHIFT) == GLFW_PRESS;
        bool sprinting = glfwGetKey(window, GLFW_KEY_LEFT_CONTROL) == GLFW_PRESS;
        player->update(*world, wishDir, jumpPressed, sneaking, sprinting, deltaTime);
        camera->setPosition(player->eyePosition());

        for (int i = 0; i < static_cast<int>(hotbar.size()); ++i) {
            if (glfwGetKey(window, GLFW_KEY_1 + i) == GLFW_PRESS) {
                selected = i;
            }
        }
        if (selected != lastSelected) {
            std::cout << "Selected block: " << blockName(hotbar[selected]) << "\n";
            lastSelected = selected;
        }

        World::RaycastHit hit = world->raycast(camera->position(), camera->front(), reach);

        bool leftDown = glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_LEFT) == GLFW_PRESS;
        bool rightDown = glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_RIGHT) == GLFW_PRESS;

        breakCooldown -= deltaTime;
        placeCooldown -= deltaTime;

        if (leftDown) {
            if (breakCooldown <= 0.0f) {
                if (hit.hit) {
                    world->setBlock(hit.blockPos.x, hit.blockPos.y, hit.blockPos.z, BlockType::Air);
                }
                breakCooldown = breakInterval;
            }
        } else {
            breakCooldown = 0.0f;
        }

        if (rightDown) {
            if (placeCooldown <= 0.0f) {
                if (hit.hit) {
                    // Must match Player's actual collision half-width -
                    // anything smaller lets a block be placed that the
                    // player's real hitbox already overlaps, and physics
                    // only ever stops you moving *into* a new overlap,
                    // it doesn't push you back out of one that appears
                    // under/beside you. That's what let placing a block
                    // "under yourself" leave you stuck in it.
                    glm::vec3 feet = player->feetPosition();
                    bool overlapsPlayer =
                        hit.placePos.x + 1.0f > feet.x - Player::HalfWidth && hit.placePos.x < feet.x + Player::HalfWidth &&
                        hit.placePos.z + 1.0f > feet.z - Player::HalfWidth && hit.placePos.z < feet.z + Player::HalfWidth &&
                        hit.placePos.y + 1.0f > feet.y && hit.placePos.y < feet.y + Player::Height;
                    if (!overlapsPlayer) {
                        world->setBlock(hit.placePos.x, hit.placePos.y, hit.placePos.z, hotbar[selected]);
                    }
                }
                placeCooldown = placeInterval;
            }
        } else {
            placeCooldown = 0.0f;
        }

        glClearColor(0.53f, 0.80f, 0.92f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

        glEnable(GL_DEPTH_TEST);
        blockShader.use();
        glm::mat4 view = camera->getViewMatrix();
        float aspect = gWindowHeight > 0 ? static_cast<float>(gWindowWidth) / static_cast<float>(gWindowHeight) : 1.0f;
        glm::mat4 projection = glm::perspective(glm::radians(70.0f), aspect, 0.05f, 300.0f);
        blockShader.setMat4("uView", view);
        blockShader.setMat4("uProjection", projection);

        world->render(blockShader);

        if (hit.hit) {
            float blink = 0.55f + 0.45f * std::sin(currentFrame * 12.0f);
            highlight.draw(view, projection, hit.blockPos, glm::vec4(blink, blink, blink, 1.0f));
        }

        glDisable(GL_DEPTH_TEST);
        ui.resize(gWindowWidth, gWindowHeight);
        ui.drawCrosshair(glm::vec4(0.9f, 0.9f, 0.9f, 1.0f));

        const float slotSize = 44.0f;
        const float slotGap = 4.0f;
        const int hotbarCount = static_cast<int>(hotbar.size());
        const float hotbarWidth = hotbarCount * slotSize + (hotbarCount - 1) * slotGap;
        const float hotbarX = (gWindowWidth - hotbarWidth) / 2.0f;
        const float hotbarY = gWindowHeight - slotSize - 14.0f;

        for (int i = 0; i < hotbarCount; ++i) {
            float slotX = hotbarX + i * (slotSize + slotGap);
            bool isSelected = (i == selected);
            if (isSelected) {
                ui.drawRect(slotX - 3.0f, hotbarY - 3.0f, slotSize + 6.0f, slotSize + 6.0f, glm::vec4(0.95f, 0.95f, 0.85f, 1.0f));
            }
            ui.drawRect(slotX, hotbarY, slotSize, slotSize,
                        isSelected ? glm::vec4(0.24f, 0.24f, 0.28f, 1.0f) : glm::vec4(0.08f, 0.08f, 0.10f, 1.0f));

            TextureAtlas::UV slotUv = world->atlas().uvFor(hotbar[i], Face::PosX);
            const float iconInset = 4.0f;
            ui.drawIcon(slotX + iconInset, hotbarY + iconInset, slotSize - iconInset * 2.0f, world->atlas().id(),
                        slotUv.u0, slotUv.v0, slotUv.u1, slotUv.v1);
        }

        glfwSwapBuffers(window);
        glfwPollEvents();
    }

    if (screen == Screen::InGame && world && player) {
        worldio::save(currentWorldPath, *world, currentSeed, player->feetPosition(), selected);
    }

    glfwTerminate();
    return 0;
}
