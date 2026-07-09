// src/renderer/scene.rs
//
// Scene object registry with dirty transform tracking and static/dynamic separation.
//
// Static objects  — uploaded once, only re-uploaded when their transform changes (dirty flag).
// Dynamic objects — always considered dirty, re-uploaded every frame.
//
// This avoids recomputing transforms for the vast majority of a scene
// (terrain, buildings, trees) that never moves.

use crate::engine::{MaterialId, MeshId};
use crate::frame_data::InstanceData;
use glam::{Quat, Vec3};
use std::collections::HashMap;

/// Opaque handle to a registered scene object.
/// Returned by `Scene::add_object`, used to update or remove objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u32);

/// Determines how aggressively the renderer updates an object's instance data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// Uploaded once. Only re-uploaded when explicitly marked dirty via `set_transform`.
    /// Use for terrain, buildings, props — anything that rarely or never moves.
    Static,
    /// Always re-uploaded every frame. Use for players, mobs, particles, projectiles.
    Dynamic,
}

/// Internal representation of a registered scene object.
struct SceneObject {
    mesh_id: MeshId,
    material_id: MaterialId,
    kind: ObjectKind,

    // Transform components
    position: Vec3,
    rotation: Quat,
    scale: f32,

    // Dirty tracking
    dirty: bool,

    // Cached GPU-ready instance data (recomputed only when dirty)
    cached: InstanceData,
}

impl SceneObject {
    fn recompute_cache(&mut self) {
        self.cached = InstanceData::new(self.position, self.rotation, self.scale);
        self.dirty = false;
    }
}

/// Entry in the sorted collection buffer used during rendering.
/// Flattened from the Scene's HashMap for cache-friendly iteration.
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
    /// True if any object was added, removed, or had its transform changed.
    /// The renderer checks this to decide whether to rebuild MDI commands.
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

    /// Register a new object in the scene.
    /// Returns a handle for future updates.
    /// The object starts at the origin with identity rotation and unit scale.
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

    /// Remove an object from the scene.
    pub fn remove_object(&mut self, handle: ObjectHandle) {
        if self.objects.remove(&handle).is_some() {
            self.scene_dirty = true;
        }
    }

    /// Update the full transform of an object. Marks it dirty.
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

    /// Fast-path: update only the position. Marks it dirty.
    /// Common for objects that translate but don't rotate or scale.
    pub fn set_position(&mut self, handle: ObjectHandle, position: Vec3) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.position = position;
            obj.dirty = true;
        }
    }

    /// Fast-path: update position and rotation. Marks it dirty.
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

    /// Recompute cached InstanceData for all dirty objects.
    /// After this call, all dirty flags are cleared.
    ///
    /// Returns the number of objects that were actually recomputed,
    /// useful for diagnostics (should be 0 for fully static scenes after first frame).
    pub fn flush_dirty(&mut self) -> usize {
        let mut recomputed = 0;
        for obj in self.objects.values_mut() {
            // Dynamic objects are always dirty
            let needs_update = obj.dirty || obj.kind == ObjectKind::Dynamic;
            if needs_update {
                obj.recompute_cache();
                recomputed += 1;
            }
        }
        self.scene_dirty = false;
        recomputed
    }

    /// Collect all objects into a flat, sorted buffer for rendering.
    /// Sorts by `(material_id, mesh_id)` for optimal batching.
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

    /// Number of registered objects.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether the scene has any registered objects.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Whether any structural change (add/remove) happened since last flush.
    pub fn is_scene_dirty(&self) -> bool {
        self.scene_dirty
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
