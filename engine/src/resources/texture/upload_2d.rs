use super::*;

impl Texture {
    pub fn from_rgba8(
        gl: Arc<glow::Context>,
        width: u32,
        height: u32,
        data: &[u8],
        filter: TextureFilter,
    ) -> Result<Self, String> {
        Self::from_rgba8_format(gl, width, height, data, filter, TextureFormat::Rgba8Linear)
    }

    pub fn from_rgba8_format(
        gl: Arc<glow::Context>,
        width: u32,
        height: u32,
        data: &[u8],
        filter: TextureFilter,
        format: TextureFormat,
    ) -> Result<Self, String> {
        if !matches!(
            format,
            TextureFormat::Rgba8Linear | TextureFormat::Rgba8Srgb
        ) {
            return Err("RGBA8 uploads require a linear or sRGB RGBA8 format".into());
        }
        let expected = rgba8_byte_len(width, height)?;
        if data.len() != expected {
            return Err("RGBA8 data length must equal width * height * 4".into());
        }

        unsafe {
            let handle = gl.create_texture().map_err(|e| e.to_string())?;
            gl.bind_texture(glow::TEXTURE_2D, Some(handle));

            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                format.internal_format(),
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

    /// Upload a single compressed mip level in BC1, BC4, or BC5 format.
    pub fn from_compressed(
        gl: Arc<glow::Context>,
        width: u32,
        height: u32,
        data: &[u8],
        filter: TextureFilter,
        format: TextureFormat,
    ) -> Result<Self, String> {
        let (block_bytes, extensions): (usize, &[&str]) = match format {
            TextureFormat::BC1Srgb => (
                8,
                &[
                    "GL_EXT_texture_compression_s3tc",
                    "GL_EXT_texture_compression_s3tc_srgb",
                    "GL_EXT_texture_compression_dxt1",
                    "GL_S3_s3tc",
                ],
            ),
            TextureFormat::BC4Linear => (
                8,
                &[
                    "GL_EXT_texture_compression_rgtc",
                    "GL_ARB_texture_compression_rgtc",
                ],
            ),
            TextureFormat::BC5Linear => (
                16,
                &[
                    "GL_EXT_texture_compression_rgtc",
                    "GL_ARB_texture_compression_rgtc",
                ],
            ),
            _ => return Err("Compressed uploads require BC1, BC4, or BC5".into()),
        };
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err("Texture dimensions must be non-zero and fit in GLsizei".into());
        }
        if !extensions
            .iter()
            .any(|extension| gl.supported_extensions().contains(*extension))
        {
            return Err(format!(
                "Texture format {format:?} is unsupported by this GL context"
            ));
        }

        let blocks_x = (width as usize).div_ceil(4);
        let blocks_y = (height as usize).div_ceil(4);
        let expected = blocks_x
            .checked_mul(blocks_y)
            .and_then(|blocks| blocks.checked_mul(block_bytes))
            .ok_or_else(|| "Compressed texture dimensions overflow".to_string())?;
        if data.len() != expected {
            return Err(format!(
                "Compressed data has {} bytes; format {format:?} requires {expected}",
                data.len()
            ));
        }
        let image_size = i32::try_from(data.len())
            .map_err(|_| "Compressed texture data exceeds GLsizei".to_string())?;

        unsafe {
            let handle = gl.create_texture().map_err(|e| e.to_string())?;
            gl.bind_texture(glow::TEXTURE_2D, Some(handle));
            gl.compressed_tex_image_2d(
                glow::TEXTURE_2D,
                0,
                format.internal_format(),
                width as i32,
                height as i32,
                0,
                image_size,
                data,
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
}
