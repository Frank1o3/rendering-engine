use glow::HasContext;
use std::sync::Arc;

/// A GPU buffer that is persistently mapped to CPU memory.
/// Eliminates per-frame allocation and mapping overhead.
pub struct PersistentMappedBuffer {
    gl: Arc<glow::Context>,
    pub handle: glow::Buffer,
    ptr: *mut u8,
    #[allow(dead_code)]
    size: usize,
}

impl PersistentMappedBuffer {
    pub fn new(gl: Arc<glow::Context>, size: usize) -> Self {
        unsafe {
            let handle = gl
                .create_buffer()
                .expect("Failed to create persistent buffer");
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(handle));

            // Allocate immutable storage with persistent mapping flags
            let flags = glow::MAP_WRITE_BIT | glow::MAP_PERSISTENT_BIT | glow::MAP_COHERENT_BIT;
            gl.buffer_storage(glow::ARRAY_BUFFER, size as i32, None, flags);

            // Map the buffer into CPU address space
            let ptr = gl.map_buffer_range(glow::ARRAY_BUFFER, 0, size as i32, flags);
            if ptr.is_null() {
                panic!("Failed to map persistent buffer");
            }

            Self {
                gl,
                handle,
                ptr,
                size,
            }
        }
    }

    /// Writes a Mat4 directly to the mapped memory at the given index.
    /// Zero allocation, zero driver validation.
    pub fn write_mat4(&self, index: usize, mat: &glam::Mat4) {
        unsafe {
            let dst = (self.ptr as *mut glam::Mat4).add(index);
            *dst = *mat;
        }
    }
}

impl Drop for PersistentMappedBuffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.unmap_buffer(glow::ARRAY_BUFFER);
            self.gl.delete_buffer(self.handle);
        }
    }
}

pub struct GpuBuffer {
    gl: Arc<glow::Context>,
    pub handle: glow::Buffer,
    pub target: u32,
}

impl GpuBuffer {
    pub fn new(gl: Arc<glow::Context>, target: u32) -> Self {
        unsafe {
            let handle = gl.create_buffer().expect("Failed to create buffer");
            Self {
                gl: gl,
                handle,
                target,
            }
        }
    }

    pub fn upload_data<T: bytemuck::Pod>(&self, data: &[T], usage: u32) {
        unsafe {
            self.gl.bind_buffer(self.target, Some(self.handle));
            self.gl
                .buffer_data_u8_slice(self.target, bytemuck::cast_slice(data), usage);
        }
    }
}

impl Drop for GpuBuffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.handle);
        }
    }
}
