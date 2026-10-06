#pragma once

#include "Shader.h"
#include "gl_core33.h"

#include <glm/glm.hpp>

// Wireframe cube outline drawn around the block the player is
// currently looking at. Uses its own tiny unlit line shader since the
// block shader expects a textured, lit surface.
class Highlight {
public:
    Highlight();
    ~Highlight();

    Highlight(const Highlight&) = delete;
    Highlight& operator=(const Highlight&) = delete;

    void draw(const glm::mat4& view, const glm::mat4& projection, const glm::ivec3& blockPos, const glm::vec4& color);

private:
    Shader shader_;
    GLuint vao_ = 0;
    GLuint vbo_ = 0;
};
