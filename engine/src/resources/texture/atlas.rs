use super::Texture;

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
