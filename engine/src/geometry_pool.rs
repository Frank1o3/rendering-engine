// src/geometry_pool.rs
//
// A single shared VBO + EBO that every mesh sub-allocates from, plus one
// shared VAO.  Replaces the old model where each `Mesh` owned its own
// VBO/EBO/VAO.
//
// Vertex layout (20 bytes, must match mesh.rs Vertex):
//
//   location 0 — position  vec3  f32  offset  0   (12 bytes)
//   location 1 — normal    vec3  i8   offset 12   ( 4 bytes, xyz packed + 1 pad, normalised)
//   location 2 — color     vec4  u8   offset 16   ( 4 bytes, normalised)
//
// Instance layout (32 bytes, InstanceData from frame_data.rs):
//
//   location 3 — iPosition vec3  f32  offset  0
//   location 4 — iScale    float f32  offset 12
//   location 5 — iRotation vec4  f32  offset 16   (quaternion xyzw)

use crate::mesh::{MeshData, Vertex};
use glow::HasContext;
use std::sync::Arc;

pub const MAX_POOL_VERTICES: usize = 1_000_000;
pub const MAX_POOL_INDICES: usize = 3_000_000;

/// Where a mesh's data lives within the shared pool.
#[derive(Clone, Copy, Debug)]
pub struct MeshRange {
    pub base_vertex: i32,
    pub first_index: u32,
    pub index_count: i32,
}

pub struct GeometryPool {
    gl: Arc<glow::Context>,
    pub vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    vertex_cursor: usize,
    index_cursor: usize,
}

impl GeometryPool {
    /// `transform_buffer` is the persistent-mapped instance buffer the engine
    /// already owns — instance attributes are bound once here on the shared VAO.
    pub fn new(gl: Arc<glow::Context>, transform_buffer: glow::Buffer) -> Self {
        unsafe {
            let vao = gl.create_vertex_array().expect("Failed to create pool VAO");
            let vbo = gl.create_buffer().expect("Failed to create pool VBO");
            let ebo = gl.create_buffer().expect("Failed to create pool EBO");

            gl.bind_vertex_array(Some(vao));

            // ── Geometry buffers ────────────────────────────────────────────
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_size(
                glow::ARRAY_BUFFER,
                (MAX_POOL_VERTICES * std::mem::size_of::<Vertex>()) as i32,
                glow::STATIC_DRAW,
            );
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
            gl.buffer_data_size(
                glow::ELEMENT_ARRAY_BUFFER,
                (MAX_POOL_INDICES * std::mem::size_of::<u32>()) as i32,
                glow::STATIC_DRAW,
            );

            let stride = std::mem::size_of::<Vertex>() as i32; // 20

            // location 0: position — vec3 f32, offset 0
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);

            // location 1: normal — vec3 GL_BYTE normalised, offset 12
            // The fourth byte (offset 15) is padding and is silently ignored
            // because we only read 3 components.
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::BYTE, true, stride, 12);

            // location 2: color — vec4 GL_UNSIGNED_BYTE normalised, offset 16
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 4, glow::UNSIGNED_BYTE, true, stride, 16);

            // ── Instance buffer (locations 3 / 4 / 5) ───────────────────────
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(transform_buffer));
            let inst_stride: i32 = 32; // InstanceData: 12 (pos) + 4 (scale) + 16 (rot)

            // location 3: iPosition vec3
            gl.enable_vertex_attrib_array(3);
            gl.vertex_attrib_pointer_f32(3, 3, glow::FLOAT, false, inst_stride, 0);
            gl.vertex_attrib_divisor(3, 1);

            // location 4: iScale float
            gl.enable_vertex_attrib_array(4);
            gl.vertex_attrib_pointer_f32(4, 1, glow::FLOAT, false, inst_stride, 12);
            gl.vertex_attrib_divisor(4, 1);

            // location 5: iRotation vec4 (quaternion xyzw)
            gl.enable_vertex_attrib_array(5);
            gl.vertex_attrib_pointer_f32(5, 4, glow::FLOAT, false, inst_stride, 16);
            gl.vertex_attrib_divisor(5, 1);

            gl.bind_vertex_array(None);

            Self {
                gl,
                vao,
                vbo,
                ebo,
                vertex_cursor: 0,
                index_cursor: 0,
            }
        }
    }

    /// Uploads a mesh into the next free region of the pool.
    pub fn upload(&mut self, data: &MeshData) -> MeshRange {
        let base_vertex = self.vertex_cursor as i32;
        let first_index = self.index_cursor as u32;
        let index_count = data.indices.len() as i32;

        assert!(
            self.vertex_cursor + data.vertices.len() <= MAX_POOL_VERTICES,
            "GeometryPool vertex capacity exceeded ({} max)",
            MAX_POOL_VERTICES
        );
        assert!(
            self.index_cursor + data.indices.len() <= MAX_POOL_INDICES,
            "GeometryPool index capacity exceeded ({} max)",
            MAX_POOL_INDICES
        );

        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_sub_data_u8_slice(
                glow::ARRAY_BUFFER,
                (self.vertex_cursor * std::mem::size_of::<Vertex>()) as i32,
                bytemuck::cast_slice(&data.vertices),
            );

            self.gl
                .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            self.gl.buffer_sub_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                (self.index_cursor * std::mem::size_of::<u32>()) as i32,
                bytemuck::cast_slice(&data.indices),
            );
        }

        self.vertex_cursor += data.vertices.len();
        self.index_cursor += data.indices.len();

        MeshRange {
            base_vertex,
            first_index,
            index_count,
        }
    }
}

impl Drop for GeometryPool {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_buffer(self.ebo);
        }
    }
}
