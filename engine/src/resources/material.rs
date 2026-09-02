use super::shader::ShaderId;
use super::texture::TextureId;
use crate::render::pipeline::PipelineStateId;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct MaterialId(pub u32);

/// Extended material properties for physically-based rendering workflows.
#[derive(Debug, Clone)]
pub struct MaterialProperties {
    /// Base color/albedo
    pub albedo: [f32; 3],
    /// Metallic factor (0 = dielectric, 1 = metal)
    pub metallic: f32,
    /// Roughness factor (0 = smooth, 1 = rough)
    pub roughness: f32,
    /// Ambient occlusion factor
    pub ao: f32,
    /// Normal map strength
    pub normal_strength: f32,
    /// Emission factor
    pub emissive: [f32; 3],
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self {
            albedo: [1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            normal_strength: 1.0,
            emissive: [0.0, 0.0, 0.0],
        }
    }
}

impl MaterialProperties {
    pub fn new(albedo: [f32; 3]) -> Self {
        Self {
            albedo,
            ..Default::default()
        }
    }
}

/// Material entry storing shader, pipeline state, and texture bindings.
#[derive(Debug, Clone)]
pub struct MaterialEntry {
    pub shader_id: ShaderId,
    pub pipeline_id: PipelineStateId,
    /// Base color/albedo texture
    pub texture_id: Option<TextureId>,
    /// Normal map texture
    pub normal_texture_id: Option<TextureId>,
    /// Metallic/roughness texture (metallic in R, roughness in G)
    pub metallic_roughness_texture_id: Option<TextureId>,
    /// Ambient occlusion texture
    pub ao_texture_id: Option<TextureId>,
    /// Emissive texture
    pub emissive_texture_id: Option<TextureId>,
    /// Material properties
    pub properties: MaterialProperties,
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
        self.create_material_full(
            shader_id,
            pipeline_id,
            None,
            None,
            None,
            None,
            MaterialProperties::default(),
        )
    }

    pub fn create_material_with_texture(
        &mut self,
        shader_id: ShaderId,
        pipeline_id: PipelineStateId,
        texture_id: Option<TextureId>,
    ) -> MaterialId {
        self.create_material_full(
            shader_id,
            pipeline_id,
            texture_id,
            None,
            None,
            None,
            MaterialProperties::default(),
        )
    }

    pub fn create_material_full(
        &mut self,
        shader_id: ShaderId,
        pipeline_id: PipelineStateId,
        texture_id: Option<TextureId>,
        normal_texture_id: Option<TextureId>,
        metallic_roughness_texture_id: Option<TextureId>,
        ao_texture_id: Option<TextureId>,
        properties: MaterialProperties,
    ) -> MaterialId {
        let id = MaterialId(self.next_id);
        self.next_id += 1;
        self.materials.insert(
            id,
            MaterialEntry {
                shader_id,
                pipeline_id,
                texture_id,
                normal_texture_id,
                metallic_roughness_texture_id,
                ao_texture_id,
                emissive_texture_id: None,
                properties,
            },
        );
        id
    }

    pub fn get(&self, id: MaterialId) -> Option<&MaterialEntry> {
        self.materials.get(&id)
    }

    pub fn get_mut(&mut self, id: MaterialId) -> Option<&mut MaterialEntry> {
        self.materials.get_mut(&id)
    }

    pub fn set_texture(&mut self, material_id: MaterialId, texture_id: Option<TextureId>) -> bool {
        if let Some(mat) = self.materials.get_mut(&material_id) {
            mat.texture_id = texture_id;
            true
        } else {
            false
        }
    }

    pub fn set_normal_texture(
        &mut self,
        material_id: MaterialId,
        texture_id: Option<TextureId>,
    ) -> bool {
        if let Some(mat) = self.materials.get_mut(&material_id) {
            mat.normal_texture_id = texture_id;
            true
        } else {
            false
        }
    }

    pub fn set_metallic_roughness_texture(
        &mut self,
        material_id: MaterialId,
        texture_id: Option<TextureId>,
    ) -> bool {
        if let Some(mat) = self.materials.get_mut(&material_id) {
            mat.metallic_roughness_texture_id = texture_id;
            true
        } else {
            false
        }
    }

    pub fn set_ao_texture(
        &mut self,
        material_id: MaterialId,
        texture_id: Option<TextureId>,
    ) -> bool {
        if let Some(mat) = self.materials.get_mut(&material_id) {
            mat.ao_texture_id = texture_id;
            true
        } else {
            false
        }
    }

    pub fn set_emissive_texture(
        &mut self,
        material_id: MaterialId,
        texture_id: Option<TextureId>,
    ) -> bool {
        if let Some(mat) = self.materials.get_mut(&material_id) {
            mat.emissive_texture_id = texture_id;
            true
        } else {
            false
        }
    }

    pub fn set_properties(
        &mut self,
        material_id: MaterialId,
        properties: MaterialProperties,
    ) -> bool {
        if let Some(mat) = self.materials.get_mut(&material_id) {
            mat.properties = properties;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_texture_setters_update_fields() {
        let mut manager = MaterialManager::new();
        let material = manager.create_material(ShaderId(1), PipelineStateId(7));

        let normal = TextureId(10);
        let mr = TextureId(11);
        let ao = TextureId(12);
        let emissive = TextureId(13);

        assert!(manager.set_normal_texture(material, Some(normal)));
        assert!(manager.set_metallic_roughness_texture(material, Some(mr)));
        assert!(manager.set_ao_texture(material, Some(ao)));
        assert!(manager.set_emissive_texture(material, Some(emissive)));

        let entry = manager.get(material).unwrap();
        assert_eq!(entry.normal_texture_id, Some(normal));
        assert_eq!(entry.metallic_roughness_texture_id, Some(mr));
        assert_eq!(entry.ao_texture_id, Some(ao));
        assert_eq!(entry.emissive_texture_id, Some(emissive));
    }
}

impl Default for MaterialManager {
    fn default() -> Self {
        Self::new()
    }
}
