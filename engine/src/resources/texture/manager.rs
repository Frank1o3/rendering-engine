use super::{Texture, TextureId};
use std::collections::HashMap;

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

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_manager_default_is_empty() {
        let manager = TextureManager::default();
        assert!(manager.textures.is_empty());
        assert_eq!(manager.next_id, 0);
    }
}
