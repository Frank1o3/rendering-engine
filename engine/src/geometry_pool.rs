use crate::free_list::{Allocation, FreeListAllocator};
use crate::mesh::{MeshData, Vertex};
use glow::HasContext;
use std::sync::Arc;

pub const MAX_POOL_VERTICES: usize = 1_000_000;
pub const MAX_POOL_INDICES: usize = 3_000_000;

/// Where a mesh's data lives within the shared pool, plus the allocation
/// handles needed to free it later. The alloc fields are crate-private —
/// external callers only see the geometry offsets they need for draw calls.
#[derive(Clone, Copy, Debug)]
pub struct MeshRange {
    pub base_vertex: i32,
    pub first_index: u32,
    pub index_count: i32,
    pub(crate) vertex_alloc: Allocation,
    pub(crate) index_alloc: Allocation,
}

pub struct GeometryPool {
    gl: Arc<glow::Context>,
    pub vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    vertex_alloc: FreeListAllocator,
    index_alloc: FreeListAllocator,
}

impl GeometryPool {
    pub fn new(gl: Arc<glow::Context>, transform_buffer: glow::Buffer) -> Self {
        unsafe {
            let vao = gl.create_vertex_array().expect("Failed to create pool VAO");
            let vbo = gl.create_buffer().expect("Failed to create pool VBO");
            let ebo = gl.create_buffer().expect("Failed to create pool EBO");

            gl.bind_vertex_array(Some(vao));

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

            let stride = std::mem::size_of::<Vertex>() as i32;

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::BYTE, true, stride, 12);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 4, glow::UNSIGNED_BYTE, true, stride, 16);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(transform_buffer));
            let inst_stride: i32 = 32;

            gl.enable_vertex_attrib_array(3);
            gl.vertex_attrib_pointer_f32(3, 3, glow::FLOAT, false, inst_stride, 0);
            gl.vertex_attrib_divisor(3, 1);
            gl.enable_vertex_attrib_array(4);
            gl.vertex_attrib_pointer_f32(4, 1, glow::FLOAT, false, inst_stride, 12);
            gl.vertex_attrib_divisor(4, 1);
            gl.enable_vertex_attrib_array(5);
            gl.vertex_attrib_pointer_f32(5, 4, glow::FLOAT, false, inst_stride, 16);
            gl.vertex_attrib_divisor(5, 1);

            gl.bind_vertex_array(None);

            Self {
                gl,
                vao,
                vbo,
                ebo,
                vertex_alloc: FreeListAllocator::new(MAX_POOL_VERTICES),
                index_alloc: FreeListAllocator::new(MAX_POOL_INDICES),
            }
        }
    }

    /// Uploads a mesh into a free region of the pool.
    ///
    /// Returns `None` if either the vertex or index pool has no single free
    /// block large enough — this replaces the old behavior of panicking via
    /// `assert!` on capacity overflow. A chunk-streaming caller needs to be
    /// able to handle "pool full" as a normal, recoverable condition (e.g.
    /// defer the upload a frame, or evict the farthest chunk first) rather
    /// than crashing the render thread.
    pub fn upload(&mut self, data: &MeshData) -> Option<MeshRange> {
        let v_alloc = self.vertex_alloc.alloc(data.vertices.len())?;
        let i_alloc = match self.index_alloc.alloc(data.indices.len()) {
            Some(a) => a,
            None => {
                // Roll back the vertex allocation so a failed upload doesn't
                // leak pool space.
                self.vertex_alloc.free(v_alloc);
                return None;
            }
        };

        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_sub_data_u8_slice(
                glow::ARRAY_BUFFER,
                (v_alloc.offset * std::mem::size_of::<Vertex>()) as i32,
                bytemuck::cast_slice(&data.vertices),
            );

            self.gl
                .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            self.gl.buffer_sub_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                (i_alloc.offset * std::mem::size_of::<u32>()) as i32,
                bytemuck::cast_slice(&data.indices),
            );
        }

        Some(MeshRange {
            base_vertex: v_alloc.offset as i32,
            first_index: i_alloc.offset as u32,
            index_count: i_alloc.len as i32,
            vertex_alloc: v_alloc,
            index_alloc: i_alloc,
        })
    }

    /// Returns a mesh's vertex/index regions to the pool for reuse. Does
    /// NOT zero the underlying GPU memory — stale bytes are harmless since
    /// nothing will reference this range via any live `MeshRange`/draw
    /// command once the caller has dropped it (see `Renderer::unload_mesh`,
    /// which removes the mesh from the lookup table in the same call).
    pub fn free(&mut self, range: MeshRange) {
        self.vertex_alloc.free(range.vertex_alloc);
        self.index_alloc.free(range.index_alloc);
    }

    pub fn free_vertex_space(&self) -> usize {
        self.vertex_alloc.free_space()
    }
    pub fn free_index_space(&self) -> usize {
        self.index_alloc.free_space()
    }
    pub fn largest_free_vertex_block(&self) -> usize {
        self.vertex_alloc.largest_free_block()
    }
    pub fn largest_free_index_block(&self) -> usize {
        self.index_alloc.largest_free_block()
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
