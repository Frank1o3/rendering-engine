# Workspace Context

Generated: 2026-07-11T18:34:38.651Z
Files indexed: 16

## File Structure

- `Cargo.toml` — Rust package for rendering engine
- `examples/demo.rs` — Renders a 3D cube and UI using the rendering engine
- `examples/platformer.rs` — Platformer game engine implementation
- `system.md` — System documentation guidelines for code development and testing
- `src/buffer.rs` — GPU buffer management for persistent mapping and data upload
- `src/engine.rs` — Renderer engine for 3D graphics rendering
- `src/lib.rs` — Expose engine modules and types for game usage
- `src/frame_data.rs` — GPU data structures for rendering 3D frames
- `src/draw_indirect.rs` — Multi-Draw Indirect infrastructure for OpenGL rendering
- `src/mesh.rs` — Vertex and mesh data structure and utility functions for 3D rendering
- `src/scene.rs` — Scene object registry with dirty transform tracking and static/dynamic separation.
- `src/pipeline.rs` — OpenGL pipeline state management and caching
- `src/shader.rs` — Renders 3D graphics using the glow library and glam math library.
- `src/triple_buffer.rs` — Lock-free triple buffer implementation for concurrent data exchange
- `src/geometry_pool.rs` — Geometry pool implementation for shared VBO, EBO, and VAO.
- `src/math.rs` — Mathematical utility functions for a game engine.

## Detailed Summaries

### `Cargo.toml`

**Purpose:** Rust package for rendering engine

**Key elements:**
- `rendering_engine`

**Dependencies:**
- `anyhow`
- `bytemuck`
- `env_logger`
- `glam`
- `glow`
- `glutin`
- `log`
- `winit`

### `examples/demo.rs`

**Purpose:** Renders a 3D cube and UI using the rendering engine

**Key elements:**
- `DemoState`
- `DemoApp`
- `create_cube_mesh`
- `create_quad_mesh`
- `Keys`
- `MeshData`
- `Vertex`
- `Renderer`

**Dependencies:**
- `glow`
- `glutin`
- `glutin_winit`
- `log`
- `raw_window_handle`
- `winit`
- `glam`
- `rendering_engine`

### `examples/platformer.rs`

**Purpose:** Platformer game engine implementation

**Key elements:**
- `Platformer`
- `Game`
- `Entity`
- `Sprite`
- `Collision`
- `Input`
- `Render`
- `Update`

**Dependencies:**
- `glam`
- `winit`
- `glow`

### `system.md`

**Purpose:** System documentation guidelines for code development and testing

**Key elements:**
- `read_request`
- `write_code`
- `edit_file`
- `add_comments`
- `generate_typescript`
- `generate_python`
- `write_tests`
- `run_commands`

**Dependencies:**
- `rustc`
- `rust`
- `cargo`

### `src/buffer.rs`

**Purpose:** GPU buffer management for persistent mapping and data upload

**Key elements:**
- `PersistentMappedBuffer`
- `GpuBuffer`
- `write_instance`
- `new`
- `upload_data`
- `InstanceData`
- `create_buffer`
- `buffer_storage`

**Dependencies:**
- `crate::frame_data`
- `glow`
- `std::sync`
- `bytemuck`

### `src/engine.rs`

**Purpose:** Renderer engine for 3D graphics rendering

**Key elements:**
- `Renderer`
- `MaterialEntry`
- `Mesh`
- `ShaderProgram`
- `Scene`
- `PipelineCache`
- `GeometryPool`
- `IndirectBuffer`

**Dependencies:**
- `glow`
- `bytemuck`
- `glam`
- `std::collections`
- `std::fs`
- `std::path`
- `std::sync`

### `src/lib.rs`

**Purpose:** Expose engine modules and types for game usage

**Key elements:**
- `Renderer`
- `DrawElementsIndirectCommand`
- `MdiStrategy`
- `FrameData`
- `InstanceData`
- `RenderCommand`
- `Mesh`
- `PipelineState`

**Dependencies:**
- `buffer`
- `draw_indirect`
- `engine`
- `frame_data`
- `geometry_pool`
- `math`
- `pipeline`
- `scene`
- `shader`
- `triple_buffer`

### `src/frame_data.rs`

**Purpose:** GPU data structures for rendering 3D frames

**Key elements:**
- `InstanceData`
- `RenderCommand`
- `FrameData`

**Dependencies:**
- `crate::engine`
- `bytemuck`
- `glam`

### `src/draw_indirect.rs`

**Purpose:** Multi-Draw Indirect infrastructure for OpenGL rendering

**Key elements:**
- `DrawElementsIndirectCommand`
- `MdiStrategy`
- `IndirectBuffer`
- `upload`
- `upload_count`
- `dispatch`
- `new`

**Dependencies:**
- `bytemuck`
- `glow`
- `std::sync::Arc`

### `src/mesh.rs`

**Purpose:** Vertex and mesh data structure and utility functions for 3D rendering

**Key elements:**
- `Vertex`
- `MeshData`
- `Mesh`
- `pack_normal`
- `compute_normals`
- `fix_winding`
- `bounding_radius`
- `new`

**Dependencies:**
- `bytemuck`
- `glam`
- `log`

### `src/scene.rs`

**Purpose:** Scene object registry with dirty transform tracking and static/dynamic separation.

**Key elements:**
- `ObjectHandle`
- `ObjectKind`
- `SceneObject`
- `SortedInstance`
- `Scene`
- `add_object`
- `remove_object`
- `set_transform`
- `flush_dirty`

**Dependencies:**
- `crate::engine`
- `crate::frame_data`
- `glam`
- `std::collections`

### `src/pipeline.rs`

**Purpose:** OpenGL pipeline state management and caching

**Key elements:**
- `PipelineStateId`
- `PipelineState`
- `CullMode`
- `DepthFunc`
- `BlendFactor`
- `PipelineCache`
- `register`
- `get`

**Dependencies:**
- `std::collections::HashMap`
- `std::hash`
- `glow`

### `src/shader.rs`

**Purpose:** Renders 3D graphics using the glow library and glam math library.

**Key elements:**
- `ShaderProgram`
- `compile_shader`
- `new`
- `from_files`
- `set_vp`
- `set_vec3`
- `set_f32`
- `set_mat4`

**Dependencies:**
- `glow`
- `std::fs`
- `std::path::Path`
- `std::sync::Arc`
- `glam`

### `src/triple_buffer.rs`

**Purpose:** Lock-free triple buffer implementation for concurrent data exchange

**Key elements:**
- `SharedState`
- `WriteHandle`
- `ReadHandle`
- `new_triple_buffer`
- `write_slot`
- `publish`
- `consume`
- `UnsafeCell`

**Dependencies:**
- `std::cell::UnsafeCell`
- `std::sync::Arc`
- `std::sync::atomic`

### `src/geometry_pool.rs`

**Purpose:** Geometry pool implementation for shared VBO, EBO, and VAO.

**Key elements:**
- `GeometryPool`
- `MeshRange`
- `MeshData`
- `Vertex`
- `upload`
- `new`

**Dependencies:**
- `crate::mesh`
- `glow`
- `std::sync::Arc`
- `bytemuck`

### `src/math.rs`

**Purpose:** Mathematical utility functions for a game engine.

**Key elements:**
- `HashableF32`
- `hash_vec3`
- `hash_quat`
- `Transform`
- `transform_to_model_matrix`
- `camera_to_view_matrix`
- `camera_to_projection_matrix`
- `extract_frustum_planes`
- `sphere_inside_frustum`

**Dependencies:**
- `glam`
- `std::collections`
- `std::hash`

### `.gitignore`

**Purpose:** Gitignore file to exclude unnecessary files from version control
