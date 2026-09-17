#include "Camera.h"

#include <glm/gtc/matrix_transform.hpp>
#include <algorithm>
#include <cmath>

Camera::Camera(glm::vec3 position) : position_(position) {
    updateVectors();
}

glm::mat4 Camera::getViewMatrix() const {
    return glm::lookAt(position_, position_ + front_, up_);
}

void Camera::processKeyboard(CameraMove direction, float deltaTime) {
    float velocity = moveSpeed * deltaTime;
    switch (direction) {
        case CameraMove::Forward:
            position_ += front_ * velocity;
            break;
        case CameraMove::Backward:
            position_ -= front_ * velocity;
            break;
        case CameraMove::Left:
            position_ -= right_ * velocity;
            break;
        case CameraMove::Right:
            position_ += right_ * velocity;
            break;
        case CameraMove::Up:
            position_ += worldUp_ * velocity;
            break;
        case CameraMove::Down:
            position_ -= worldUp_ * velocity;
            break;
    }
}

void Camera::processMouseMovement(float xOffset, float yOffset) {
    yaw_ += xOffset * mouseSensitivity;
    pitch_ += yOffset * mouseSensitivity;
    pitch_ = std::clamp(pitch_, -89.0f, 89.0f);
    updateVectors();
}

void Camera::updateVectors() {
    float yawRad = glm::radians(yaw_);
    float pitchRad = glm::radians(pitch_);

    glm::vec3 newFront;
    newFront.x = std::cos(yawRad) * std::cos(pitchRad);
    newFront.y = std::sin(pitchRad);
    newFront.z = std::sin(yawRad) * std::cos(pitchRad);

    front_ = glm::normalize(newFront);
    right_ = glm::normalize(glm::cross(front_, worldUp_));
    up_ = glm::normalize(glm::cross(right_, front_));
}
