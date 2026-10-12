use super::{Texture, TextureFilter};
use std::collections::HashMap;
use std::sync::Arc;

/// RGBA8 source image placed into a packed texture atlas.
#[derive(Clone, Debug)]
pub struct AtlasImage {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtlasRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Normalized UV bounds in `[u_min, v_min, u_max, v_max]` order.
    pub uv_rect: [f32; 4],
}

/// CPU-packed RGBA8 atlas ready for upload to a `Texture`.
#[derive(Debug)]
pub struct PackedAtlas {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
    pub regions: HashMap<String, AtlasRegion>,
}

impl PackedAtlas {
    pub fn upload(&self, gl: Arc<glow::Context>, filter: TextureFilter) -> Result<Texture, String> {
        Texture::from_rgba8(gl, self.width, self.height, &self.rgba8, filter)
    }
}

/// Pack RGBA8 images into horizontal shelves without scaling or rotation.
pub fn pack_rgba8(
    images: &[AtlasImage],
    max_width: u32,
    max_height: u32,
) -> Result<PackedAtlas, String> {
    if images.is_empty() {
        return Err("Cannot pack an empty texture atlas".into());
    }
    if max_width == 0 || max_height == 0 {
        return Err("Atlas dimensions must be non-zero".into());
    }

    let mut order: Vec<usize> = (0..images.len()).collect();
    order.sort_unstable_by_key(|&index| std::cmp::Reverse(images[index].height));

    let mut x = 0u32;
    let mut y = 0u32;
    let mut shelf_height = 0u32;
    let mut used_width = 0u32;
    let mut used_height = 0u32;
    let mut placements = vec![(0u32, 0u32); images.len()];
    let mut names = HashMap::with_capacity(images.len());

    for index in order {
        let image = &images[index];
        if image.name.is_empty() || names.insert(image.name.as_str(), ()).is_some() {
            return Err(format!(
                "Atlas image name is empty or duplicated: {:?}",
                image.name
            ));
        }
        let expected = (image.width as usize)
            .checked_mul(image.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| format!("Image '{}' dimensions overflow", image.name))?;
        if image.width == 0 || image.height == 0 || image.rgba8.len() != expected {
            return Err(format!(
                "Image '{}' must have non-zero dimensions and exactly width * height * 4 bytes",
                image.name
            ));
        }
        if image.width > max_width || image.height > max_height {
            return Err(format!("Image '{}' exceeds atlas limits", image.name));
        }

        let next_x = x
            .checked_add(image.width)
            .ok_or_else(|| "Atlas width overflow".to_string())?;
        if next_x > max_width {
            x = 0;
            y = y
                .checked_add(shelf_height)
                .ok_or_else(|| "Atlas height overflow".to_string())?;
            shelf_height = 0;
        }
        let next_y = y
            .checked_add(image.height)
            .ok_or_else(|| "Atlas height overflow".to_string())?;
        if next_y > max_height {
            return Err(format!(
                "Atlas does not fit within {max_width}x{max_height}"
            ));
        }

        placements[index] = (x, y);
        x = x
            .checked_add(image.width)
            .ok_or_else(|| "Atlas width overflow".to_string())?;
        shelf_height = shelf_height.max(image.height);
        used_width = used_width.max(x);
        used_height = used_height.max(next_y);
    }

    let byte_len = (used_width as usize)
        .checked_mul(used_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Packed atlas dimensions overflow".to_string())?;
    let mut rgba8 = vec![0; byte_len];
    let mut regions = HashMap::with_capacity(images.len());

    for (image, (x, y)) in images.iter().zip(placements) {
        let row_bytes = image.width as usize * 4;
        for row in 0..image.height as usize {
            let src = row * row_bytes;
            let dst = ((y as usize + row) * used_width as usize + x as usize) * 4;
            rgba8[dst..dst + row_bytes].copy_from_slice(&image.rgba8[src..src + row_bytes]);
        }
        regions.insert(
            image.name.clone(),
            AtlasRegion {
                x,
                y,
                width: image.width,
                height: image.height,
                uv_rect: [
                    x as f32 / used_width as f32,
                    y as f32 / used_height as f32,
                    (x + image.width) as f32 / used_width as f32,
                    (y + image.height) as f32 / used_height as f32,
                ],
            },
        );
    }

    Ok(PackedAtlas {
        width: used_width,
        height: used_height,
        rgba8,
        regions,
    })
}
