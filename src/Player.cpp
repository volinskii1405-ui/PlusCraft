#include "Player.h"

#include <cmath>

Player::Player(glm::vec3 spawnFeetPosition) : position_(spawnFeetPosition) {}

glm::vec3 Player::eyePosition() const {
    float eyeHeight = sneaking_ ? CrouchEyeHeight : EyeHeight;
    return position_ + glm::vec3(0.0f, eyeHeight, 0.0f);
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

bool Player::hasSupportAt(const World& world, const glm::vec3& feet) const {
    // Checked a bit below the feet rather than right at them, since
    // moveAxis's binary search leaves a tiny (sub-centimeter) gap
    // above the true surface instead of landing exactly flush.
    const float probeDepth = 0.05f;
    const float eps = 1e-4f;

    int minX = static_cast<int>(std::floor(feet.x - HalfWidth + eps));
    int maxX = static_cast<int>(std::floor(feet.x + HalfWidth - eps));
    int minZ = static_cast<int>(std::floor(feet.z - HalfWidth + eps));
    int maxZ = static_cast<int>(std::floor(feet.z + HalfWidth - eps));
    int y = static_cast<int>(std::floor(feet.y - probeDepth));

    for (int z = minZ; z <= maxZ; ++z) {
        for (int x = minX; x <= maxX; ++x) {
            if (!isAir(world.getBlock(x, y, z))) {
                return true;
            }
        }
    }
    return false;
}

void Player::moveAxis(const World& world, int axis, float delta, bool preventFallOff) {
    if (delta == 0.0f) {
        return;
    }

    auto blockedAt = [&](const glm::vec3& pos) {
        return boxIntersectsSolid(world, pos) || (preventFallOff && !hasSupportAt(world, pos));
    };

    glm::vec3 start = position_;
    glm::vec3 next = start;
    next[axis] += delta;

    if (!blockedAt(next)) {
        position_ = next;
        return;
    }

    // Binary-search how far along delta we can actually go, so the
    // player lands flush against a surface (or an edge, while
    // sneaking) instead of stopping short with a visible gap.
    float freeFraction = 0.0f;
    float blockedFraction = 1.0f;
    for (int i = 0; i < 10; ++i) {
        float mid = (freeFraction + blockedFraction) * 0.5f;
        glm::vec3 probe = start;
        probe[axis] += delta * mid;
        if (blockedAt(probe)) {
            blockedFraction = mid;
        } else {
            freeFraction = mid;
        }
    }

    position_ = start;
    position_[axis] += delta * freeFraction;

    if (axis == 1 && delta < 0.0f) {
        onGround_ = true;
    }
    velocity_[axis] = 0.0f;
}

void Player::update(const World& world, const glm::vec3& wishDir, bool jumpPressed, bool sneaking, float dt) {
    sneaking_ = sneaking;

    velocity_.x = wishDir.x * MoveSpeed;
    velocity_.z = wishDir.z * MoveSpeed;

    velocity_.y -= Gravity * dt;
    if (velocity_.y < -TerminalVelocity) {
        velocity_.y = -TerminalVelocity;
    }

    if (onGround_ && jumpPressed) {
        velocity_.y = JumpSpeed;
    }

    bool preventFallOff = sneaking && onGround_;

    onGround_ = false;

    moveAxis(world, 0, velocity_.x * dt, preventFallOff);
    moveAxis(world, 2, velocity_.z * dt, preventFallOff);
    moveAxis(world, 1, velocity_.y * dt, false);
}
