#pragma once

// Minimal hand-written OpenGL loader.
//
// GL/gl.h (system headers) only exposes OpenGL 1.1. Everything from
// OpenGL 1.5 (buffer objects) through 3.3 core (shaders, VAOs) that this
// project needs is declared here as a function pointer and resolved at
// runtime via glfwGetProcAddress in glCore33Init(), the same approach
// libraries like GLAD generate for you.

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif

#if defined(__APPLE__)
#include <OpenGL/gl.h>
#else
#include <GL/gl.h>
#endif

#include <cstddef>

#ifndef APIENTRY
#define APIENTRY
#endif

// ---- Types missing from the GL 1.1 headers ----
typedef char GLchar;
typedef ptrdiff_t GLsizeiptr;
typedef ptrdiff_t GLintptr;

// ---- Constants missing from the GL 1.1 headers ----
#define GL_ARRAY_BUFFER 0x8892
#define GL_ELEMENT_ARRAY_BUFFER 0x8893
#define GL_STATIC_DRAW 0x88E4
#define GL_DYNAMIC_DRAW 0x88E8
#define GL_FRAGMENT_SHADER 0x8B30
#define GL_VERTEX_SHADER 0x8B31
#define GL_COMPILE_STATUS 0x8B81
#define GL_LINK_STATUS 0x8B82
#define GL_INFO_LOG_LENGTH 0x8B84
#define GL_TEXTURE0 0x84C0
#define GL_CLAMP_TO_EDGE 0x812F
#define GL_MULTISAMPLE 0x809D

// ---- Function pointer typedefs (GL 1.5 - 3.3 core subset we use) ----
typedef void(APIENTRY* PFNGLGENBUFFERSPROC)(GLsizei n, GLuint* buffers);
typedef void(APIENTRY* PFNGLBINDBUFFERPROC)(GLenum target, GLuint buffer);
typedef void(APIENTRY* PFNGLBUFFERDATAPROC)(GLenum target, GLsizeiptr size, const void* data, GLenum usage);
typedef void(APIENTRY* PFNGLBUFFERSUBDATAPROC)(GLenum target, GLintptr offset, GLsizeiptr size, const void* data);
typedef void(APIENTRY* PFNGLDELETEBUFFERSPROC)(GLsizei n, const GLuint* buffers);

typedef void(APIENTRY* PFNGLGENVERTEXARRAYSPROC)(GLsizei n, GLuint* arrays);
typedef void(APIENTRY* PFNGLBINDVERTEXARRAYPROC)(GLuint array);
typedef void(APIENTRY* PFNGLDELETEVERTEXARRAYSPROC)(GLsizei n, const GLuint* arrays);

typedef void(APIENTRY* PFNGLENABLEVERTEXATTRIBARRAYPROC)(GLuint index);
typedef void(APIENTRY* PFNGLDISABLEVERTEXATTRIBARRAYPROC)(GLuint index);
typedef void(APIENTRY* PFNGLVERTEXATTRIBPOINTERPROC)(GLuint index, GLint size, GLenum type, GLboolean normalized, GLsizei stride, const void* pointer);

typedef GLuint(APIENTRY* PFNGLCREATESHADERPROC)(GLenum type);
typedef void(APIENTRY* PFNGLSHADERSOURCEPROC)(GLuint shader, GLsizei count, const GLchar* const* string, const GLint* length);
typedef void(APIENTRY* PFNGLCOMPILESHADERPROC)(GLuint shader);
typedef void(APIENTRY* PFNGLGETSHADERIVPROC)(GLuint shader, GLenum pname, GLint* params);
typedef void(APIENTRY* PFNGLGETSHADERINFOLOGPROC)(GLuint shader, GLsizei bufSize, GLsizei* length, GLchar* infoLog);
typedef void(APIENTRY* PFNGLDELETESHADERPROC)(GLuint shader);

typedef GLuint(APIENTRY* PFNGLCREATEPROGRAMPROC)();
typedef void(APIENTRY* PFNGLATTACHSHADERPROC)(GLuint program, GLuint shader);
typedef void(APIENTRY* PFNGLLINKPROGRAMPROC)(GLuint program);
typedef void(APIENTRY* PFNGLGETPROGRAMIVPROC)(GLuint program, GLenum pname, GLint* params);
typedef void(APIENTRY* PFNGLGETPROGRAMINFOLOGPROC)(GLuint program, GLsizei bufSize, GLsizei* length, GLchar* infoLog);
typedef void(APIENTRY* PFNGLUSEPROGRAMPROC)(GLuint program);
typedef void(APIENTRY* PFNGLDELETEPROGRAMPROC)(GLuint program);

typedef GLint(APIENTRY* PFNGLGETUNIFORMLOCATIONPROC)(GLuint program, const GLchar* name);
typedef void(APIENTRY* PFNGLUNIFORMMATRIX4FVPROC)(GLint location, GLsizei count, GLboolean transpose, const GLfloat* value);
typedef void(APIENTRY* PFNGLUNIFORM1IPROC)(GLint location, GLint v0);
typedef void(APIENTRY* PFNGLUNIFORM1FPROC)(GLint location, GLfloat v0);
typedef void(APIENTRY* PFNGLUNIFORM3FVPROC)(GLint location, GLsizei count, const GLfloat* value);
typedef void(APIENTRY* PFNGLUNIFORM4FVPROC)(GLint location, GLsizei count, const GLfloat* value);

typedef void(APIENTRY* PFNGLGENERATEMIPMAPPROC)(GLenum target);

// glActiveTexture is GL 1.3, which some platforms' base gl.h headers
// (e.g. Linux/Mesa) already declare as a real prototype, while others
// (e.g. the Windows SDK's, stuck at GL 1.1) do not. Only load it as a
// function pointer where the system header hasn't already given us it,
// to avoid redeclaring the same symbol as two different kinds of entity.
#ifndef GL_VERSION_1_3
typedef void(APIENTRY* PFNGLACTIVETEXTUREPROC)(GLenum texture);
extern PFNGLACTIVETEXTUREPROC glActiveTexture;
#endif

extern PFNGLGENBUFFERSPROC glGenBuffers;
extern PFNGLBINDBUFFERPROC glBindBuffer;
extern PFNGLBUFFERDATAPROC glBufferData;
extern PFNGLBUFFERSUBDATAPROC glBufferSubData;
extern PFNGLDELETEBUFFERSPROC glDeleteBuffers;

extern PFNGLGENVERTEXARRAYSPROC glGenVertexArrays;
extern PFNGLBINDVERTEXARRAYPROC glBindVertexArray;
extern PFNGLDELETEVERTEXARRAYSPROC glDeleteVertexArrays;

extern PFNGLENABLEVERTEXATTRIBARRAYPROC glEnableVertexAttribArray;
extern PFNGLDISABLEVERTEXATTRIBARRAYPROC glDisableVertexAttribArray;
extern PFNGLVERTEXATTRIBPOINTERPROC glVertexAttribPointer;

extern PFNGLCREATESHADERPROC glCreateShader;
extern PFNGLSHADERSOURCEPROC glShaderSource;
extern PFNGLCOMPILESHADERPROC glCompileShader;
extern PFNGLGETSHADERIVPROC glGetShaderiv;
extern PFNGLGETSHADERINFOLOGPROC glGetShaderInfoLog;
extern PFNGLDELETESHADERPROC glDeleteShader;

extern PFNGLCREATEPROGRAMPROC glCreateProgram;
extern PFNGLATTACHSHADERPROC glAttachShader;
extern PFNGLLINKPROGRAMPROC glLinkProgram;
extern PFNGLGETPROGRAMIVPROC glGetProgramiv;
extern PFNGLGETPROGRAMINFOLOGPROC glGetProgramInfoLog;
extern PFNGLUSEPROGRAMPROC glUseProgram;
extern PFNGLDELETEPROGRAMPROC glDeleteProgram;

extern PFNGLGETUNIFORMLOCATIONPROC glGetUniformLocation;
extern PFNGLUNIFORMMATRIX4FVPROC glUniformMatrix4fv;
extern PFNGLUNIFORM1IPROC glUniform1i;
extern PFNGLUNIFORM1FPROC glUniform1f;
extern PFNGLUNIFORM3FVPROC glUniform3fv;
extern PFNGLUNIFORM4FVPROC glUniform4fv;

extern PFNGLGENERATEMIPMAPPROC glGenerateMipmap;

// Resolves every pointer above via glfwGetProcAddress. Must be called
// after glfwMakeContextCurrent(). Returns false (and logs to stderr)
// if any required function is missing.
bool glCore33Init();
