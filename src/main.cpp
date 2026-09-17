#include "gl_core33.h"

#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>

#include "Block.h"
#include "Camera.h"
#include "Player.h"
#include "Shader.h"
#include "Shaders.h"
#include "Ui.h"
#include "World.h"

#include <glm/gtc/matrix_transform.hpp>
#include <algorithm>
#include <array>
#include <cmath>
#include <iostream>

namespace {

int gWindowWidth = 1280;
int gWindowHeight = 720;
bool gFirstMouse = true;
float gLastX = gWindowWidth / 2.0f;
float gLastY = gWindowHeight / 2.0f;

void framebufferSizeCallback(GLFWwindow*, int width, int height) {
    gWindowWidth = width;
    gWindowHeight = height;
    glViewport(0, 0, width, height);
}

void mouseCallback(GLFWwindow* window, double xpos, double ypos) {
    auto* camera = static_cast<Camera*>(glfwGetWindowUserPointer(window));
    if (!camera) {
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

    camera->processMouseMovement(xOffset, yOffset);
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
    glfwSetInputMode(window, GLFW_CURSOR, GLFW_CURSOR_DISABLED);
    glfwSetCursorPosCallback(window, mouseCallback);

    if (!glCore33Init()) {
        std::cerr << "Failed to load required OpenGL functions (need an OpenGL 3.3 capable driver)\n";
        glfwTerminate();
        return 1;
    }

    glDepthFunc(GL_LESS);
    glClearColor(0.53f, 0.80f, 0.92f, 1.0f);

    Shader blockShader(kBlockVertexShader, kBlockFragmentShader);
    World world(1337u);

    Player player(world.spawnPoint());
    Camera camera(player.eyePosition());
    glfwSetWindowUserPointer(window, &camera);

    Ui ui;

    const std::array<BlockType, 6> hotbar = {
        BlockType::Dirt, BlockType::Stone, BlockType::Sand,
        BlockType::Wood, BlockType::Leaves, BlockType::Grass,
    };
    int selected = 1; // Stone
    int lastSelected = -1;

    // Holding a mouse button repeats the action every breakInterval /
    // placeInterval seconds; a fresh press always fires immediately
    // (cooldown starts at 0 and is reset to 0 on release).
    const float breakInterval = 0.2f;
    const float placeInterval = 0.25f;
    float breakCooldown = 0.0f;
    float placeCooldown = 0.0f;

    float lastFrame = static_cast<float>(glfwGetTime());
    const float reach = 6.0f;

    std::cout << "PlusCraft - WASD move, mouse look, Space to jump\n";
    std::cout << "Left click (hold to repeat): break block, Right click (hold to repeat): place block\n";
    std::cout << "1-6: select block, Esc: quit\n";

    while (!glfwWindowShouldClose(window)) {
        float currentFrame = static_cast<float>(glfwGetTime());
        float deltaTime = std::min(currentFrame - lastFrame, 0.05f); // clamp so a stall can't blow past a block in one physics step
        lastFrame = currentFrame;

        if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS) {
            glfwSetWindowShouldClose(window, true);
        }

        glm::vec3 forwardFlat(camera.front().x, 0.0f, camera.front().z);
        glm::vec3 rightFlat(camera.right().x, 0.0f, camera.right().z);
        if (glm::length(forwardFlat) > 1e-4f) forwardFlat = glm::normalize(forwardFlat);
        if (glm::length(rightFlat) > 1e-4f) rightFlat = glm::normalize(rightFlat);

        glm::vec3 wishDir(0.0f);
        if (glfwGetKey(window, GLFW_KEY_W) == GLFW_PRESS) wishDir += forwardFlat;
        if (glfwGetKey(window, GLFW_KEY_S) == GLFW_PRESS) wishDir -= forwardFlat;
        if (glfwGetKey(window, GLFW_KEY_D) == GLFW_PRESS) wishDir += rightFlat;
        if (glfwGetKey(window, GLFW_KEY_A) == GLFW_PRESS) wishDir -= rightFlat;
        if (glm::length(wishDir) > 1e-4f) wishDir = glm::normalize(wishDir);

        bool jumpPressed = glfwGetKey(window, GLFW_KEY_SPACE) == GLFW_PRESS;
        player.update(world, wishDir, jumpPressed, deltaTime);
        camera.setPosition(player.eyePosition());

        for (int i = 0; i < static_cast<int>(hotbar.size()); ++i) {
            if (glfwGetKey(window, GLFW_KEY_1 + i) == GLFW_PRESS) {
                selected = i;
            }
        }
        if (selected != lastSelected) {
            std::cout << "Selected block: " << blockName(hotbar[selected]) << "\n";
            lastSelected = selected;
        }

        World::RaycastHit hit = world.raycast(camera.position(), camera.front(), reach);

        bool leftDown = glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_LEFT) == GLFW_PRESS;
        bool rightDown = glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_RIGHT) == GLFW_PRESS;

        breakCooldown -= deltaTime;
        placeCooldown -= deltaTime;

        if (leftDown) {
            if (breakCooldown <= 0.0f) {
                if (hit.hit) {
                    world.setBlock(hit.blockPos.x, hit.blockPos.y, hit.blockPos.z, BlockType::Air);
                }
                breakCooldown = breakInterval;
            }
        } else {
            breakCooldown = 0.0f;
        }

        if (rightDown) {
            if (placeCooldown <= 0.0f) {
                if (hit.hit) {
                    glm::vec3 feet = player.feetPosition();
                    bool overlapsPlayer =
                        hit.placePos.x + 1.0f > feet.x - Player::HalfWidth && hit.placePos.x < feet.x + Player::HalfWidth &&
                        hit.placePos.z + 1.0f > feet.z - Player::HalfWidth && hit.placePos.z < feet.z + Player::HalfWidth &&
                        hit.placePos.y + 1.0f > feet.y && hit.placePos.y < feet.y + Player::Height;
                    if (!overlapsPlayer) {
                        world.setBlock(hit.placePos.x, hit.placePos.y, hit.placePos.z, hotbar[selected]);
                    }
                }
                placeCooldown = placeInterval;
            }
        } else {
            placeCooldown = 0.0f;
        }

        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

        glEnable(GL_DEPTH_TEST);
        blockShader.use();
        glm::mat4 view = camera.getViewMatrix();
        float aspect = gWindowHeight > 0 ? static_cast<float>(gWindowWidth) / static_cast<float>(gWindowHeight) : 1.0f;
        glm::mat4 projection = glm::perspective(glm::radians(70.0f), aspect, 0.05f, 300.0f);
        blockShader.setMat4("uView", view);
        blockShader.setMat4("uProjection", projection);

        world.render(blockShader);

        glDisable(GL_DEPTH_TEST);
        ui.resize(gWindowWidth, gWindowHeight);

        glm::vec4 crosshairColor;
        if (hit.hit) {
            float blink = 0.55f + 0.45f * std::sin(currentFrame * 12.0f);
            crosshairColor = glm::vec4(blink, blink, blink, 1.0f);
        } else {
            crosshairColor = glm::vec4(0.8f, 0.8f, 0.8f, 1.0f);
        }
        ui.drawCrosshair(crosshairColor);

        TextureAtlas::UV iconUv = world.atlas().uvFor(hotbar[selected], Face::PosX);
        const float iconSize = 48.0f;
        const float iconMargin = 16.0f;
        ui.drawIcon(iconMargin, gWindowHeight - iconMargin - iconSize, iconSize, world.atlas().id(),
                    iconUv.u0, iconUv.v0, iconUv.u1, iconUv.v1);

        glfwSwapBuffers(window);
        glfwPollEvents();
    }

    glfwTerminate();
    return 0;
}
