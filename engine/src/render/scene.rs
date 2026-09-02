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
    cell_size: f32,
    grid: HashMap<(i32, i32, i32), Vec<ObjectHandle>>,
    grid_dirty: bool,

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
            cell_size: 32.0,
            grid: HashMap::new(),
            grid_dirty: false,
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
            self.grid_dirty = true;
            self.order_dirty = true;
        }
    }

    fn mark_moved(&mut self, handle: ObjectHandle) {
        if let Some(obj) = self.objects.get_mut(&handle) {
            obj.dirty = true;
            self.grid_dirty = true;
            if obj.kind != ObjectKind::Dynamic {
                self.moved_static.insert(handle);
            }
        }
    }

    fn cell_for(position: Vec3, cell_size: f32) -> (i32, i32, i32) {
        (
            (position.x / cell_size).floor() as i32,
            (position.y / cell_size).floor() as i32,
            (position.z / cell_size).floor() as i32,
        )
    }

    fn rebuild_grid(&mut self) {
        self.grid.clear();
        for (&handle, obj) in &self.objects {
            let cell = Self::cell_for(obj.position, self.cell_size);
            self.grid.entry(cell).or_default().push(handle);
        }
        self.grid_dirty = false;
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

    pub fn collect_nearby_sorted_into(
        &mut self,
        out: &mut Vec<SortedInstance>,
        camera_position: Vec3,
        radius_cells: i32,
    ) {
        if self.grid_dirty {
            self.rebuild_grid();
        }

        let origin = Self::cell_for(camera_position, self.cell_size);
        let mut seen = HashSet::new();
        out.clear();

        for x in -radius_cells..=radius_cells {
            for y in -radius_cells..=radius_cells {
                for z in -radius_cells..=radius_cells {
                    let key = (origin.0 + x, origin.1 + y, origin.2 + z);
                    let Some(handles) = self.grid.get(&key) else {
                        continue;
                    };

                    for &handle in handles {
                        if !seen.insert(handle) {
                            continue;
                        }

                        if let Some(obj) = self.objects.get(&handle) {
                            out.push(SortedInstance {
                                material_id: obj.material_id,
                                mesh_id: obj.mesh_id,
                                instance: obj.cached,
                            });
                        }
                    }
                }
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

    /// Query the number of objects in a spatial region (in cells) around a position.
    /// Useful for deciding whether to use full or partial visibility.
    pub fn count_nearby_objects(&mut self, camera_position: Vec3, radius_cells: i32) -> usize {
        if self.grid_dirty {
            self.rebuild_grid();
        }

        let origin = Self::cell_for(camera_position, self.cell_size);
        let mut count = 0;
        let mut seen = HashSet::new();

        for x in -radius_cells..=radius_cells {
            for y in -radius_cells..=radius_cells {
                for z in -radius_cells..=radius_cells {
                    let key = (origin.0 + x, origin.1 + y, origin.2 + z);
                    if let Some(handles) = self.grid.get(&key) {
                        for &handle in handles {
                            if seen.insert(handle) {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        count
    }

    /// Get the cell size used for spatial partitioning.
    pub fn get_cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Set the cell size for spatial partitioning. Larger values are faster but
    /// less precise; smaller values are more precise but require more grid cells.
    pub fn set_cell_size(&mut self, cell_size: f32) {
        if (cell_size - self.cell_size).abs() > 0.01 {
            self.cell_size = cell_size.max(1.0);
            self.grid_dirty = true;
        }
    }

    /// Suggest an appropriate visibility radius based on object count and spatial distribution.
    /// Returns a recommended number of cells to include around the camera.
    pub fn suggest_visibility_radius(&mut self, camera_position: Vec3, max_objects: usize) -> i32 {
        // Start with a conservative estimate
        let mut radius = 1i32;

        // Grow radius until we've seen about 80% of max_objects or reach a reasonable max
        while radius < 32 {
            let count = self.count_nearby_objects(camera_position, radius);
            if count as usize >= (max_objects * 8) / 10 {
                break;
            }
            radius += 1;
        }

        radius
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
