#pragma once

#include <glm/glm.hpp>

enum class CameraMove {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
};

// Free-fly first-person camera (creative mode: no gravity, no collision).
class Camera {
public:
    explicit Camera(glm::vec3 position);

    glm::mat4 getViewMatrix() const;

    void processKeyboard(CameraMove direction, float deltaTime);
    void processMouseMovement(float xOffset, float yOffset);

    const glm::vec3& position() const { return position_; }
    const glm::vec3& front() const { return front_; }

    float moveSpeed = 8.0f;
    float mouseSensitivity = 0.1f;

private:
    void updateVectors();

    glm::vec3 position_;
    glm::vec3 front_{0.0f, 0.0f, -1.0f};
    glm::vec3 up_{0.0f, 1.0f, 0.0f};
    glm::vec3 right_{1.0f, 0.0f, 0.0f};
    glm::vec3 worldUp_{0.0f, 1.0f, 0.0f};

    float yaw_ = -90.0f;
    float pitch_ = 0.0f;
};
