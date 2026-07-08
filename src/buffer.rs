use glow::HasContext;
use std::sync::Arc;

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
