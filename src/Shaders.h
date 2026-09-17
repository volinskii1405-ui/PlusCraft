#pragma once

// GLSL sources embedded as strings so the executable is self-contained
// and doesn't need to locate shader files relative to the working
// directory at runtime.

inline const char* kBlockVertexShader = R"glsl(
#version 330 core
layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;

out vec3 vNormal;
out vec2 vTexCoord;
out float vAO;

uniform mat4 uView;
uniform mat4 uProjection;

void main() {
    gl_Position = uProjection * uView * vec4(aPos, 1.0);
    vNormal = aNormal;
    vTexCoord = aTexCoord;
}
)glsl";

inline const char* kBlockFragmentShader = R"glsl(
#version 330 core
in vec3 vNormal;
in vec2 vTexCoord;

out vec4 FragColor;

uniform sampler2D uAtlas;

void main() {
    vec4 texColor = texture(uAtlas, vTexCoord);
    if (texColor.a < 0.1) {
        discard;
    }

    vec3 lightDir = normalize(vec3(0.4, 1.0, 0.3));
    float diffuse = max(dot(normalize(vNormal), lightDir), 0.0);
    float light = 0.45 + 0.55 * diffuse;

    FragColor = vec4(texColor.rgb * light, texColor.a);
}
)glsl";
