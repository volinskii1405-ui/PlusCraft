# PlusCraft

A minimal Minecraft-style voxel sandbox written from scratch in C++17 and
OpenGL 3.3 core. This is a first, deliberately small MVP: a small
procedurally generated world you can walk around, look at, and dig
into.

## What's here

- A first-person walking camera (WASD + mouse look, Space to jump,
  Left Shift to sneak, Left Ctrl to sprint) with gravity and AABB
  collision against the terrain - you walk on the ground and can't
  clip through blocks. Sneaking also lowers your eye height and, while
  on ground, refuses to walk you off an edge with nothing underneath
  (and overrides sprint, same as vanilla). `R` teleports you back to
  the spawn point.
- A 2x2 grid of 32x48x32 chunks (64x48x64 blocks total) generated from
  one continuous value-noise heightmap, with grass/dirt/stone layers,
  sandy beaches at low elevation, and a scattering of trees; chunks
  are meshed against each other so there's no seam at the borders.
- Textured cubes: an 11-tile texture atlas (grass, dirt, stone, sand,
  wood, planks, wool, glass, leaves) is generated procedurally at
  startup, so the repo ships with zero external image assets. Glass is
  genuinely see-through (real alpha blending, not just an on/off
  cutout like leaves) - a faint tinted pane with a brighter frame.
- A 9-slot hotbar bar at the bottom of the screen, one icon per block
  (dirt, stone, sand, wood, planks, wool, glass, leaves, grass) with
  the selected slot outlined. Keys `1`-`9` or the mouse wheel change
  the selection.
- Break/place: left click removes the block you're looking at, right
  click places the currently selected block against it; hold either
  button down to repeat.
- A static crosshair at screen center; whatever block it's over
  (within a 5-block reach) gets a blinking white wireframe outline.
- Face-culled meshing: only the faces touching air (or, for leaves and
  glass, touching something other than more of the same block) are
  actually drawn.
- A main menu (its own tiny bitmap-font text renderer, no image
  assets) before you ever touch a chunk: **Create World** prompts for
  a name, picks a random seed, and drops you in; every world you've
  made is also listed there, click one to keep playing where you left
  off. `Esc` in-game saves and returns to the desktop; there's no
  separate "save" button because leaving the game *is* the save.

## What's deliberately *not* here yet

This is an MVP, not a full clone. No infinite world (just a fixed 2x2
chunk grid), no inventory or crafting, no mobs, no swimming. The code
is structured (`Chunk`, `World`, `Camera`, `Player`, `Shader`,
`TextureAtlas`) so those are natural next additions rather than
rewrites.

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

**Menu:** click **CREATE WORLD**, type a name, `Enter` to confirm
(`Esc` cancels back to the list); click any listed world to load it;
`Esc` at the list quits.

**In game:**

| Input                 | Action                      |
|------------------------|------------------------------|
| `W` `A` `S` `D`         | Move                         |
| Mouse                  | Look around                  |
| `Space`                | Jump                         |
| `Left Shift`           | Sneak (slower, lower, can't fall off edges) |
| `Left Ctrl`            | Sprint (faster; overridden by sneak) |
| `R`                    | Respawn                      |
| Left click (hold to repeat) | Break the targeted block |
| Right click (hold to repeat) | Place the selected block |
| `1`-`9` / mouse wheel  | Select block to place        |
| `Esc`                  | Save and quit to desktop     |

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
  then Y; when a move would intersect a solid block (or, while
  sneaking and on ground, would leave no block underneath), it
  binary-searches how far along that step it can actually go, so it
  lands flush against the surface or edge rather than stopping short
  or clipping in (landing on Y while falling also sets "on ground",
  which is what allows the next jump). Move speed is a base walk speed
  scaled down while sneaking or up while sprinting (sneaking wins if
  both are held).
- **`TextureAtlas`** - generates an 11-tile 176x16 RGBA texture in
  memory on startup. Most tiles are a base color plus per-pixel hash
  noise so they read as "textured" instead of flat; a few are
  hand-built instead where per-pixel noise reads wrong - stone uses
  coarser, lower-frequency blotches with a few sparse dark/light flecks
  so it looks like mottled rock rather than uniform gravel, and planks
  are four horizontal bands (with seam lines between them) in
  alternating shades rather than a single noisy color. The leaves tile
  punches random alpha holes for a leafy silhouette (the fragment
  shader `discard`s anything below alpha 0.1); glass instead keeps
  every texel but at low alpha (~22%, brighter and less transparent in
  a 1px frame around the edge) and actually blends - `main.cpp` enables
  `GL_BLEND` with standard `(SRC_ALPHA, ONE_MINUS_SRC_ALPHA)` once at
  startup, which is a no-op for every other block since they're all
  fully opaque.
- **`Chunk`** - owns a flat `BlockType` array, procedurally fills it
  (heightmap + trees) in `generate()`, and turns it into a single
  interleaved vertex buffer in `rebuildMesh()`: for every solid block,
  each of its 6 faces is only emitted if the neighboring block in that
  direction is transparent (air, or a different block of leaves/glass).
  `generate()` and `rebuildMesh()` both work in world-space coordinates
  (given the chunk's own offset), so terrain and face culling are both
  continuous across chunk borders instead of repeating or seaming.
- **`World`** - owns a `ChunksX x ChunksZ` grid of `Chunk`s (currently
  2x2) behind a `getBlock`/`setBlock`/`raycast`/`render` interface that
  doesn't care how many chunks there are. `getBlock`/`setBlock`
  translate a world coordinate to (chunk, local coordinate); `setBlock`
  also rebuilds any neighboring chunk whose shared faces the edit could
  have changed. `raycast()` marches in small steps along the camera's
  look vector to find the targeted block and the empty cell just before
  it (for placement).
- **`Highlight`** - draws a blinking wireframe cube (12 `GL_LINES`
  edges, its own unlit shader) around the targeted block, slightly
  larger than a unit cube so it doesn't z-fight with the block's own
  faces. Drawn with the same view/projection as the world, before the
  UI pass turns depth testing off.
- **`Font` / `Ui`** - `Font.h` is a hand-authored 5x7 bitmap font (just
  A-Z, 0-9, space, `-`, `_`, `.` - enough for menu/HUD text, no image
  assets). `Ui` is a tiny 2D overlay (its own shader + one dynamic quad
  buffer, drawn with depth testing off after the 3D scene): the
  crosshair is two rectangles at screen center, `drawRect`/`drawText`
  (one small quad per lit glyph pixel) are what the menu and the
  hotbar's slot backgrounds are built out of, and `drawIcon` is a
  textured quad sampling a block's side tile straight out of the atlas
  - `main.cpp` draws one per hotbar slot, each frame, in a loop.
- **`Menu`** - the pre-game screen's input/state machine: a
  **CREATE WORLD** button, a click-to-load list of `worlds/*.wrld`
  (scanned via `WorldIO::listWorldNames`), and a name-entry sub-screen
  (typed characters arrive through a `glfwSetCharCallback`, forwarded
  to `Menu::onChar`). It only decides *what* should happen
  (`Action::StartNewWorld` / `LoadWorld` / `Quit`); `main.cpp` is the
  one that actually creates or loads a `World`.
- **`WorldIO`** - `.wrld` save files: a small binary header (seed,
  world/chunk dimensions for a sanity check on load, player position,
  selected hotbar slot) followed by every chunk's raw `BlockType`
  array back to back. Loading builds a `World` in its "empty" mode
  (see below) and pours the saved bytes straight into each chunk via
  `loadChunkBlocks`, then meshes once - so edits round-trip exactly,
  nothing is regenerated from the seed. `listWorldNames`/`pathForName`
  handle the `worlds/` directory and filename sanitizing for the menu.
  New `BlockType`s always get added at the end of the enum (before
  `Count`) rather than inserted, so old `.wrld` files keep decoding to
  the same blocks they were saved with.
- **`main.cpp`** - GLFW window/input glue. Starts on the menu screen
  (normal visible cursor, no `World`/`Player`/`Camera` yet - those are
  built lazily once the menu picks a world); switches to the FPS
  screen (cursor disabled, gameplay loop as described above) once one
  does. On quit, if a game was in progress, it's saved back to its
  `.wrld` file before `glfwTerminate()`.

## License

MIT - see [LICENSE](LICENSE).
