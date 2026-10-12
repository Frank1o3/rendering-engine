use super::*;

impl Texture {
    pub fn from_array_rgba8(
        gl: Arc<glow::Context>,
        width: u32,
        height: u32,
        layers: &[&[u8]],
        filter: TextureFilter,
    ) -> Result<Self, String> {
        let layer_bytes = rgba8_byte_len(width, height)?;
        let depth = layers.len() as u32;
        if depth == 0 {
            return Err("Texture array must contain at least one layer".to_string());
        }
        if layers.iter().any(|layer| layer.len() != layer_bytes) {
            return Err("Every texture array layer must be width * height * 4 bytes".into());
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
}
