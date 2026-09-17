#pragma once

#include "Shader.h"
#include "gl_core33.h"

#include <glm/glm.hpp>

// Minimal 2D overlay drawn in screen-pixel space on top of the 3D
// scene: a crosshair and a single "selected block" icon. Not a general
// UI toolkit - just the two things this game needs right now.
class Ui {
public:
    Ui();
    ~Ui();

    Ui(const Ui&) = delete;
    Ui& operator=(const Ui&) = delete;

    // Call once per frame (cheap) before drawing, so the overlay tracks
    // the current window size.
    void resize(int windowWidth, int windowHeight);

    // A plus-sign crosshair at screen center. Vary color's brightness
    // frame to frame (e.g. via sin(time)) for a blink effect.
    void drawCrosshair(const glm::vec4& color);

    // A textured square at pixel (x, y) (top-left corner), sampling
    // [u0,v0]-[u1,v1] of textureId. UVs are taken as given in world
    // space (v=1 is "up"), and flipped internally so the icon reads
    // right-side-up in screen space (y grows downward).
    void drawIcon(float x, float y, float size, GLuint textureId, float u0, float v0, float u1, float v1);

private:
    void drawQuad(float x, float y, float w, float h, float u0, float v0, float u1, float v1,
                  const glm::vec4& color, bool useTexture, GLuint textureId);

    Shader shader_;
    GLuint vao_ = 0;
    GLuint vbo_ = 0;
    glm::mat4 projection_{1.0f};
    int windowWidth_ = 0;
    int windowHeight_ = 0;
};
