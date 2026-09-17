#include "Ui.h"
#include "Shaders.h"

#include <glm/gtc/matrix_transform.hpp>

Ui::Ui() : shader_(kUiVertexShader, kUiFragmentShader) {
    glGenVertexArrays(1, &vao_);
    glGenBuffers(1, &vbo_);

    glBindVertexArray(vao_);
    glBindBuffer(GL_ARRAY_BUFFER, vbo_);
    glBufferData(GL_ARRAY_BUFFER, static_cast<GLsizeiptr>(sizeof(float) * 4 * 6), nullptr, GL_DYNAMIC_DRAW);

    const GLsizei stride = 4 * sizeof(float);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, stride, reinterpret_cast<void*>(0));
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, stride, reinterpret_cast<void*>(2 * sizeof(float)));
    glEnableVertexAttribArray(1);

    glBindVertexArray(0);
}

Ui::~Ui() {
    glDeleteBuffers(1, &vbo_);
    glDeleteVertexArrays(1, &vao_);
}

void Ui::resize(int windowWidth, int windowHeight) {
    windowWidth_ = windowWidth;
    windowHeight_ = windowHeight;
    projection_ = glm::ortho(0.0f, static_cast<float>(windowWidth), static_cast<float>(windowHeight), 0.0f, -1.0f, 1.0f);
}

void Ui::drawQuad(float x, float y, float w, float h, float u0, float v0, float u1, float v1,
                   const glm::vec4& color, bool useTexture, GLuint textureId) {
    const float verts[6][4] = {
        {x, y, u0, v0},
        {x + w, y, u1, v0},
        {x + w, y + h, u1, v1},
        {x, y, u0, v0},
        {x + w, y + h, u1, v1},
        {x, y + h, u0, v1},
    };

    glBindBuffer(GL_ARRAY_BUFFER, vbo_);
    glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);

    shader_.use();
    shader_.setMat4("uProjection", projection_);
    shader_.setVec4("uColor", color);
    shader_.setInt("uUseTexture", useTexture ? 1 : 0);
    if (useTexture) {
        glActiveTexture(GL_TEXTURE0);
        glBindTexture(GL_TEXTURE_2D, textureId);
        shader_.setInt("uTex", 0);
    }

    glBindVertexArray(vao_);
    glDrawArrays(GL_TRIANGLES, 0, 6);
    glBindVertexArray(0);
}

void Ui::drawCrosshair(const glm::vec4& color) {
    float cx = windowWidth_ / 2.0f;
    float cy = windowHeight_ / 2.0f;
    const float length = 18.0f;
    const float thickness = 2.0f;

    drawQuad(cx - length / 2.0f, cy - thickness / 2.0f, length, thickness, 0, 0, 1, 1, color, false, 0);
    drawQuad(cx - thickness / 2.0f, cy - length / 2.0f, thickness, length, 0, 0, 1, 1, color, false, 0);
}

void Ui::drawIcon(float x, float y, float size, GLuint textureId, float u0, float v0, float u1, float v1) {
    // World-space v=1 is "up"; screen space y grows downward, so the
    // top of the quad (smallest y) must sample v1, not v0.
    drawQuad(x, y, size, size, u0, v1, u1, v0, glm::vec4(1.0f), true, textureId);
}
