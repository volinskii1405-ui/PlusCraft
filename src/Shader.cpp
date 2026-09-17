#include "Shader.h"

#include <glm/gtc/type_ptr.hpp>
#include <iostream>
#include <vector>

GLuint Shader::compile(GLenum type, const char* src) {
    GLuint shader = glCreateShader(type);
    glShaderSource(shader, 1, &src, nullptr);
    glCompileShader(shader);

    GLint success = 0;
    glGetShaderiv(shader, GL_COMPILE_STATUS, &success);
    if (!success) {
        GLint logLength = 0;
        glGetShaderiv(shader, GL_INFO_LOG_LENGTH, &logLength);
        std::vector<char> log(logLength > 0 ? logLength : 1);
        glGetShaderInfoLog(shader, static_cast<GLsizei>(log.size()), nullptr, log.data());
        std::cerr << "Shader compile error: " << log.data() << "\n";
    }
    return shader;
}

Shader::Shader(const char* vertexSrc, const char* fragmentSrc) {
    GLuint vertex = compile(GL_VERTEX_SHADER, vertexSrc);
    GLuint fragment = compile(GL_FRAGMENT_SHADER, fragmentSrc);

    id_ = glCreateProgram();
    glAttachShader(id_, vertex);
    glAttachShader(id_, fragment);
    glLinkProgram(id_);

    GLint success = 0;
    glGetProgramiv(id_, GL_LINK_STATUS, &success);
    if (!success) {
        GLint logLength = 0;
        glGetProgramiv(id_, GL_INFO_LOG_LENGTH, &logLength);
        std::vector<char> log(logLength > 0 ? logLength : 1);
        glGetProgramInfoLog(id_, static_cast<GLsizei>(log.size()), nullptr, log.data());
        std::cerr << "Shader link error: " << log.data() << "\n";
    }

    glDeleteShader(vertex);
    glDeleteShader(fragment);
}

Shader::~Shader() {
    if (id_ != 0) {
        glDeleteProgram(id_);
    }
}

void Shader::use() const {
    glUseProgram(id_);
}

void Shader::setMat4(const std::string& name, const glm::mat4& value) const {
    glUniformMatrix4fv(glGetUniformLocation(id_, name.c_str()), 1, GL_FALSE, glm::value_ptr(value));
}

void Shader::setInt(const std::string& name, int value) const {
    glUniform1i(glGetUniformLocation(id_, name.c_str()), value);
}

void Shader::setFloat(const std::string& name, float value) const {
    glUniform1f(glGetUniformLocation(id_, name.c_str()), value);
}

void Shader::setVec3(const std::string& name, const glm::vec3& value) const {
    glUniform3fv(glGetUniformLocation(id_, name.c_str()), 1, glm::value_ptr(value));
}
