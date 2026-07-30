use crate::render::frame_data::InstanceData;
use crate::render::renderer::MeshId;
use crate::resources::material::MaterialId;
use glam::{Quat, Vec3};
use std::collections::HashMap;

/// Opaque handle to a registered scene object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u32);

/// Determines how aggressively the renderer updates an object's instance data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// Uploaded once. Only re-uploaded when explicitly marked dirty.
    Static,
    /// Always re-uploaded every frame.
    Dynamic,
}

struct SceneObject {
    mesh_id: MeshId,
    material_id: MaterialId,
    kind: ObjectKind,

    position: Vec3,
    rotation: Quat,
    scale: f32,

    dirty: bool,

    cached: InstanceData,
}

impl SceneObject {
    fn recompute_cache(&mut self) {
        self.cached = InstanceData::new(self.position, self.rotation, self.scale);
        self.dirty = false;
    }
}

/// Entry in the sorted collection buffer used during rendering.
#[derive(Clone, Copy)]
pub struct SortedInstance {
    pub material_id: MaterialId,
    pub mesh_id: MeshId,
    pub instance: InstanceData,
}

/// Manages all registered scene objects and their transforms.
pub struct Scene {
    objects: HashMap<ObjectHandle, SceneObject>,
    next_handle: u32,
    scene_dirty: bool,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            objects: HashMap::with_capacity(1024),
            next_handle: 0,
            scene_dirty: false,
        }
    }

    pub fn add_object(
        &mut self,
        mesh_id: MeshId,
        material_id: MaterialId,
        kind: ObjectKind,
    ) -> ObjectHandle {
        let handle = ObjectHandle(self.next_handle);
        self.next_handle += 1;

        let mut obj = SceneObject {
            mesh_id,
            material_id,
            kind,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: 1.0,
            dirty: true,
            cached: InstanceData::IDENTITY,
        };
        obj.recompute_cache();

        self.objects.insert(handle, obj);
        self.scene_dirty = true;
        handle
    }

    pub fn remove_object(&mut self, handle: ObjectHandle) {
        if self.objects.remove(&handle).is_some() {
            self.scene_dirty = true;
        }
    }

    pub fn set_transform(
        &mut self,
        handle: ObjectHandle,
        position: Vec3,
        rotation: Quat,
        scale: f32,
    ) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.position = position;
            obj.rotation = rotation;
            obj.scale = scale;
            obj.dirty = true;
        }
    }

    pub fn set_position(&mut self, handle: ObjectHandle, position: Vec3) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.position = position;
            obj.dirty = true;
        }
    }

    pub fn set_position_rotation(
        &mut self,
        handle: ObjectHandle,
        position: Vec3,
        rotation: Quat,
    ) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.position = position;
            obj.rotation = rotation;
            obj.dirty = true;
        }
    }

    pub fn flush_dirty(&mut self) -> usize {
        let mut recomputed = 0;
        for obj in self.objects.values_mut() {
            let needs_update = obj.dirty || obj.kind == ObjectKind::Dynamic;
            if needs_update {
                obj.recompute_cache();
                recomputed += 1;
            }
        }
        self.scene_dirty = false;
        recomputed
    }

    pub fn collect_sorted_into(&self, out: &mut Vec<SortedInstance>) {
        out.clear();
        out.reserve(self.objects.len());
        for obj in self.objects.values() {
            out.push(SortedInstance {
                material_id: obj.material_id,
                mesh_id: obj.mesh_id,
                instance: obj.cached,
            });
        }
        out.sort_unstable_by_key(|o| (o.material_id, o.mesh_id));
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn is_scene_dirty(&self) -> bool {
        self.scene_dirty
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
