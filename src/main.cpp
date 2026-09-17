#include "gl_core33.h"

#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>

#include "Block.h"
#include "Camera.h"
#include "Shader.h"
#include "Shaders.h"
#include "World.h"

#include <glm/gtc/matrix_transform.hpp>
#include <array>
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

    glEnable(GL_DEPTH_TEST);
    glDepthFunc(GL_LESS);
    glClearColor(0.53f, 0.80f, 0.92f, 1.0f);

    Shader blockShader(kBlockVertexShader, kBlockFragmentShader);
    World world(1337u);

    Camera camera(world.spawnPoint());
    glfwSetWindowUserPointer(window, &camera);

    const std::array<BlockType, 6> hotbar = {
        BlockType::Dirt, BlockType::Stone, BlockType::Sand,
        BlockType::Wood, BlockType::Leaves, BlockType::Grass,
    };
    int selected = 1; // Stone
    int lastSelected = -1;

    bool prevLeftDown = false;
    bool prevRightDown = false;
    float lastFrame = static_cast<float>(glfwGetTime());
    const float reach = 6.0f;

    std::cout << "PlusCraft - WASD move, mouse look, Space/Shift fly up/down\n";
    std::cout << "Left click: break block, Right click: place block, 1-6: select block, Esc: quit\n";

    while (!glfwWindowShouldClose(window)) {
        float currentFrame = static_cast<float>(glfwGetTime());
        float deltaTime = currentFrame - lastFrame;
        lastFrame = currentFrame;

        if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS) {
            glfwSetWindowShouldClose(window, true);
        }

        if (glfwGetKey(window, GLFW_KEY_W) == GLFW_PRESS) camera.processKeyboard(CameraMove::Forward, deltaTime);
        if (glfwGetKey(window, GLFW_KEY_S) == GLFW_PRESS) camera.processKeyboard(CameraMove::Backward, deltaTime);
        if (glfwGetKey(window, GLFW_KEY_A) == GLFW_PRESS) camera.processKeyboard(CameraMove::Left, deltaTime);
        if (glfwGetKey(window, GLFW_KEY_D) == GLFW_PRESS) camera.processKeyboard(CameraMove::Right, deltaTime);
        if (glfwGetKey(window, GLFW_KEY_SPACE) == GLFW_PRESS) camera.processKeyboard(CameraMove::Up, deltaTime);
        if (glfwGetKey(window, GLFW_KEY_LEFT_SHIFT) == GLFW_PRESS) camera.processKeyboard(CameraMove::Down, deltaTime);

        for (int i = 0; i < static_cast<int>(hotbar.size()); ++i) {
            if (glfwGetKey(window, GLFW_KEY_1 + i) == GLFW_PRESS) {
                selected = i;
            }
        }
        if (selected != lastSelected) {
            std::cout << "Selected block: " << blockName(hotbar[selected]) << "\n";
            lastSelected = selected;
        }

        bool leftDown = glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_LEFT) == GLFW_PRESS;
        bool rightDown = glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_RIGHT) == GLFW_PRESS;

        if (leftDown && !prevLeftDown) {
            World::RaycastHit hit = world.raycast(camera.position(), camera.front(), reach);
            if (hit.hit) {
                world.setBlock(hit.blockPos.x, hit.blockPos.y, hit.blockPos.z, BlockType::Air);
            }
        }
        if (rightDown && !prevRightDown) {
            World::RaycastHit hit = world.raycast(camera.position(), camera.front(), reach);
            if (hit.hit) {
                world.setBlock(hit.placePos.x, hit.placePos.y, hit.placePos.z, hotbar[selected]);
            }
        }
        prevLeftDown = leftDown;
        prevRightDown = rightDown;

        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

        blockShader.use();
        glm::mat4 view = camera.getViewMatrix();
        float aspect = gWindowHeight > 0 ? static_cast<float>(gWindowWidth) / static_cast<float>(gWindowHeight) : 1.0f;
        glm::mat4 projection = glm::perspective(glm::radians(70.0f), aspect, 0.05f, 300.0f);
        blockShader.setMat4("uView", view);
        blockShader.setMat4("uProjection", projection);

        world.render(blockShader);

        glfwSwapBuffers(window);
        glfwPollEvents();
    }

    glfwTerminate();
    return 0;
}
