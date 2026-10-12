use super::*;

impl Texture {
    /// Create a cubemap from six RGBA8 faces in +X, -X, +Y, -Y, +Z, -Z order.
    pub fn from_cubemap_rgba8(
        gl: Arc<glow::Context>,
        size: u32,
        faces: [&[u8]; 6],
        filter: TextureFilter,
    ) -> Result<Self, String> {
        let face_bytes = rgba8_byte_len(size, size)?;
        if faces.iter().any(|face| face.len() != face_bytes) {
            return Err("Every cubemap face must be size * size * 4 bytes".into());
        }
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
}
