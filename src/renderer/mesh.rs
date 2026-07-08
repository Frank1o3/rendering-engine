use std::sync::Arc;

use crate::renderer::buffer::GpuBuffer;
use bytemuck::{Pod, Zeroable};
use glow::HasContext;

// The optimized 16-byte vertex struct
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3], // 12 bytes
    pub color: [u8; 4],     // 4 bytes
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub struct Mesh {
    gl: Arc<glow::Context>,
    pub vao: glow::VertexArray,
    #[allow(unused)]
    pub vbo: GpuBuffer,
    #[allow(unused)]
    pub ebo: GpuBuffer,
    pub index_count: i32,
}

impl Mesh {
    pub fn new(gl: Arc<glow::Context>, data: &MeshData) -> Self {
        unsafe {
            let vao = gl.create_vertex_array().expect("Failed to create VAO");
            let vbo = GpuBuffer::new(gl.clone(), glow::ARRAY_BUFFER);
            let ebo = GpuBuffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);

            gl.bind_vertex_array(Some(vao));

            vbo.upload_data(&data.vertices, glow::STATIC_DRAW);
            ebo.upload_data(&data.indices, glow::STATIC_DRAW);

            let stride = std::mem::size_of::<Vertex>() as i32; // 16 bytes

            // Attribute 0: Position (f32)
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);

            // Attribute 1: Color (u8 normalized to 0.0-1.0)
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 4, glow::UNSIGNED_BYTE, true, stride, 12);

            gl.bind_vertex_array(None);

            Self {
                gl: gl,
                vao,
                vbo,
                ebo,
                index_count: data.indices.len() as i32,
            }
        }
    }
}

impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_vertex_array(self.vao);
        }
    }
}
