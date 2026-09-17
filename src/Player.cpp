#include "Player.h"

#include <cmath>

Player::Player(glm::vec3 spawnFeetPosition) : position_(spawnFeetPosition) {}

glm::vec3 Player::eyePosition() const {
    return position_ + glm::vec3(0.0f, EyeHeight, 0.0f);
}

bool Player::boxIntersectsSolid(const World& world, const glm::vec3& feet) const {
    const float eps = 1e-4f;
    glm::vec3 minP = feet - glm::vec3(HalfWidth, 0.0f, HalfWidth);
    glm::vec3 maxP = feet + glm::vec3(HalfWidth, Height, HalfWidth);

    int minX = static_cast<int>(std::floor(minP.x + eps));
    int maxX = static_cast<int>(std::floor(maxP.x - eps));
    int minY = static_cast<int>(std::floor(minP.y + eps));
    int maxY = static_cast<int>(std::floor(maxP.y - eps));
    int minZ = static_cast<int>(std::floor(minP.z + eps));
    int maxZ = static_cast<int>(std::floor(maxP.z - eps));

    for (int y = minY; y <= maxY; ++y) {
        for (int z = minZ; z <= maxZ; ++z) {
            for (int x = minX; x <= maxX; ++x) {
                if (!isAir(world.getBlock(x, y, z))) {
                    return true;
                }
            }
        }
    }
    return false;
}

void Player::moveAxis(const World& world, int axis, float delta) {
    if (delta == 0.0f) {
        return;
    }

    glm::vec3 next = position_;
    next[axis] += delta;

    if (boxIntersectsSolid(world, next)) {
        if (axis == 1 && delta < 0.0f) {
            onGround_ = true;
        }
        velocity_[axis] = 0.0f;
        return;
    }

    position_ = next;
}

void Player::update(const World& world, const glm::vec3& wishDir, bool jumpPressed, float dt) {
    velocity_.x = wishDir.x * MoveSpeed;
    velocity_.z = wishDir.z * MoveSpeed;

    velocity_.y -= Gravity * dt;
    if (velocity_.y < -TerminalVelocity) {
        velocity_.y = -TerminalVelocity;
    }

    if (onGround_ && jumpPressed) {
        velocity_.y = JumpSpeed;
    }

    onGround_ = false;

    moveAxis(world, 0, velocity_.x * dt);
    moveAxis(world, 2, velocity_.z * dt);
    moveAxis(world, 1, velocity_.y * dt);
}
