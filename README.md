# Rendering Engine

A cross-platform 3D/2D rendering engine written in Rust using OpenGL / OpenGL ES through `glow`.

The goal of this project is to build a lightweight, modular, and flexible rendering library crate (`rendering_engine`) that runs seamlessly across desktop and mobile while utilizing modern rendering techniques (Multi-Draw Indirect, persistent mapped buffers, multi-threaded rendering, texture management, FBOs, compute shaders, and skybox passes).

---

## 🌟 Features

- **Modular Architecture (`core`, `resources`, `render`)**
  - Clean separation between low-level memory/buffers, graphics resources, and rendering pipelines.

- **Indirect Draw Batching**
  - Groups instances by material and mesh and submits indirect draw commands.
  - The current `glow` path dispatches those commands with a GL-call loop; the buffer layout is ready for native MDI when the backend exposes it.
  - Uses GPU-side instance data handling to avoid unnecessary CPU-to-GPU synchronization.

- **Textures, Atlases & Texture Arrays**
  - 2D, cubemap, and array textures; sRGB and BC1/BC4/BC5 uploads; CPU RGBA8 atlas packing and UV helpers.

- **Offscreen Framebuffers & Render Targets**
  - FBO abstractions (`Framebuffer`) with color attachments, depth textures, and depth renderbuffers.

- **Compute Shader Pipelines**
  - `ComputeShader` support for OpenGL ES 3.1+ / OpenGL 4.3+.

- **Fullscreen Skybox Pass**
  - Pre-pass skybox pipeline with `LEQUAL` depth testing and inverse view-projection uniform upload.

- **Dedicated Screen-Space UI**
  - Pixel-coordinate draw list with its own shader and buffers, separate from scene meshes and camera transforms.

- **Light Uniform Block**
  - Renderer-managed std140 UBO for up to 16 directional, point, or spot lights.
  - Shaders opt into the shared block layout; the demo uses it for diffuse lighting.

- **Persistent Mapped Buffers**
  - Uses persistent mapped SSBOs with multiple fenced regions (`TRANSFORM_REGIONS`) to safely update GPU data while avoiding CPU/GPU conflicts.

- **Dedicated Render Thread & Triple-Buffer Handoff**
  - Game logic and rendering exchange the latest frame through clone-free slot swaps. The GL context and `Renderer` are created and used only on the render thread.

- **Frame Diagnostics**
  - Per-frame counts for submitted and culled instances, draw calls, pipeline changes, and estimated triangles.

- **Desktop and Android Support**
  - Desktop support through `winit` / `glutin`.
  - Android support through `android-native-activity` and OpenGL ES.

---

## 📁 Repository Structure

```text
rendering-engine/
├── engine/                 # Core rendering engine crate (`rendering_engine`)
│   └── src/
│       ├── lib.rs
│       ├── core/           # Memory allocators, persistent buffers, math & triple buffer
│       ├── render/         # Rendering APIs, frame data, passes, scene graph
│       │   └── renderer/   # Scene API, resources, passes, sync, world and draw submission
│       ├── resources/      # GPU resources and resource managers
│       │   ├── shader/     # Shader program and manager
│       │   └── texture/    # Texture formats, atlas, and manager
│
├── demo/                   # Example voxel 3D application
│   └── src/
│
├── docs/                   # Library documentation & guides
│   └── USAGE.md            # Simple, comprehensive library usage guide
│
└── shaders_gles/           # OpenGL / OpenGL ES shader sources
```

---

## 📖 Documentation & Library Usage

For code examples on using `rendering_engine` as a library crate in your own projects, see **[Library Usage Guide](docs/USAGE.md)**.

---

## 🚀 Getting Started

### Building & Running Desktop Demo

To launch the desktop demo application:

```bash
cargo run --bin desktop
```

### Controls (Demo)

- **W / A / S / D**: Move camera forward / left / backward / right.
- **Mouse / Drag**: Look around (yaw & pitch).
- **Space / Left Ctrl**: Fly up / down.
- **E**: Toggle vertical sync.
- **G**: Toggle terrain wireframe.
- **Escape**: Release or recapture the mouse cursor.
- On Android, use the on-screen movement controls and touch-drag to look around.

### What the Demo Exercises

The voxel terrain demo combines procedural terrain generation and greedy
chunk meshing on worker threads with center-out radial chunk loading,
frustum-culled GPU chunk upload/eviction, instanced scene rendering, directional
lighting, a procedural skybox, and an on-screen frame-rate display. The
wireframe and vertical-sync toggles let you inspect geometry and presentation
timing. Desktop tuning values, including render distance, are stored in
`config.json` beside the executable after the first run.

---

## 📜 License

This project is open-source and available under the [BSD-3-Clause License](LICENSE).
