// engine/src/render/scene.rs
use crate::render::frame_data::InstanceData;
use crate::render::renderer::MeshId;
use crate::resources::material::MaterialId;
use glam::{Quat, Vec3};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Static,
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

#[derive(Clone, Copy)]
pub struct SortedInstance {
    pub material_id: MaterialId,
    pub mesh_id: MeshId,
    pub instance: InstanceData,
}

pub struct Scene {
    objects: HashMap<ObjectHandle, SceneObject>,
    next_handle: u32,

    /// True when the *set* of objects changed (added/removed) — the only
    /// thing that invalidates `cached_order`. Moving an object does NOT
    /// set this.
    order_dirty: bool,

    /// Handles that must be re-encoded into `SortedInstance` this frame
    /// (all Dynamic objects, plus any Static object explicitly moved via
    /// set_transform/set_position). Cleared every `flush_dirty` call.
    dynamic_handles: HashSet<ObjectHandle>,
    moved_static: HashSet<ObjectHandle>,

    /// Sort order valid whenever `!order_dirty`. Rebuilding this is the
    /// O(n log n) operation we want to avoid doing every frame.
    cached_order: Vec<ObjectHandle>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            objects: HashMap::with_capacity(1024),
            next_handle: 0,
            order_dirty: false,
            dynamic_handles: HashSet::new(),
            moved_static: HashSet::new(),
            cached_order: Vec::new(),
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

        if kind == ObjectKind::Dynamic {
            self.dynamic_handles.insert(handle);
        }

        self.objects.insert(handle, obj);
        self.order_dirty = true;
        handle
    }

    pub fn remove_object(&mut self, handle: ObjectHandle) {
        if self.objects.remove(&handle).is_some() {
            self.dynamic_handles.remove(&handle);
            self.moved_static.remove(&handle);
            self.order_dirty = true;
        }
    }

    fn mark_moved(&mut self, handle: ObjectHandle) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.dirty = true;
            if obj.kind != ObjectKind::Dynamic {
                self.moved_static.insert(handle);
            }
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
        }
        self.mark_moved(handle);
    }

    pub fn set_position(&mut self, handle: ObjectHandle, position: Vec3) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.position = position;
        }
        self.mark_moved(handle);
    }

    pub fn set_position_rotation(&mut self, handle: ObjectHandle, position: Vec3, rotation: Quat) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.position = position;
            obj.rotation = rotation;
        }
        self.mark_moved(handle);
    }

    /// Recomputes cached InstanceData only for objects that can actually
    /// have changed this frame: all Dynamic objects (always, since they
    /// might move any frame) plus any Static object explicitly moved.
    /// This replaces the old full-HashMap scan.
    pub fn flush_dirty(&mut self) -> usize {
        let mut recomputed = 0;

        for &handle in &self.dynamic_handles {
            if let Some(obj) = self.objects.get_mut(&handle) {
                obj.recompute_cache();
                recomputed += 1;
            }
        }
        for handle in self.moved_static.drain() {
            if let Some(obj) = self.objects.get_mut(&handle) {
                obj.recompute_cache();
                recomputed += 1;
            }
        }

        recomputed
    }

    /// Rebuilds `cached_order` only when membership changed; otherwise
    /// reuses the existing order and just re-reads each object's current
    /// (already-recomputed-by-flush_dirty) cached instance data. This is
    /// the fix: an O(n log n) sort every frame becomes an O(n) copy on the
    /// common frame, and a real rebuild only on chunk load/unload.
    pub fn collect_sorted_into(&mut self, out: &mut Vec<SortedInstance>) {
        if self.order_dirty {
            self.cached_order.clear();
            self.cached_order.extend(self.objects.keys().copied());
            let objects = &self.objects;
            self.cached_order.sort_unstable_by_key(|h| {
                let obj = &objects[h];
                (obj.material_id, obj.mesh_id)
            });
            self.order_dirty = false;
        }

        out.clear();
        out.reserve(self.cached_order.len());
        for &handle in &self.cached_order {
            if let Some(obj) = self.objects.get(&handle) {
                out.push(SortedInstance {
                    material_id: obj.material_id,
                    mesh_id: obj.mesh_id,
                    instance: obj.cached,
                });
            }
        }
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn is_scene_dirty(&self) -> bool {
        self.order_dirty
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
