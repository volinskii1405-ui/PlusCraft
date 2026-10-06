#!/usr/bin/env bash
# Перекомпилирует GLSL -> SPIR-V в shaders/spv (коммитятся в репозиторий,
# чтобы проект собирался без Vulkan SDK).
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p shaders/spv
for f in shaders/*.vert shaders/*.frag; do
    name=$(basename "$f")
    glslangValidator -V --target-env vulkan1.1 -Ishaders -o "shaders/spv/$name.spv" "$f" >/dev/null
    echo "ok: $name"
done
