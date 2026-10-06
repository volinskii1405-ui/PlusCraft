#pragma once

#include <glm/glm.hpp>

// Look direction and view matrix only. Movement and collision are
// Player's job; main.cpp copies Player::eyePosition() into this each
// frame via setPosition().
class Camera {
public:
    explicit Camera(glm::vec3 position);

    glm::mat4 getViewMatrix() const;

    void processMouseMovement(float xOffset, float yOffset);
    void setPosition(const glm::vec3& position) { position_ = position; }

    const glm::vec3& position() const { return position_; }
    const glm::vec3& front() const { return front_; }
    const glm::vec3& right() const { return right_; }

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
