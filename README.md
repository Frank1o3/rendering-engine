# Rendering Engine

A cross-platform 3D rendering engine written in Rust using OpenGL / OpenGL ES through `glow`.

The goal of this project is to build a lightweight but flexible rendering architecture that can run on both desktop and Android while experimenting with modern rendering techniques such as GPU batching, persistent buffers, multi-threaded rendering, and efficient scene management.

---

## 🌟 Features

- **Multi-Draw Indirect (MDI) & GPU Batching**
  - Groups draw calls together to reduce CPU overhead.
  - Uses GPU-side instance data handling to avoid unnecessary CPU-to-GPU synchronization.

- **Persistent Mapped Buffers**
  - Uses persistent mapped SSBOs with multiple fenced regions (`TRANSFORM_REGIONS`) to safely update GPU data while avoiding CPU/GPU conflicts.

- **Dedicated Render Thread**
  - Game logic and rendering are separated into different threads.
  - The game thread writes frame data into a lock-free `TripleBuffer`, while the render thread consumes and submits GPU commands independently.

- **Static and Dynamic Object Handling**
  - Static objects (terrain, structures, etc.) only update when marked dirty.
  - Dynamic objects (players, moving objects, physics entities) update every frame.

- **Frustum Culling**
  - Performs CPU-side bounding sphere checks to skip objects outside the camera view.

- **OpenGL State / Pipeline Caching**
  - Tracks rendering state changes to avoid unnecessary OpenGL calls.

- **Desktop and Android Support**
  - Desktop support through `winit` / `glutin`.
  - Android support through `android-native-activity` and OpenGL ES.

---

## 🗺️ Roadmap

### 📱 Built-in UI System

A UI system integrated into the renderer itself.

Planned features:

- Screen-space layouts and anchors.
- Responsive UI panels.
- Custom bitmap/vector font rendering.
- Mobile touch controls:
  - Virtual joysticks.
  - Touch D-pads.
  - On-screen action buttons.

### ⚡ GPU-Based Culling

Moving more rendering decisions onto the GPU using compute shaders.

Planned improvements:

- GPU frustum culling.
- Occlusion culling.
- Larger scene scalability.

### 🎨 Materials and Lighting

Future rendering improvements:

- PBR material workflow.
- Dynamic lighting.
- Shadow mapping.
- More advanced shader pipelines.

---

## 📁 Repository Structure

```text
rendering-engine/
├── engine/                 # Core rendering engine crate
│   └── src/
│       ├── buffer.rs       # Persistent mapped buffers and GL buffer abstractions
│       ├── draw_indirect.rs# MDI command structures and indirect buffers
│       ├── engine.rs       # Main renderer implementation
│       ├── frame_data.rs   # Per-frame render data structures
│       ├── geometry_pool.rs# Mesh memory management
│       ├── pipeline.rs     # Cached OpenGL render states
│       ├── scene.rs        # Scene object tracking and transforms
│       └── triple_buffer.rs# Lock-free frame synchronization
│
├── demo/                   # Example application
│   └── src/
│       ├── app.rs          # Application lifecycle
│       ├── game.rs         # Game update logic and camera controls
│       ├── render_thread.rs# Render thread implementation
│       └── touch.rs        # Touch controls and UI interaction
│
└── shaders_gles/           # OpenGL/OpenGL ES shaders
```

---

## 🚀 Getting Started

### Prerequisites

- **Rust**: Nightly or Stable (Edition 2024 support required).
- **Graphics Drivers**: Support for OpenGL 3.3+ (Desktop) or OpenGL ES 3.0+ (Mobile).
- **Cargo APK** *(for Android builds)*: Install via `cargo install cargo-apk`.

### Building & Running Desktop Demo

To launch the desktop demo application:

```bash
cargo run --bin desktop
```

### Controls (Demo)

- **W / A / S / D**: Move camera forward / left / backward / right.
- **Mouse / Drag**: Look around (yaw & pitch).
- **Touch / Click**: Interact with on-screen buttons (for desktop debugging of mobile controls).

### Building for Android

Ensure you have the Android NDK and SDK configured, then run:

```bash
cargo apk build --package demo
```

To run directly on a connected Android device:

```bash
cargo apk run --package demo
```

---

## 📜 License

This project is open-source and available under the [MIT License](LICENSE).
