#!/usr/bin/env bash
# Сборка release и упаковка в .tar.gz для Linux x86_64.
# Использование: scripts/package.sh <метка>   (например: stage1)
set -euo pipefail
cd "$(dirname "$0")/.."
LABEL="${1:-dev}"
NAME="pluscraft-${LABEL}-linux-x86_64"
DIST="dist/${NAME}"

if command -v glslangValidator >/dev/null; then scripts/compile_shaders.sh >/dev/null; fi
cargo build --release

rm -rf "$DIST"
mkdir -p "$DIST/assets/shaders" "$DIST/assets/textures" "$DIST/assets/fonts" "$DIST/assets/data"
cp target/release/pluscraft "$DIST/"
strip "$DIST/pluscraft" 2>/dev/null || true
cp shaders/spv/*.spv "$DIST/assets/shaders/"
cp assets/fonts/* "$DIST/assets/fonts/"
if compgen -G "assets/data/*" > /dev/null; then cp assets/data/* "$DIST/assets/data/"; fi
# Атлас текстур и настройки по умолчанию генерирует сама игра.
(cd "$DIST" && ./pluscraft --gen-assets)
cp packaging/README.txt "$DIST/README.txt"
cp LICENSE "$DIST/LICENSE.txt"

tar -czf "dist/${NAME}.tar.gz" -C dist "$NAME"
echo "Готово: dist/${NAME}.tar.gz ($(du -h "dist/${NAME}.tar.gz" | cut -f1))"
