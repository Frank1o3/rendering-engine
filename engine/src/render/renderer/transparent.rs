use super::*;

impl Renderer {
    /// Draw blended objects individually in back-to-front order. Grouping
    /// these by pipeline or mesh would change their compositing order.
    pub(super) fn render_transparent_instances(
        &mut self,
        vp: &glam::Mat4,
        frustum: &[Vec4; 6],
        mut transform_offset: usize,
        region_limit: usize,
    ) -> usize {
        if self.frame_scratch.transparent_instances.is_empty() {
            return transform_offset;
        }

        self.frame_stats.input_instances += self.frame_scratch.transparent_instances.len() as u64;
        unsafe { self.gl.bind_vertex_array(Some(self.geometry_pool.vao)) };
        let start_offset = transform_offset;
        self.frame_scratch.visible_indices.clear();
        for index in 0..self.frame_scratch.transparent_instances.len() {
            let sorted = self.frame_scratch.transparent_instances[index].1;
            if transform_offset >= region_limit {
                log::warn!("Max transform capacity ({}) exceeded", self.max_objects);
                break;
            }
            let bounding_radius = self
                .meshes
                .get(&sorted.mesh_id)
                .map(|mesh| mesh.bounding_radius)
                .expect("Mesh ID not found");
            let center = glam::Vec3::from(sorted.instance.position);
            let radius = bounding_radius * sorted.instance.scale + 0.5;
            if !sphere_inside_frustum(center, radius, frustum) {
                self.frame_stats.frustum_culled_instances += 1;
                continue;
            }

            self.transform_buffer
                .write_instance(transform_offset, &sorted.instance);
            self.frame_scratch.visible_indices.push(index);
            transform_offset += 1;
        }

        if transform_offset == start_offset {
            unsafe { self.gl.bind_vertex_array(None) };
            return transform_offset;
        }
        unsafe {
            self.gl
                .memory_barrier(glow::CLIENT_MAPPED_BUFFER_BARRIER_BIT);
        }
        self.frame_stats.submitted_instances += (transform_offset - start_offset) as u64;

        for draw_index in 0..self.frame_scratch.visible_indices.len() {
            let index = self.frame_scratch.visible_indices[draw_index];
            let sorted = self.frame_scratch.transparent_instances[index].1;
            let (index_count, first_index, base_vertex) = self
                .meshes
                .get(&sorted.mesh_id)
                .map(|mesh| (mesh.index_count, mesh.first_index, mesh.base_vertex))
                .expect("Mesh ID not found");

            let pipeline_id = self
                .materials
                .get(sorted.material_id)
                .expect("Material ID not found")
                .pipeline_id;
            let pipeline = *self
                .pipeline_cache
                .get_by_id(pipeline_id)
                .expect("Pipeline state not found");
            self.apply_pipeline(pipeline_id, &pipeline);
            unsafe { self.gl.depth_mask(false) };
            let shader = self
                .shaders
                .get(ShaderId(pipeline.shader_id))
                .expect("Shader ID not found");
            shader.set_vp(vp);
            self.bind_material(sorted.material_id, shader);

            let command = DrawElementsIndirectCommand {
                count: index_count as u32,
                instance_count: 1,
                first_index,
                base_vertex,
                base_instance: (start_offset + draw_index) as u32,
            };
            let command_count = self.indirect_buffer.upload(std::slice::from_ref(&command));
            self.frame_stats.draw_calls += command_count as u64;
            self.frame_stats.triangles +=
                (command.count as u64 / 3) * command.instance_count as u64;
            if self.mdi_strategy == MdiStrategy::MultiCount {
                self.indirect_buffer.upload_count(command_count as u32);
            }
            self.indirect_buffer.dispatch(
                self.mdi_strategy,
                glow::UNSIGNED_INT,
                0,
                command_count,
                command_count as u32,
            );
        }
        unsafe { self.gl.bind_vertex_array(None) };
        transform_offset
    }
}
