# Rendering Engine

A cross-platform 3D/2D rendering engine written in Rust using OpenGL / OpenGL ES through `glow`.

The goal of this project is to build a lightweight, modular, and flexible rendering library crate (`rendering_engine`) that runs seamlessly across desktop and mobile while utilizing modern rendering techniques (Multi-Draw Indirect, persistent mapped buffers, multi-threaded rendering, texture management, FBOs, compute shaders, and skybox passes).

---

## 🌟 Features

- **Modular Architecture (`core`, `resources`, `render`)**
  - Clean separation between low-level memory/buffers, graphics resources, and rendering pipelines.

- **Multi-Draw Indirect (MDI) & GPU Batching**
  - Groups draw calls together to reduce CPU overhead.
  - Uses GPU-side instance data handling to avoid unnecessary CPU-to-GPU synchronization.

- **Textures, Atlases & Texture Arrays**
  - 2D textures, 2D texture arrays (`GL_TEXTURE_2D_ARRAY`), and UV atlas helpers (`TextureAtlas`).

- **Offscreen Framebuffers & Render Targets**
  - FBO abstractions (`Framebuffer`) with color attachments, depth textures, and depth renderbuffers.

- **Compute Shader Pipelines**
  - `ComputeShader` support for OpenGL ES 3.1+ / OpenGL 4.3+.

- **Fullscreen Skybox Pass**
  - Pre-pass skybox pipeline with `LEQUAL` depth testing and inverse view-projection uniform upload.

- **Persistent Mapped Buffers**
  - Uses persistent mapped SSBOs with multiple fenced regions (`TRANSFORM_REGIONS`) to safely update GPU data while avoiding CPU/GPU conflicts.

- **Dedicated Render Thread & Lock-Free Triple Buffering**
  - Game logic and rendering are separated into different threads via a lock-free `TripleBuffer`.

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
│       ├── resources/      # Meshes, geometry pool, shaders, textures, materials, compute
│       └── render/         # Renderer facade, passes, MDI, framebuffers, skybox, scene graph
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

---

## 📜 License

This project is open-source and available under the [BSD-3-Clause License](LICENSE).
