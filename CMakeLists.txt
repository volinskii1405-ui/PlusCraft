cmake_minimum_required(VERSION 3.16)
project(PlusCraft LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release)
endif()

include(FetchContent)

set(GLFW_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_DOCS OFF CACHE BOOL "" FORCE)
set(GLFW_INSTALL OFF CACHE BOOL "" FORCE)

FetchContent_Declare(
    glfw
    GIT_REPOSITORY https://github.com/glfw/glfw.git
    GIT_TAG 3.4
    GIT_SHALLOW TRUE
)

FetchContent_Declare(
    glm
    GIT_REPOSITORY https://github.com/g-truc/glm.git
    GIT_TAG 1.0.1
    GIT_SHALLOW TRUE
)

FetchContent_MakeAvailable(glfw glm)

find_package(OpenGL REQUIRED)

add_executable(PlusCraft
    src/main.cpp
    src/gl_core33.cpp
    src/Shader.cpp
    src/Camera.cpp
    src/Texture.cpp
    src/Chunk.cpp
    src/World.cpp
    src/Player.cpp
)

target_link_libraries(PlusCraft PRIVATE glfw OpenGL::GL glm::glm)

if(MSVC)
    target_compile_options(PlusCraft PRIVATE /W4)
else()
    target_compile_options(PlusCraft PRIVATE -Wall -Wextra)
endif()
