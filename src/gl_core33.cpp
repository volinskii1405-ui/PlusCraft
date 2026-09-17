#include "gl_core33.h"

#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>
#include <iostream>

PFNGLGENBUFFERSPROC glGenBuffers = nullptr;
PFNGLBINDBUFFERPROC glBindBuffer = nullptr;
PFNGLBUFFERDATAPROC glBufferData = nullptr;
PFNGLBUFFERSUBDATAPROC glBufferSubData = nullptr;
PFNGLDELETEBUFFERSPROC glDeleteBuffers = nullptr;

PFNGLGENVERTEXARRAYSPROC glGenVertexArrays = nullptr;
PFNGLBINDVERTEXARRAYPROC glBindVertexArray = nullptr;
PFNGLDELETEVERTEXARRAYSPROC glDeleteVertexArrays = nullptr;

PFNGLENABLEVERTEXATTRIBARRAYPROC glEnableVertexAttribArray = nullptr;
PFNGLDISABLEVERTEXATTRIBARRAYPROC glDisableVertexAttribArray = nullptr;
PFNGLVERTEXATTRIBPOINTERPROC glVertexAttribPointer = nullptr;

PFNGLCREATESHADERPROC glCreateShader = nullptr;
PFNGLSHADERSOURCEPROC glShaderSource = nullptr;
PFNGLCOMPILESHADERPROC glCompileShader = nullptr;
PFNGLGETSHADERIVPROC glGetShaderiv = nullptr;
PFNGLGETSHADERINFOLOGPROC glGetShaderInfoLog = nullptr;
PFNGLDELETESHADERPROC glDeleteShader = nullptr;

PFNGLCREATEPROGRAMPROC glCreateProgram = nullptr;
PFNGLATTACHSHADERPROC glAttachShader = nullptr;
PFNGLLINKPROGRAMPROC glLinkProgram = nullptr;
PFNGLGETPROGRAMIVPROC glGetProgramiv = nullptr;
PFNGLGETPROGRAMINFOLOGPROC glGetProgramInfoLog = nullptr;
PFNGLUSEPROGRAMPROC glUseProgram = nullptr;
PFNGLDELETEPROGRAMPROC glDeleteProgram = nullptr;

PFNGLGETUNIFORMLOCATIONPROC glGetUniformLocation = nullptr;
PFNGLUNIFORMMATRIX4FVPROC glUniformMatrix4fv = nullptr;
PFNGLUNIFORM1IPROC glUniform1i = nullptr;
PFNGLUNIFORM1FPROC glUniform1f = nullptr;
PFNGLUNIFORM3FVPROC glUniform3fv = nullptr;
PFNGLUNIFORM4FVPROC glUniform4fv = nullptr;

#ifndef GL_VERSION_1_3
PFNGLACTIVETEXTUREPROC glActiveTexture = nullptr;
#endif
PFNGLGENERATEMIPMAPPROC glGenerateMipmap = nullptr;

namespace {

template <typename T>
bool load(T& fnPtr, const char* name) {
    fnPtr = reinterpret_cast<T>(glfwGetProcAddress(name));
    if (!fnPtr) {
        std::cerr << "gl_core33: failed to load " << name << "\n";
        return false;
    }
    return true;
}

} // namespace

bool glCore33Init() {
    bool ok = true;

    ok &= load(glGenBuffers, "glGenBuffers");
    ok &= load(glBindBuffer, "glBindBuffer");
    ok &= load(glBufferData, "glBufferData");
    ok &= load(glBufferSubData, "glBufferSubData");
    ok &= load(glDeleteBuffers, "glDeleteBuffers");

    ok &= load(glGenVertexArrays, "glGenVertexArrays");
    ok &= load(glBindVertexArray, "glBindVertexArray");
    ok &= load(glDeleteVertexArrays, "glDeleteVertexArrays");

    ok &= load(glEnableVertexAttribArray, "glEnableVertexAttribArray");
    ok &= load(glDisableVertexAttribArray, "glDisableVertexAttribArray");
    ok &= load(glVertexAttribPointer, "glVertexAttribPointer");

    ok &= load(glCreateShader, "glCreateShader");
    ok &= load(glShaderSource, "glShaderSource");
    ok &= load(glCompileShader, "glCompileShader");
    ok &= load(glGetShaderiv, "glGetShaderiv");
    ok &= load(glGetShaderInfoLog, "glGetShaderInfoLog");
    ok &= load(glDeleteShader, "glDeleteShader");

    ok &= load(glCreateProgram, "glCreateProgram");
    ok &= load(glAttachShader, "glAttachShader");
    ok &= load(glLinkProgram, "glLinkProgram");
    ok &= load(glGetProgramiv, "glGetProgramiv");
    ok &= load(glGetProgramInfoLog, "glGetProgramInfoLog");
    ok &= load(glUseProgram, "glUseProgram");
    ok &= load(glDeleteProgram, "glDeleteProgram");

    ok &= load(glGetUniformLocation, "glGetUniformLocation");
    ok &= load(glUniformMatrix4fv, "glUniformMatrix4fv");
    ok &= load(glUniform1i, "glUniform1i");
    ok &= load(glUniform1f, "glUniform1f");
    ok &= load(glUniform3fv, "glUniform3fv");
    ok &= load(glUniform4fv, "glUniform4fv");

#ifndef GL_VERSION_1_3
    ok &= load(glActiveTexture, "glActiveTexture");
#endif
    ok &= load(glGenerateMipmap, "glGenerateMipmap");

    return ok;
}
