use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct TextureId(pub u32);

/// Texture format with color space awareness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    /// 8-bit RGBA, linear color space.
    Rgba8Linear,
    /// 8-bit RGBA, sRGB color space (for textures loaded from sRGB images).
    Rgba8Srgb,
    /// 16-bit float RGBA, linear.
    Rgba16F,
    /// Depth texture (24 or 32 bits depending on platform).
    Depth32F,
    /// Compressed BC1 (DXT1) - 6:1 compression, sRGB.
    BC1Srgb,
    /// Compressed BC4 - single channel, linear.
    BC4Linear,
    /// Compressed BC5 - normal maps, linear.
    BC5Linear,
}

impl TextureFormat {
    /// Internal GL format for this texture format.
    pub fn internal_format(&self) -> i32 {
        match self {
            TextureFormat::Rgba8Linear => glow::RGBA8 as i32,
            TextureFormat::Rgba8Srgb => glow::SRGB8_ALPHA8 as i32,
            TextureFormat::Rgba16F => glow::RGBA16F as i32,
            TextureFormat::Depth32F => glow::DEPTH_COMPONENT32F as i32,
            TextureFormat::BC1Srgb => glow::COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT as i32,
            TextureFormat::BC4Linear => glow::COMPRESSED_RED_RGTC1 as i32,
            TextureFormat::BC5Linear => glow::COMPRESSED_RG_RGTC2 as i32,
        }
    }

    /// GL pixel format (channels).
    pub fn pixel_format(&self) -> u32 {
        match self {
            TextureFormat::Rgba8Linear | TextureFormat::Rgba8Srgb | TextureFormat::Rgba16F => {
                glow::RGBA
            }
            TextureFormat::Depth32F => glow::DEPTH_COMPONENT,
            TextureFormat::BC1Srgb | TextureFormat::BC4Linear | TextureFormat::BC5Linear => {
                glow::RGBA // Compressed formats don't use this
            }
        }
    }

    /// GL pixel data type.
    pub fn pixel_type(&self) -> u32 {
        match self {
            TextureFormat::Rgba8Linear | TextureFormat::Rgba8Srgb => glow::UNSIGNED_BYTE,
            TextureFormat::Rgba16F => glow::HALF_FLOAT,
            TextureFormat::Depth32F => glow::FLOAT,
            TextureFormat::BC1Srgb | TextureFormat::BC4Linear | TextureFormat::BC5Linear => 0,
        }
    }

    pub fn is_compressed(&self) -> bool {
        matches!(
            self,
            TextureFormat::BC1Srgb | TextureFormat::BC4Linear | TextureFormat::BC5Linear
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilter {
    Nearest,
    Linear,
    NearestMipmapNearest,
    LinearMipmapLinear,
}

impl TextureFilter {
    pub fn to_glow(&self) -> i32 {
        match self {
            TextureFilter::Nearest => glow::NEAREST as i32,
            TextureFilter::Linear => glow::LINEAR as i32,
            TextureFilter::NearestMipmapNearest => glow::NEAREST_MIPMAP_NEAREST as i32,
            TextureFilter::LinearMipmapLinear => glow::LINEAR_MIPMAP_LINEAR as i32,
        }
    }
}

pub struct Texture {
    gl: Arc<glow::Context>,
    pub handle: glow::Texture,
    pub target: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

impl Texture {
    pub fn from_rgba8(
        gl: Arc<glow::Context>,
        width: u32,
        height: u32,
        data: &[u8],
        filter: TextureFilter,
    ) -> Result<Self, String> {
        unsafe {
            let handle = gl.create_texture().map_err(|e| e.to_string())?;
            gl.bind_texture(glow::TEXTURE_2D, Some(handle));

            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );

            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter.to_glow());
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                match filter {
                    TextureFilter::Nearest | TextureFilter::NearestMipmapNearest => {
                        glow::NEAREST as i32
                    }
                    _ => glow::LINEAR as i32,
                },
            );

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            if matches!(
                filter,
                TextureFilter::NearestMipmapNearest | TextureFilter::LinearMipmapLinear
            ) {
                gl.generate_mipmap(glow::TEXTURE_2D);
            }

            gl.bind_texture(glow::TEXTURE_2D, None);

            Ok(Self {
                gl,
                handle,
                target: glow::TEXTURE_2D,
                width,
                height,
                depth: 1,
            })
        }
    }

    pub fn from_array_rgba8(
        gl: Arc<glow::Context>,
        width: u32,
        height: u32,
        layers: &[&[u8]],
        filter: TextureFilter,
    ) -> Result<Self, String> {
        let depth = layers.len() as u32;
        if depth == 0 {
            return Err("Texture array must contain at least one layer".to_string());
        }

        unsafe {
            let handle = gl.create_texture().map_err(|e| e.to_string())?;
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(handle));

            gl.tex_image_3d(
                glow::TEXTURE_2D_ARRAY,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                depth as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );

            for (layer_idx, layer_data) in layers.iter().enumerate() {
                gl.tex_sub_image_3d(
                    glow::TEXTURE_2D_ARRAY,
                    0,
                    0,
                    0,
                    layer_idx as i32,
                    width as i32,
                    height as i32,
                    1,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(layer_data)),
                );
            }

            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MIN_FILTER,
                filter.to_glow(),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MAG_FILTER,
                match filter {
                    TextureFilter::Nearest | TextureFilter::NearestMipmapNearest => {
                        glow::NEAREST as i32
                    }
                    _ => glow::LINEAR as i32,
                },
            );

            if matches!(
                filter,
                TextureFilter::NearestMipmapNearest | TextureFilter::LinearMipmapLinear
            ) {
                gl.generate_mipmap(glow::TEXTURE_2D_ARRAY);
            }

            gl.bind_texture(glow::TEXTURE_2D_ARRAY, None);

            Ok(Self {
                gl,
                handle,
                target: glow::TEXTURE_2D_ARRAY,
                width,
                height,
                depth,
            })
        }
    }

    /// Create a cubemap texture from 6 faces (in order: +X, -X, +Y, -Y, +Z, -Z).
    pub fn from_cubemap_rgba8(
        gl: Arc<glow::Context>,
        size: u32,
        faces: [&[u8]; 6],
        filter: TextureFilter,
    ) -> Result<Self, String> {
        unsafe {
            let handle = gl.create_texture().map_err(|e| e.to_string())?;
            gl.bind_texture(glow::TEXTURE_CUBE_MAP, Some(handle));

            let face_targets = [
                glow::TEXTURE_CUBE_MAP_POSITIVE_X,
                glow::TEXTURE_CUBE_MAP_NEGATIVE_X,
                glow::TEXTURE_CUBE_MAP_POSITIVE_Y,
                glow::TEXTURE_CUBE_MAP_NEGATIVE_Y,
                glow::TEXTURE_CUBE_MAP_POSITIVE_Z,
                glow::TEXTURE_CUBE_MAP_NEGATIVE_Z,
            ];

            for (i, &target) in face_targets.iter().enumerate() {
                gl.tex_image_2d(
                    target,
                    0,
                    glow::RGBA8 as i32,
                    size as i32,
                    size as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(faces[i])),
                );
            }

            gl.tex_parameter_i32(
                glow::TEXTURE_CUBE_MAP,
                glow::TEXTURE_MIN_FILTER,
                filter.to_glow(),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_CUBE_MAP,
                glow::TEXTURE_MAG_FILTER,
                match filter {
                    TextureFilter::Nearest | TextureFilter::NearestMipmapNearest => {
                        glow::NEAREST as i32
                    }
                    _ => glow::LINEAR as i32,
                },
            );

            gl.tex_parameter_i32(
                glow::TEXTURE_CUBE_MAP,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_CUBE_MAP,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_CUBE_MAP,
                glow::TEXTURE_WRAP_R,
                glow::CLAMP_TO_EDGE as i32,
            );

            if matches!(
                filter,
                TextureFilter::NearestMipmapNearest | TextureFilter::LinearMipmapLinear
            ) {
                gl.generate_mipmap(glow::TEXTURE_CUBE_MAP);
            }

            gl.bind_texture(glow::TEXTURE_CUBE_MAP, None);

            Ok(Self {
                gl,
                handle,
                target: glow::TEXTURE_CUBE_MAP,
                width: size,
                height: size,
                depth: 6,
            })
        }
    }

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

pub struct TextureAtlas {
    pub texture: Texture,
    pub tile_size: (u32, u32),
    pub grid: (u32, u32),
}

impl TextureAtlas {
    pub fn new(texture: Texture, tile_size: (u32, u32), grid: (u32, u32)) -> Self {
        Self {
            texture,
            tile_size,
            grid,
        }
    }

    pub fn uv_rect(&self, tile_x: u32, tile_y: u32) -> [f32; 4] {
        let cols = self.grid.0.max(1);
        let rows = self.grid.1.max(1);
        let tile_x = tile_x.min(cols - 1);
        let tile_y = tile_y.min(rows - 1);

        let u_min = tile_x as f32 / cols as f32;
        let v_min = tile_y as f32 / rows as f32;
        let u_max = (tile_x + 1) as f32 / cols as f32;
        let v_max = (tile_y + 1) as f32 / rows as f32;
        [u_min, v_min, u_max, v_max]
    }
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TextureManager {
    textures: HashMap<TextureId, Texture>,
    next_id: u32,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn register(&mut self, texture: Texture) -> TextureId {
        let id = TextureId(self.next_id);
        self.next_id += 1;
        self.textures.insert(id, texture);
        id
    }

    pub fn get(&self, id: TextureId) -> Option<&Texture> {
        self.textures.get(&id)
    }

    pub fn remove(&mut self, id: TextureId) -> Option<Texture> {
        self.textures.remove(&id)
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

    #[test]
    fn texture_manager_default_is_empty() {
        let manager = TextureManager::default();
        assert!(manager.textures.is_empty());
        assert_eq!(manager.next_id, 0);
    }
}
