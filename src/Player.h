#pragma once

#include "World.h"
#include <glm/glm.hpp>

// Axis-separated AABB physics: gravity, jumping, and collision against
// solid blocks (leaves included - they're visually see-through but
// still solid, same as vanilla Minecraft). No swimming.
class Player {
public:
    explicit Player(glm::vec3 spawnFeetPosition);

    // wishDir is a normalized (or zero) horizontal direction in world
    // space; jumpPressed only triggers a jump while standing on ground.
    // sneaking lowers the eye height and, while already on ground,
    // refuses horizontal movement that would walk the player off an
    // edge with nothing underneath; it also overrides sprinting (can't
    // sprint-sneak, same as vanilla).
    void update(const World& world, const glm::vec3& wishDir, bool jumpPressed, bool sneaking, bool sprinting, float dt);

    glm::vec3 eyePosition() const;
    const glm::vec3& feetPosition() const { return position_; }
    bool onGround() const { return onGround_; }
    // True only on the update() call where a jump was actually
    // triggered (not just while airborne afterward) - for one-shot
    // jump sound/effects.
    bool justJumped() const { return justJumped_; }

    // Resets position and lets the player fall/stand fresh from there
    // (used by the "R" respawn key).
    void teleport(const glm::vec3& feetPosition);

    static constexpr float HalfWidth = 0.3f;
    static constexpr float Height = 1.8f;
    static constexpr float EyeHeight = 1.62f;
    static constexpr float CrouchEyeHeight = 1.35f;

private:
    void moveAxis(const World& world, int axis, float delta, bool preventFallOff);
    bool boxIntersectsSolid(const World& world, const glm::vec3& feet) const;
    bool hasSupportAt(const World& world, const glm::vec3& feet) const;

    glm::vec3 position_; // feet, base center
    glm::vec3 velocity_{0.0f};
    bool onGround_ = false;
    bool sneaking_ = false;
    bool justJumped_ = false;

    static constexpr float MoveSpeed = 4.0f;
    static constexpr float SneakSpeedFactor = 0.4f;
    static constexpr float SprintSpeedFactor = 1.5f;
    static constexpr float JumpSpeed = 8.0f;
    static constexpr float Gravity = 22.0f;
    static constexpr float TerminalVelocity = 30.0f;
};
