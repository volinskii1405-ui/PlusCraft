#pragma once

#include "World.h"
#include <glm/glm.hpp>

// Axis-separated AABB physics: gravity, jumping, and collision against
// solid blocks (leaves included - they're visually see-through but
// still solid, same as vanilla Minecraft). No sprinting or swimming.
class Player {
public:
    explicit Player(glm::vec3 spawnFeetPosition);

    // wishDir is a normalized (or zero) horizontal direction in world
    // space; jumpPressed only triggers a jump while standing on ground.
    void update(const World& world, const glm::vec3& wishDir, bool jumpPressed, float dt);

    glm::vec3 eyePosition() const;
    const glm::vec3& feetPosition() const { return position_; }

    static constexpr float HalfWidth = 0.3f;
    static constexpr float Height = 1.8f;
    static constexpr float EyeHeight = 1.62f;

private:
    void moveAxis(const World& world, int axis, float delta);
    bool boxIntersectsSolid(const World& world, const glm::vec3& feet) const;

    glm::vec3 position_; // feet, base center
    glm::vec3 velocity_{0.0f};
    bool onGround_ = false;

    static constexpr float MoveSpeed = 5.0f;
    static constexpr float JumpSpeed = 8.0f;
    static constexpr float Gravity = 22.0f;
    static constexpr float TerminalVelocity = 30.0f;
};
