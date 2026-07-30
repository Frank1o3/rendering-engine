# Using `rendering_engine` as a Library

`rendering_engine` is a modular, high-performance, cross-platform 3D/2D graphics library written in Rust using `glow` (OpenGL / OpenGL ES).

---

## Crate Organization

The library is organized into three domain-oriented modules:

```rust
use rendering_engine::core;      // Math, Persistent Mapped Buffers, Free-List Allocator, TripleBuffer
use rendering_engine::resources; // Vertex, MeshData, GeometryPool, ShaderManager, TextureManager, MaterialManager, ComputeShader
use rendering_engine::render;    // Renderer, PipelineState, Framebuffer, SkyboxPipeline, Scene
```

---

## 1. Initializing the Renderer

The main entry point is `rendering_engine::render::Renderer`. It consumes a `glow::Context` and a `ReadHandle<FrameData>` (from a `triple_buffer` SPSC queue for off-thread rendering).

```rust
use rendering_engine::core::triple_buffer::new_triple_buffer;
use rendering_engine::render::frame_data::FrameData;
use rendering_engine::render::renderer::Renderer;
use std::sync::Arc;

// 1. Create triple buffer for thread synchronization
let (write_handle, read_handle) = new_triple_buffer::<FrameData>();

// 2. Initialize Renderer on your render thread with a glow context
let mut renderer = Renderer::new(gl_context, read_handle);
```

---

## 2. Shaders, Textures, and Materials

### Shader Compilation & Loading
Shaders can be compiled from raw strings, loaded from files, or loaded from embedded directories (`include_dir`):

```rust
use rendering_engine::resources::shader::ShaderId;

let shader_id = renderer.load_shader(vert_src, None, frag_src);
```

### Texture Support
The engine supports 2D textures, 2D texture arrays, and texture atlases:

```rust
use rendering_engine::resources::texture::{Texture, TextureFilter, TextureAtlas};

// 2D Texture from raw RGBA bytes
let texture = Texture::from_rgba8(
    renderer.context(),
    width,
    height,
    &rgba_bytes,
    TextureFilter::LinearMipmapLinear,
).unwrap();
let texture_id = renderer.textures.register(texture);

// Texture Atlas UV Rect calculation
let atlas = TextureAtlas::new(texture, (16, 16), (4, 4));
let [u_min, v_min, u_max, v_max] = atlas.uv_rect(tile_x, tile_y);
```

### Material Creation
Materials group a shader, pipeline state (cull mode, depth testing, blending), and optional texture bindings:

```rust
use rendering_engine::render::pipeline::{PipelineState, CullMode, DepthFunc, BlendFactor};
use rendering_engine::resources::material::MaterialId;

let pipeline = PipelineState {
    shader_id: shader_id.0,
    cull_mode: CullMode::Back,
    depth_test: true,
    depth_write: true,
    depth_func: DepthFunc::Less,
    blend_enabled: false,
    src_factor: BlendFactor::One,
    dst_factor: BlendFactor::Zero,
};

let material_id = renderer.create_material_with_texture(shader_id, pipeline, Some(texture_id));
```

---

## 3. Geometry & Meshes

Vertices store position, packed normal, color, and UV coordinates:

```rust
use rendering_engine::resources::mesh::{MeshData, Vertex};

let mut mesh = MeshData {
    vertices: vec![
        Vertex::new_with_uv([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [255, 255, 255, 255], [0.0, 0.0]),
        Vertex::new_with_uv([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [255, 255, 255, 255], [1.0, 0.0]),
        Vertex::new_with_uv([1.0, 1.0, 0.0], [0.0, 0.0, 1.0], [255, 255, 255, 255], [1.0, 1.0]),
    ],
    indices: vec![0, 1, 2],
};

let mesh_id = renderer.load_mesh(mesh).unwrap();
```

---

## 4. Scene Objects (Static vs Dynamic)

Objects are registered into the engine's `Scene`:

```rust
use rendering_engine::render::scene::ObjectKind;

// Static object (uploaded once, rendered via Multi-Draw Indirect)
let handle = renderer.add_object(mesh_id, material_id, ObjectKind::Static);

// Position & rotation
renderer.set_position(handle, glam::Vec3::new(0.0, 1.0, 0.0));
```

---

## 5. Advanced Engine Features

### Skybox Pass
Enable a fullscreen skybox pass with procedural or cubemap shaders:

```rust
let skybox_shader = renderer.setup_skybox(skybox_vert_src, skybox_frag_src);
renderer.enable_skybox(true);
renderer.set_skybox_color(glam::Vec3::new(0.5, 0.7, 1.0));
```

### Offscreen Render Targets (FBOs)
Render to offscreen framebuffers:

```rust
use rendering_engine::render::framebuffer::Framebuffer;

let fbo = Framebuffer::new(renderer.context(), 1920, 1080, true).unwrap();
fbo.bind();
// Perform offscreen rendering passes
fbo.unbind(default_width, default_height);
```

### Compute Shaders
Dispatch compute shaders on OpenGL ES 3.1+ / OpenGL 4.3+:

```rust
use rendering_engine::resources::compute::ComputeShader;

let compute = ComputeShader::new(renderer.context(), compute_src).unwrap();
compute.dispatch(16, 16, 1);
compute.memory_barrier(glow::SHADER_IMAGE_ACCESS_BARRIER_BIT);
```

---

## 6. Rendering Loop Execution

Submit per-frame commands on your main/game thread and call `renderer.render()` on your render thread:

```rust
// On Render Thread:
renderer.render();
```
