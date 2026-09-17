# PlusCraft

A minimal Minecraft-style voxel sandbox written from scratch in C++17 and
OpenGL 3.3 core. This is a first, deliberately small MVP: a single
procedurally generated chunk of landscape you can walk around, look at,
and dig into.

## What's here

- A first-person walking camera (WASD + mouse look, Space to jump)
  with gravity and AABB collision against the terrain - you walk on
  the ground and can't clip through blocks.
- One 32x48x32 chunk of terrain generated from a small value-noise
  heightmap, with grass/dirt/stone layers, sandy beaches at low
  elevation, and a scattering of trees.
- Textured cubes: an 8-tile texture atlas (grass, dirt, stone, sand,
  wood, leaves) is generated procedurally at startup, so the repo ships
  with zero external image assets.
- Break/place: left click removes the block you're looking at, right
  click places the currently selected block against it. Keys `1`-`6`
  switch the selected block (dirt, stone, sand, wood, leaves, grass).
- Face-culled meshing: only the faces touching air (or, for leaves,
  touching something other than more leaves) are actually drawn.

## What's deliberately *not* here yet

This is an MVP, not a full clone. No multiple chunks / infinite world,
no inventory or crafting, no saving/loading, no mobs, no sprinting or
swimming. The code is structured (`Chunk`, `World`, `Camera`, `Player`,
`Shader`, `TextureAtlas`) so those are natural next additions rather
than rewrites.

## Building

Requires a C++17 compiler, CMake >= 3.16, and an internet connection the
first time you configure (CMake `FetchContent` pulls in
[GLFW](https://www.glfw.org/) and [GLM](https://github.com/g-truc/glm);
OpenGL itself just needs to be available on your system - no other
dependencies, no bundled loader library, no image assets to fetch).

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j
./build/PlusCraft        # Linux/macOS
# build\Release\PlusCraft.exe   on Windows
```

On Linux you'll also need the usual OpenGL/X11 development headers if
they aren't already installed, e.g. on Debian/Ubuntu:

```sh
sudo apt install libgl1-mesa-dev libx11-dev libxrandr-dev libxinerama-dev \
                  libxcursor-dev libxi-dev
```

## Controls

| Input                 | Action                      |
|------------------------|------------------------------|
| `W` `A` `S` `D`         | Move                         |
| Mouse                  | Look around                  |
| `Space`                | Jump                         |
| Left click             | Break the targeted block     |
| Right click            | Place the selected block     |
| `1`-`6`                | Select block to place        |
| `Esc`                  | Quit                         |

## How it's built

- **`gl_core33.h/.cpp`** - a small hand-written OpenGL function loader.
  System `GL/gl.h` only exposes OpenGL 1.1, so everything from 1.5
  (buffer objects) through 3.3 core (shaders, VAOs) is resolved at
  runtime via `glfwGetProcAddress`, the same idea a generated loader
  like GLAD gives you, just written out by hand for the ~30 functions
  this project actually calls.
- **`Shader`** - compiles/links a vertex+fragment program and exposes
  small `setMat4`/`setInt`/`setFloat`/`setVec3` helpers. Shader source
  lives inline in `Shaders.h` as raw string literals.
- **`Camera`** - look direction only (yaw/pitch -> front vector,
  `glm::lookAt` for the view matrix); its position is just copied from
  `Player` each frame.
- **`Player`** - axis-separated AABB physics: gravity, jumping, and
  collision against solid blocks. Each frame it moves on X, then Z,
  then Y, and reverts+zeroes the velocity on whichever axis' move
  would intersect a solid block (landing on Y while falling also sets
  "on ground", which is what allows the next jump).
- **`TextureAtlas`** - generates an 8-tile 128x16 RGBA texture in
  memory on startup. Each tile is filled with a base color plus
  per-pixel hash noise so it reads as "textured" instead of flat; the
  leaves tile additionally punches random alpha holes and the fragment
  shader `discard`s low-alpha texels for a leafy silhouette.
- **`Chunk`** - owns a flat `BlockType` array, procedurally fills it
  (heightmap + trees) in `generate()`, and turns it into a single
  interleaved vertex buffer in `rebuildMesh()`: for every solid block,
  each of its 6 faces is only emitted if the neighboring block in that
  direction is transparent (air, or a different block's leaves).
- **`World`** - wraps the one `Chunk` + `TextureAtlas` behind a
  `getBlock`/`setBlock`/`raycast`/`render` interface, so a future
  multi-chunk world doesn't have to change `main.cpp`. `raycast()`
  marches in small steps along the camera's look vector to find the
  targeted block and the empty cell just before it (for placement).
- **`main.cpp`** - GLFW window/input glue: builds the shader, world,
  player and camera, then each frame turns WASD into a wish direction,
  updates the player's physics, copies its eye position into the
  camera, does the two raycasts on a fresh left/right click, and draws.

## License

MIT - see [LICENSE](LICENSE).
