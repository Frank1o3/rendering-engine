# `CONTEXT.md`
## Master Reference for the Modular Rendering Engine Project

> **Purpose:** This document contains the complete architectural vision, design decisions, and implementation roadmap for the Modular Rendering Engine. Use this as the definitive source of truth when using the Continue extension or any AI assistant to help with implementation.

---

## 📌 PROJECT OVERVIEW

**Project Name:** Modular Rendering Engine (OpenGL Core + Future Vulkan)

**Core Philosophy:** Build a decoupled, reusable rendering engine that:
- Takes a lock‑free input buffer from a game engine.
- Batches objects into near‑zero CPU‑overhead draw calls.
- Caches GPU pipeline states to minimise driver validation.
- Ships a game **first**, optimises later.

**Golden Rule:** *If a task does not help you render a 3D object in your actual game right now, skip it. Stop at Phase 4. That is your “Ship” point.*

---

## 🧠 KEY DESIGN DECISIONS (The “Why”)

| Decision                                                                 | Rationale                                                                                                                                                                                                                                                               |
| :----------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **1. Triple Buffer over Mutex‑protected queue**                          | Lock‑free, avoids contention. Three slots ensure the producer never blocks even if the consumer is one frame behind. Preserves ordering (FIFO) while allowing the consumer to skip stale frames if needed.                                                              |
| **2. Persistent Mapped Buffers for transforms**                          | Eliminates per‑frame `glBufferData` calls. CPU writes directly to GPU‑visible memory. Required for true “async” pipelining (CPU prepares next frame while GPU consumes current).                                                                                        |
| **3. Multi‑Draw Indirect (MDI) as the batching mechanism**               | Reduces draw calls from `O(N)` to `O(unique_shaders)`. Pack all draw parameters into a GPU buffer and issue one call per shader group. Achieves the “30% of frame in one call” goal.                                                                                    |
| **4. Pipeline State Caching (emulating Vulkan PSOs)**                    | OpenGL drivers perform expensive validation on every `glEnable`/`glDepthFunc`. By caching the entire state combination and only applying it when it changes, we eliminate redundant driver work.                                                                        |
| **5. Game Engine handles coarse culling, Renderer handles fine culling** | Game has full scene knowledge (physics, AI, spatial partitioning) – it can cull 80% of objects early. Renderer trusts that list and only does sorting/batching. Backface culling is done by GPU hardware for free. Prevents CPU from touching individual triangles.     |
| **6. Modularity via `FrameData` struct**                                 | The game engine knows nothing about OpenGL. It only fills a `FrameData` struct with `RenderCommand`s (handles, transforms) and a camera. The renderer translates this to GL calls. Allows the renderer to be swapped for a Vulkan backend later.                        |
| **7. “Stop and Ship” at Phase 4**                                        | Prevents endless optimisation. A forward renderer with MDI and state caching is fast enough to ship a game. Compute shaders, deferred rendering, and Vulkan are **post‑ship** features, funded by game revenue.                                                         |
| **8. SSBO exposure for future specialists**                              | By binding transform and indirect buffers as Shader Storage Buffer Objects, we allow a future shader specialist to write compute shaders that perform GPU culling/lighting without rewriting our Rust code. Config flags (`use_gpu_culling`) enable seamless switching. |

---

## 📁 PROJECT STRUCTURE (Proposed)

```
src/
├── main.rs
├── app.rs                 # Winit event loop, window, GL context
├── renderer/
│   ├── mod.rs             # Re‑exports public API
│   ├── frame_data.rs      # RenderCommand, FrameData
│   ├── triple_buffer.rs   # Lock‑free triple buffer
│   ├── engine.rs          # Renderer main struct, resource handles
│   ├── mesh.rs            # Mesh loading, VAO/VBO/EBO
│   ├── shader.rs          # Shader compilation, uniform caching
│   ├── buffer.rs          # GpuBuffer with persistent mapping
│   ├── draw_indirect.rs   # DrawIndirectCommand, MDI builder
│   ├── pipeline.rs        # PipelineState, PipelineCache
│   ├── math.rs            # Matrix helpers (transform, view, proj)
│   └── renderer.rs        # (Optional) backend trait placeholder
```

---

## 🗺️ PHASED ROADMAP (Detailed)

### PHASE 0: PROJECT FOUNDATION ✅ (You are here)
**Goal:** `winit` + `glutin` + `glow` are set up. A rectangle renders with a custom shader.

- [x] 0.1 Set up `winit` event loop with `ControlFlow::Poll`.
- [x] 0.2 Create OpenGL context (4.6 Core) with `glutin`, making it current.
- [x] 0.3 Load `glow::Context` from the display’s proc address.
- [x] 0.4 Create `Renderer` struct that owns the `Arc<glow::Context>`.
- [x] 0.5 Load a simple vertex/fragment shader (colored rectangle).
- [x] 0.6 Upload vertices/indices to a `Mesh`, draw with `glDrawElements`.

---

### PHASE 1: LOCK‑FREE INPUT (The Triple Buffer)
**Goal:** Game logic builds a list of objects; the renderer reads it without blocking or allocating per frame.

- [ ] **1.1 Define `FrameData` and `RenderCommand`**
  - **File:** `src/renderer/frame_data.rs`
  - **`RenderCommand`:** `pub struct RenderCommand { pub mesh_id: MeshId, pub material_id: MaterialId, pub transform: Mat4 }`
  - **`FrameData`:** `pub struct FrameData { pub commands: Vec<RenderCommand>, pub camera_position: Vec3, pub camera_rotation: Quat }`
  - **Design:** `Mat4` is pre‑computed by the game (position, rotation, scale combined). The renderer does not compute matrices – it only uploads them.
  - **Allocation:** Pre‑allocate `commands` with `Vec::with_capacity(1024)` to avoid per‑frame reallocation.
  - **Learn:** `bytemuck` derives (`Pod`, `Zeroable`) for `RenderCommand` if we later want to upload them directly.

- [ ] **1.2 Implement the Triple Buffer**
  - **File:** `src/renderer/triple_buffer.rs`
  - **Struct:** `TripleBuffer<T>` with `slots: [T; 3]`, `back: AtomicUsize`, `mid: AtomicUsize`, `front: usize`, `fresh: AtomicBool`.
  - **Methods:**
    - `write_slot(&mut self) -> &mut T` – loads `back` with `Relaxed`, returns mutable ref.
    - `publish(&mut self)` – swaps `back` and `mid` using `AcqRel` on `mid`, stores new `back` with `Relaxed`, sets `fresh` to `true` with `Release`.
    - `consume(&mut self, out: &mut T) -> bool` – checks `fresh` with `Acquire`; if false, returns. Swaps `mid` and `front` using `AcqRel`; sets `front` to the old `mid`; copies the slot into `out`; returns true.
  - **Design decision:** `front` is non‑atomic because only the consumer touches it. Safe as long as we have one consumer thread (which we do – the render loop is single‑threaded).
  - **Learn:** `std::sync::atomic::Ordering` – `Relaxed` for indices that are always eventually consistent, `Acquire`/`Release` for publishing the data.

- [ ] **1.3 Integrate into `App`**
  - Store `TripleBuffer<FrameData>` inside `App` (or inside `GlState`).
  - In `RedrawRequested`:
    1. `let frame = app.buffer.write_slot();`
    2. `frame.commands.clear();` (keep capacity)
    3. Push your rectangle’s `RenderCommand` (using `MeshId` and `MaterialId` from `GlState`).
    4. Set camera fields.
    5. `app.buffer.publish();`
  - Call `state.renderer.render(&mut app.buffer);` – the renderer consumes.
  - **Test:** The rectangle should still draw, but now the data flow goes through the buffer.

---

### PHASE 2: THE “NEARLY 1 DRAW CALL” CORE
**Goal:** Eliminate per‑object `glDrawElements`. Use Persistent Mapping + MDI.

- [ ] **2.1 Persistent Mapped Transform Buffer**
  - **File:** `src/renderer/buffer.rs` – extend `GpuBuffer` with a new method `map_persistent(&mut self, size: usize) -> *mut u8`.
  - **Task:** Allocate a buffer with `glBufferStorage` (not `glBufferData`) using `GL_MAP_WRITE_BIT | GL_MAP_PERSISTENT_BIT | GL_MAP_COHERENT_BIT`.
  - **Map:** Call `glMapBufferRange` with the same flags. Store the returned pointer in `Renderer`.
  - **Safety:** The pointer is valid until the buffer is destroyed. We write to it every frame using `ptr::copy_nonoverlapping` or direct indexing.
  - **Learn:** OpenGL 4.4+ `glBufferStorage` vs `glBufferData`. Coherent mapping ensures writes are visible to the GPU without `glFlush`.
  - **Design decision:** We allocate enough space for `MAX_OBJECTS` (e.g., 65536) transforms. This is a fixed upper bound – we never re‑allocate, preventing stutter.

- [ ] **2.2 Build Indirect Commands**
  - **File:** `src/renderer/draw_indirect.rs`
  - **Struct:** `#[repr(C)] struct DrawIndirectCommand { count: u32, instance_count: u32, first_index: u32, base_vertex: i32, base_instance: u32 }`
  - **Task:** In `Renderer::render()`, after consuming `FrameData`:
    1. Let `offset = 0` (in units of `Mat4`).
    2. Create a local `Vec<DrawIndirectCommand>` with capacity = `commands.len()`.
    3. For each `cmd` in `frame.commands`:
       - Write `cmd.transform` to the persistent buffer at `offset * size_of::<Mat4>()`.
       - Push a `DrawIndirectCommand` with `count = mesh.index_count`, `instance_count = 1`, `first_index = 0`, `base_vertex = 0`, `base_instance = offset`.
       - Increment `offset`.
  - **Note:** We assume all meshes share the same vertex/index buffer layout, or we set `base_vertex` to the start of each mesh’s vertices (if we merge all meshes into one VBO/IBO). For simplicity, keep each mesh in its own VBO/IBO and bind the VAO before the MDI call (this requires grouping by mesh as well, but it’s acceptable for now).
  - **Alternative:** If all meshes are merged into a single VBO/IBO, `first_index` and `base_vertex` become meaningful. We’ll do this later; for now, we’ll bind the VAO per group.

- [ ] **2.3 Upload and Dispatch MDI**
  - **Task:** Upload the `Vec<DrawIndirectCommand>` to a `GL_DRAW_INDIRECT_BUFFER` using `glBufferData` with `GL_STREAM_DRAW`.
  - **Call:** `glMultiDrawElementsIndirect(GL_TRIANGLES, GL_UNSIGNED_INT, ptr::null(), command_count, 0)`.
  - **Important:** This call must happen **after** binding the correct VAO and the correct shader.
  - **Design decision:** We issue one MDI call per **shader group**, not per mesh. This means we need to split `frame.commands` by `material_id` first.

- [ ] **2.4 Group by Shader (Batching)**
  - **Task:** Before building the indirect buffer, sort `frame.commands` by `material_id` (stable sort).
  - **Loop:**
    ```rust
    let mut start = 0;
    while start < commands.len() {
        let mat_id = commands[start].material_id;
        let mut end = start + 1;
        while end < commands.len() && commands[end].material_id == mat_id { end += 1; }

        // 1. Bind shader for this group (via pipeline cache)
        // 2. Bind VAO (assuming all meshes in this group have the same VAO layout; if not, we need to group by VAO too)
        // 3. Build the indirect buffer ONLY for commands[start..end]
        // 4. Upload and call glMultiDrawElementsIndirect for this slice

        start = end;
    }
    ```
  - **Benefit:** If 1000 objects share 1 shader, it’s 1 MDI call. If they share 5 shaders, it’s 5 MDI calls.

---

### PHASE 3: PIPELINE STATE CACHING (PSO Emulation)
**Goal:** Prevent redundant OpenGL state changes.

- [ ] **3.1 Define `PipelineState` and `PipelineCache`**
  - **File:** `src/renderer/pipeline.rs`
  - **`PipelineState`:** Contains `shader_id`, `cull_mode` (None/Back/Front), `depth_test` (bool), `depth_write` (bool), `depth_func` (Less/Lequal/etc.), `blend_enabled` (bool), `src_factor`, `dst_factor`.
  - **`PipelineCache`:** `HashMap<u64, PipelineStateId>`. The key is a hash of the state combination (e.g., `fxhash` or a packed tuple).
  - **Design decision:** We hash the config rather than storing a raw `PipelineState` in the map’s key to avoid large structs in the key. The hash is deterministic.

- [ ] **3.2 Implement `apply_pipeline`**
  - **File:** `src/renderer/engine.rs` (inside `Renderer` impl).
  - **Store:** `current_pipeline_id: Option<PipelineStateId>` in `Renderer`.
  - **Logic:** If `requested_id == current_pipeline_id`, return immediately.
  - **Else:** Use the cached `PipelineState` to call `glUseProgram`, `glEnable/Disable(GL_CULL_FACE)`, `glCullFace`, `glDepthFunc`, `glDepthMask`, `glEnable/Disable(GL_BLEND)`, `glBlendFunc`.
  - **Important:** This must be called **once per unique pipeline per frame**. The first time it’s called, it will set all flags; subsequent calls with the same ID do nothing.

- [ ] **3.3 Update Material System**
  - **Task:** `Material` now stores a `PipelineStateId` instead of just a `ShaderId`.
  - **Integration:** In the MDI group loop, before building the indirect buffer for that group, call `self.apply_pipeline(material.pipeline_id)`.
  - **Benefit:** The loop now issues zero redundant state changes. This mimics Vulkan’s PSO model on OpenGL.

---

### 🚨 PHASE 4: STOP & SHIP (THE GAME BARRIER) 🚨
**Goal:** Do not add compute shaders. Do not add Vulkan. **Make a game.**

- [ ] **4.1 Build a Simple 3D Scene**
  - Load OBJ models (e.g., with `tobj` or `obj` crate).
  - Convert each mesh into our `Vertex` format.
  - Upload to the renderer via `Renderer::load_mesh()`.
  - In the game loop, push many objects with different transforms.

- [ ] **4.2 Implement FPS Camera Controls**
  - Use `winit` keyboard/mouse events to update `FrameData.camera_position` and `camera_rotation`.
  - In `math.rs`, compute `view = Mat4::look_at_rh(position, target, up)`.

- [ ] **4.3 Profile with RenderDoc**
  - Capture a frame. Check that draw calls are ≤ `number_of_unique_shaders` (plus maybe 1 for UI).
  - Verify that `glUseProgram` and state‑changing calls are minimal.

- [ ] **4.4 Publish (Itch.io / Steam)**
  - Get feedback. This is your **“Proof of Market”**.

---

### PHASE 5: THE “I HAVE A BUDGET” EXPANSION
*(Only start this after Phase 4 is complete)*

**Goal:** Make the renderer ready for a Shader Specialist / Compute Optimizations.

- [ ] **5.1 Expose SSBOs**
  - **Task:** Bind the persistent transform buffer and indirect buffer to `GL_SHADER_STORAGE_BUFFER` binding points (e.g., 0 and 1) using `glBindBufferBase`.
  - **Why:** A shader specialist can write a compute shader that reads the transform buffer, performs frustum/occlusion tests, and rewrites the indirect buffer directly on the GPU, eliminating CPU culling entirely.
  - **Documentation:** Comment in the code that binding 0 = transforms, binding 1 = indirect commands.

- [ ] **5.2 Add Debug Markers**
  - **Task:** Wrap every phase (Culling, Upload, Draw) with `glPushDebugGroup` and `glPopDebugGroup`.
  - **Why:** RenderDoc will show precise timings for each phase, making it easy to identify bottlenecks.

- [ ] **5.3 Implement `RendererConfig`**
  - **Struct:** `pub struct RendererConfig { pub use_gpu_culling: bool, pub use_gpu_lighting: bool }`.
  - **Why:** Flip a boolean to enable the specialist’s compute shaders without rewriting the CPU fallback. This also supports fallback for older hardware.

---

### PHASE 6: THE ULTIMATE VISION (Long Term)
*(Do not open this file until Phase 4 is done)*

- [ ] **6.1 Abstract the Backend:** Create `trait GraphicsBackend` with associated types for Shader, Pipeline, Buffer, Texture.
- [ ] **6.2 Move all OpenGL code into `OpenGlBackend`** implementing that trait.
- [ ] **6.3 (Optional/Expert)** Start a `VulkanBackend` using `ash` or `vulkano`, implementing the same trait.
- [ ] **6.4 Implement a Deferred Rendering path** (GBuffer, tiled lighting).
- [ ] **6.5 Implement Shader Packs (Iris style)** using SPIR‑V cross‑compilation and a standard uniform block.

---

## 🔧 CODE‑LEVEL REFERENCE SNIPPETS

### Triple Buffer (Rust)
```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct TripleBuffer<T> {
    slots: [T; 3],
    back: AtomicUsize,
    mid: AtomicUsize,
    front: usize,
    fresh: AtomicBool,
}

impl<T: Default + Clone> TripleBuffer<T> {
    pub fn new() -> Self { ... }
    pub fn write_slot(&mut self) -> &mut T { ... }
    pub fn publish(&mut self) { ... }
    pub fn consume(&mut self, out: &mut T) -> bool { ... }
}
```

### Persistent Buffer Allocation
```rust
unsafe {
    self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(handle));
    self.gl.buffer_storage(glow::ARRAY_BUFFER, size as i32, None,
        glow::MAP_WRITE_BIT | glow::MAP_PERSISTENT_BIT | glow::MAP_COHERENT_BIT);
    let ptr = self.gl.map_buffer_range(glow::ARRAY_BUFFER, 0, size as i32,
        glow::MAP_WRITE_BIT | glow::MAP_PERSISTENT_BIT | glow::MAP_COHERENT_BIT);
}
```

### MDI Call
```rust
gl.bind_buffer(glow::DRAW_INDIRECT_BUFFER, Some(indirect_buffer));
gl.buffer_data_u8_slice(glow::DRAW_INDIRECT_BUFFER, bytemuck::cast_slice(&commands), glow::STREAM_DRAW);
gl.multi_draw_elements_indirect(glow::TRIANGLES, glow::UNSIGNED_INT, ptr::null(), commands.len() as i32, 0);
```

### Pipeline Cache Hashing
```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash_pipeline_config(config: &PipelineConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    config.shader_id.hash(&mut hasher);
    config.cull_mode.hash(&mut hasher);
    config.depth_test.hash(&mut hasher);
    // ...
    hasher.finish()
}
```

---

## 🧠 FREQUENTLY ASKED QUESTIONS (from the discussion)

**Q: Why not use `wgpu` immediately for multi‑backend?**
*A: `wgpu` introduces its own abstraction. We want to deeply understand OpenGL first, ship a game, then abstract. This is the “learn the hard way so you appreciate the easy way” approach.*

**Q: Isn’t OpenGL CPU‑bound? Why bother optimising?**
*A: Yes, but with MDI + persistent mapping, we can push the bottleneck to the GPU. OpenGL can still render tens of thousands of objects with < 20 draw calls – that’s more than enough for most indie games.*

**Q: What about compute shaders for culling?**
*A: That’s Phase 5. We intentionally defer it to avoid scope creep. A simple CPU frustum cull is fine for shipping.*

**Q: How do I handle different mesh layouts (e.g., some with normals, some without)?**
*A: For now, enforce a single vertex layout (position + color). For future flexibility, we can add a `VertexLayout` enum and create separate VAOs, but that’s an optimisation for later.*

**Q: Why does the game engine pre‑compute the `Mat4` transform?**
*A: To keep the renderer dumb. The game has the full Transform (position, rotation, scale) – it can compute the matrix once and pass it. The renderer only uploads and draws. This reduces CPU work in the renderer.*

---
