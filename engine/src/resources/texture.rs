use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct TextureId(pub u32);

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
        let (cols, rows) = (self.grid.0 as f32, self.grid.1 as f32);
        let u_min = tile_x as f32 / cols;
        let v_min = tile_y as f32 / rows;
        let u_max = (tile_x + 1) as f32 / cols;
        let v_max = (tile_y + 1) as f32 / rows;
        [u_min, v_min, u_max, v_max]
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
