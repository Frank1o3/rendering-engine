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
