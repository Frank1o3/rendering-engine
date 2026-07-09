// src/lib.rs

// Expose all the engine modules publicly so the game (examples/) can use them.
pub mod buffer;
pub mod draw_indirect;
pub mod engine;
pub mod frame_data;
pub mod math;
pub mod mesh;
pub mod scene;
pub mod shader;
pub mod triple_buffer;

// Optional: Re-export the most commonly used types at the root level
// so the game engine can just do `use rendering_engine::Renderer;`
pub use draw_indirect::{DrawElementsIndirectCommand, MdiStrategy};
pub use engine::Renderer;
pub use frame_data::{FrameData, InstanceData, RenderCommand};
pub use mesh::{Mesh, MeshData, Vertex};
pub use scene::{ObjectHandle, ObjectKind, Scene};
pub use triple_buffer::{ReadHandle, WriteHandle, new_triple_buffer};
