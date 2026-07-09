use std::sync::Arc;

use crate::buffer::GpuBuffer;
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
    pub fn new(gl: Arc<glow::Context>, data: &MeshData, transform_buffer: glow::Buffer) -> Self {
        unsafe {
            let vao = gl.create_vertex_array().expect("Failed to create VAO");
            let vbo = GpuBuffer::new(gl.clone(), glow::ARRAY_BUFFER);
            let ebo = GpuBuffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);

            gl.bind_vertex_array(Some(vao));
            vbo.upload_data(&data.vertices, glow::STATIC_DRAW);
            ebo.upload_data(&data.indices, glow::STATIC_DRAW);

            let stride = std::mem::size_of::<Vertex>() as i32;

            // Attribute 0: Position
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);

            // Attribute 1: Color
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 4, glow::UNSIGNED_BYTE, true, stride, 12);

            // ==========================================
            // NEW: Instanced Transform Attributes (Locations 2, 3, 4)
            // ==========================================
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(transform_buffer));
            let inst_stride = 32; // 32 bytes for InstanceData: 12 (pos) + 4 (scale) + 16 (rot)

            // Location 2: Position (vec3)
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 3, glow::FLOAT, false, inst_stride, 0);
            gl.vertex_attrib_divisor(2, 1);

            // Location 3: Scale (float)
            gl.enable_vertex_attrib_array(3);
            gl.vertex_attrib_pointer_f32(3, 1, glow::FLOAT, false, inst_stride, 12);
            gl.vertex_attrib_divisor(3, 1);

            // Location 4: Rotation (vec4 - quaternion xyzw)
            gl.enable_vertex_attrib_array(4);
            gl.vertex_attrib_pointer_f32(4, 4, glow::FLOAT, false, inst_stride, 16);
            gl.vertex_attrib_divisor(4, 1);

            gl.bind_vertex_array(None);

            Self {
                gl,
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
