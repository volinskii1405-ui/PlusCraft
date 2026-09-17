#pragma once

#include "gl_core33.h"
#include <glm/glm.hpp>
#include <string>

class Shader {
public:
    Shader(const char* vertexSrc, const char* fragmentSrc);
    ~Shader();

    Shader(const Shader&) = delete;
    Shader& operator=(const Shader&) = delete;

    void use() const;

    void setMat4(const std::string& name, const glm::mat4& value) const;
    void setInt(const std::string& name, int value) const;
    void setFloat(const std::string& name, float value) const;
    void setVec3(const std::string& name, const glm::vec3& value) const;

private:
    GLuint id_ = 0;

    static GLuint compile(GLenum type, const char* src);
};
