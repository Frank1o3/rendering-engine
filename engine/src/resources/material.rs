use std::collections::HashMap;
use bytemuck::{Pod, Zeroable};
use crate::render::pipeline::PipelineStateId;
use super::shader::ShaderId;
use super::texture::TextureId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct MaterialId(pub u32);

/// Material entry storing shader, pipeline state, and optional texture binding.
#[derive(Debug, Clone)]
pub struct MaterialEntry {
    pub shader_id: ShaderId,
    pub pipeline_id: PipelineStateId,
    pub texture_id: Option<TextureId>,
}

pub struct MaterialManager {
    materials: HashMap<MaterialId, MaterialEntry>,
    next_id: u32,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self {
            materials: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn create_material(
        &mut self,
        shader_id: ShaderId,
        pipeline_id: PipelineStateId,
    ) -> MaterialId {
        self.create_material_with_texture(shader_id, pipeline_id, None)
    }

    pub fn create_material_with_texture(
        &mut self,
        shader_id: ShaderId,
        pipeline_id: PipelineStateId,
        texture_id: Option<TextureId>,
    ) -> MaterialId {
        let id = MaterialId(self.next_id);
        self.next_id += 1;
        self.materials.insert(
            id,
            MaterialEntry {
                shader_id,
                pipeline_id,
                texture_id,
            },
        );
        id
    }

    pub fn get(&self, id: MaterialId) -> Option<&MaterialEntry> {
        self.materials.get(&id)
    }

    pub fn set_texture(&mut self, material_id: MaterialId, texture_id: Option<TextureId>) -> bool {
        if let Some(mat) = self.materials.get_mut(&material_id) {
            mat.texture_id = texture_id;
            true
        } else {
            false
        }
    }
}
