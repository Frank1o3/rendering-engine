use super::*;

impl Renderer {
    pub(super) fn render_world_pass(
        &mut self,
        vp: &glam::Mat4,
        frustum: &[Vec4; 6],
        transform_offset: usize,
        region_limit: usize,
    ) -> usize {
        self.scene.flush_dirty();
        self.frame_scratch.sorted_instances.clear();
        self.scene.collect_nearby_sorted_into(
            &mut self.frame_scratch.sorted_instances,
            self.last_frame.camera_position,
            self.visibility_radius_cells,
        );

        let dynamic_commands_exist = !self.last_frame.commands.is_empty();
        for cmd in &self.last_frame.commands {
            self.frame_scratch.sorted_instances.push(SortedInstance {
                material_id: cmd.material_id,
                mesh_id: cmd.mesh_id,
                instance: cmd.to_instance_data(),
            });
        }
        if dynamic_commands_exist {
            self.frame_scratch
                .sorted_instances
                .sort_unstable_by_key(|o| (o.material_id, o.mesh_id));
        }

        let camera_position = self.last_frame.camera_position;
        let camera_forward = self.last_frame.camera_rotation * glam::Vec3::NEG_Z;
        let materials = &self.materials;
        let pipeline_cache = &self.pipeline_cache;
        let transparent = &mut self.frame_scratch.transparent_instances;
        transparent.clear();
        self.frame_scratch.sorted_instances.retain(|instance| {
            let is_transparent = materials
                .get(instance.material_id)
                .and_then(|material| pipeline_cache.get_by_id(material.pipeline_id))
                .is_some_and(|pipeline| pipeline.blend_enabled);
            if is_transparent {
                let position = glam::Vec3::from(instance.instance.position);
                let depth = (position - camera_position).dot(camera_forward);
                transparent.push((depth, *instance));
                false
            } else {
                true
            }
        });
        transparent.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));

        self.render_instances(vp, transform_offset, region_limit, Some(frustum))
    }
}
