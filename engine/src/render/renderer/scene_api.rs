use super::*;

impl Renderer {
    // ── Scene delegation API ─────────────────────────────────────────────────

    pub fn add_object(
        &mut self,
        mesh_id: MeshId,
        material_id: MaterialId,
        kind: ObjectKind,
    ) -> ObjectHandle {
        self.scene.add_object(mesh_id, material_id, kind)
    }

    pub fn remove_object(&mut self, handle: ObjectHandle) {
        self.scene.remove_object(handle);
    }

    pub fn set_transform(
        &mut self,
        handle: ObjectHandle,
        pos: glam::Vec3,
        rot: glam::Quat,
        scale: f32,
    ) {
        self.scene.set_transform(handle, pos, rot, scale);
    }

    pub fn set_position(&mut self, handle: ObjectHandle, pos: glam::Vec3) {
        self.scene.set_position(handle, pos);
    }

    pub fn set_position_rotation(
        &mut self,
        handle: ObjectHandle,
        pos: glam::Vec3,
        rot: glam::Quat,
    ) {
        self.scene.set_position_rotation(handle, pos, rot);
    }

    pub fn set_mdi_strategy(&mut self, strategy: MdiStrategy) {
        self.mdi_strategy = strategy;
    }

    pub fn set_visibility_radius_cells(&mut self, radius: i32) {
        self.visibility_radius_cells = radius.max(0);
    }
}
