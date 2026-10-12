use crate::render::lighting::{LightManager, LightUniformBlock};
use glow::HasContext;
use std::sync::Arc;

/// GPU storage for the renderer's fixed-size light uniform block.
pub struct LightingBuffer {
    gl: Arc<glow::Context>,
    handle: glow::Buffer,
    staging: LightUniformBlock,
}

impl LightingBuffer {
    pub fn new(gl: Arc<glow::Context>) -> Self {
        let handle = unsafe {
            let buffer = gl
                .create_buffer()
                .expect("Failed to create lighting uniform buffer");
            gl.bind_buffer(glow::UNIFORM_BUFFER, Some(buffer));
            gl.buffer_data_size(
                glow::UNIFORM_BUFFER,
                std::mem::size_of::<LightUniformBlock>() as i32,
                glow::DYNAMIC_DRAW,
            );
            gl.bind_buffer(glow::UNIFORM_BUFFER, None);
            buffer
        };

        Self {
            gl,
            handle,
            staging: LightUniformBlock::default(),
        }
    }

    pub fn upload_and_bind(&mut self, lights: &LightManager, binding: u32) {
        lights.write_uniform_block(&mut self.staging);
        unsafe {
            self.gl.bind_buffer(glow::UNIFORM_BUFFER, Some(self.handle));
            self.gl.buffer_sub_data_u8_slice(
                glow::UNIFORM_BUFFER,
                0,
                bytemuck::bytes_of(&self.staging),
            );
            self.gl
                .bind_buffer_base(glow::UNIFORM_BUFFER, binding, Some(self.handle));
            self.gl.bind_buffer(glow::UNIFORM_BUFFER, None);
        }
    }
}

impl Drop for LightingBuffer {
    fn drop(&mut self) {
        unsafe { self.gl.delete_buffer(self.handle) }
    }
}
