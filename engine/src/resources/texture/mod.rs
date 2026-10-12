use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::sync::Arc;

mod atlas;
mod format;
mod manager;
mod packer;
mod upload_2d;
mod upload_array;
mod upload_cubemap;
pub use atlas::TextureAtlas;
pub use format::{TextureFilter, TextureFormat};
pub use manager::TextureManager;
pub use packer::{AtlasImage, AtlasRegion, PackedAtlas, pack_rgba8};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct TextureId(pub u32);

pub struct Texture {
    gl: Arc<glow::Context>,
    pub handle: glow::Texture,
    pub target: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

fn rgba8_byte_len(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err("Texture dimensions must be non-zero and fit in GLsizei".into());
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Texture dimensions overflow".to_string())
}

impl Texture {
    pub fn bind(&self, slot: u32) {
        unsafe {
            self.gl.active_texture(glow::TEXTURE0 + slot);
            self.gl.bind_texture(self.target, Some(self.handle));
        }
    }

    pub fn unbind(&self, slot: u32) {
        unsafe {
            self.gl.active_texture(glow::TEXTURE0 + slot);
            self.gl.bind_texture(self.target, None);
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_texture(self.handle);
        }
    }
}

#[cfg(test)]
#[allow(invalid_value)]
mod tests {
    use super::*;

    #[test]
    fn atlas_uv_rect_clamps_to_valid_tile_range() {
        let atlas = std::mem::ManuallyDrop::new(TextureAtlas::new(
            unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
            (16, 16),
            (2, 2),
        ));

        assert_eq!(atlas.uv_rect(99, 99), [0.5, 0.5, 1.0, 1.0]);
        assert_eq!(atlas.uv_rect(0, 0), [0.0, 0.0, 0.5, 0.5]);
    }
}
