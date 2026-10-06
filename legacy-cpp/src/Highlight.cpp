#include "Highlight.h"
#include "Shaders.h"

namespace {

// Slightly larger than a unit cube so the outline doesn't z-fight with
// the block's own faces underneath it.
constexpr float kEps = 0.0015f;
constexpr float kLo = -kEps;
constexpr float kHi = 1.0f + kEps;

const int kEdges[12][2] = {
    {0, 1}, {1, 2}, {2, 3}, {3, 0}, // bottom face
    {4, 5}, {5, 6}, {6, 7}, {7, 4}, // top face
    {0, 4}, {1, 5}, {2, 6}, {3, 7}, // verticals
};

} // namespace

Highlight::Highlight() : shader_(kHighlightVertexShader, kHighlightFragmentShader) {
    glGenVertexArrays(1, &vao_);
    glGenBuffers(1, &vbo_);

    glBindVertexArray(vao_);
    glBindBuffer(GL_ARRAY_BUFFER, vbo_);
    glBufferData(GL_ARRAY_BUFFER, static_cast<GLsizeiptr>(sizeof(float) * 3 * 24), nullptr, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 3 * sizeof(float), reinterpret_cast<void*>(0));
    glEnableVertexAttribArray(0);
    glBindVertexArray(0);
}

Highlight::~Highlight() {
    glDeleteBuffers(1, &vbo_);
    glDeleteVertexArrays(1, &vao_);
}

void Highlight::draw(const glm::mat4& view, const glm::mat4& projection, const glm::ivec3& blockPos, const glm::vec4& color) {
    glm::vec3 origin(blockPos.x, blockPos.y, blockPos.z);
    glm::vec3 corners[8] = {
        origin + glm::vec3(kLo, kLo, kLo), origin + glm::vec3(kHi, kLo, kLo),
        origin + glm::vec3(kHi, kHi, kLo), origin + glm::vec3(kLo, kHi, kLo),
        origin + glm::vec3(kLo, kLo, kHi), origin + glm::vec3(kHi, kLo, kHi),
        origin + glm::vec3(kHi, kHi, kHi), origin + glm::vec3(kLo, kHi, kHi),
    };

    float verts[24][3];
    int v = 0;
    for (const auto& edge : kEdges) {
        const glm::vec3& a = corners[edge[0]];
        const glm::vec3& b = corners[edge[1]];
        verts[v][0] = a.x;
        verts[v][1] = a.y;
        verts[v][2] = a.z;
        ++v;
        verts[v][0] = b.x;
        verts[v][1] = b.y;
        verts[v][2] = b.z;
        ++v;
    }

    glBindBuffer(GL_ARRAY_BUFFER, vbo_);
    glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);

    shader_.use();
    shader_.setMat4("uView", view);
    shader_.setMat4("uProjection", projection);
    shader_.setVec4("uColor", color);

    glLineWidth(2.0f);
    glBindVertexArray(vao_);
    glDrawArrays(GL_LINES, 0, 24);
    glBindVertexArray(0);
}
