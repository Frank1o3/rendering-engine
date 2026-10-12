use super::*;

impl Renderer {
    // ── Render ───────────────────────────────────────────────────────────────

    pub fn render(&mut self) {
        self.frame_stats = FrameStats::default();
        if self.read_handle.consume(&mut self.last_frame) {
            self.has_frame = true;
        }

        if !self.has_frame {
            unsafe {
                self.gl.depth_mask(true);
                self.gl
                    .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            }
            return;
        }

        let region = (self.frame_index % self.region_fences.len() as u64) as usize;
        self.wait_for_region(region);
        let region_base = region * self.max_objects;
        let region_limit = region_base + self.max_objects;

        self.current_pipeline_id = None;
        self.frame_scratch.clear();
        self.lighting_buffer
            .upload_and_bind(&self.lights, LIGHTS_UBO_BINDING);

        let view = math::camera_to_view_matrix(
            self.last_frame.camera_position,
            self.last_frame.camera_rotation,
        );
        let proj = math::camera_to_projection_matrix(
            self.last_frame.camera_fov,
            self.last_frame.camera_aspect_ratio,
            self.last_frame.camera_near,
            self.last_frame.camera_far,
        );
        let vp = proj * view;
        let inv_vp = vp.inverse();

        self.apply_pass_config(&PassConfig::new(PassType::Skybox));
        self.render_skybox_pass(&inv_vp);

        let frustum = math::extract_frustum_planes(vp);
        self.last_frustum = Some(frustum);

        self.apply_pass_config(&PassConfig::new(PassType::Opaque));
        let mut transform_offset = region_base;
        transform_offset = self.render_world_pass(&vp, &frustum, transform_offset, region_limit);

        self.apply_pass_config(&PassConfig::new(PassType::Transparent));
        transform_offset =
            self.render_transparent_instances(&vp, &frustum, transform_offset, region_limit);

        self.apply_pass_config(&PassConfig::new(PassType::UI));
        self.render_ui_pass(transform_offset, region_limit);

        self.fence_region(region);
        self.frame_index = self.frame_index.wrapping_add(1);
    }

    fn render_skybox_pass(&mut self, inv_vp: &glam::Mat4) {
        if let Some(skybox) = &self.skybox {
            skybox.draw(&self.shaders, inv_vp);
        }
    }

    fn render_ui_pass(&mut self, transform_offset: usize, region_limit: usize) {
        if self.last_frame.ui_commands.is_empty() {
            return;
        }

        let w = self.width as f32;
        let h = self.height as f32;
        let ui_proj = glam::Mat4::from_translation(glam::Vec3::new(-1.0, 1.0, 0.0))
            * glam::Mat4::from_scale(glam::Vec3::new(2.0 / w, -2.0 / h, 1.0));

        self.frame_scratch.sorted_instances.clear();
        for cmd in &self.last_frame.ui_commands {
            self.frame_scratch.sorted_instances.push(SortedInstance {
                material_id: cmd.material_id,
                mesh_id: cmd.mesh_id,
                instance: cmd.to_instance_data(),
            });
        }
        self.frame_scratch
            .sorted_instances
            .sort_unstable_by_key(|o| (o.material_id, o.mesh_id));

        self.render_instances(&ui_proj, transform_offset, region_limit, None);
    }

    pub(super) fn render_instances(
        &mut self,
        vp: &glam::Mat4,
        mut transform_offset: usize,
        region_limit: usize,
        frustum: Option<&[Vec4; 6]>,
    ) -> usize {
        if self.frame_scratch.sorted_instances.is_empty() {
            return transform_offset;
        }

        let start_offset = transform_offset;
        let total = self.frame_scratch.sorted_instances.len();
        self.frame_stats.input_instances += total as u64;

        self.frame_scratch.visible_indices.clear();
        self.frame_scratch
            .visible_indices
            .reserve(total.min(self.max_objects));

        for i in 0..total {
            if transform_offset >= region_limit {
                log::warn!("Max transform capacity ({}) exceeded", self.max_objects);
                break;
            }

            let inst = &self.frame_scratch.sorted_instances[i];

            if let Some(planes) = frustum {
                let mesh = self.meshes.get(&inst.mesh_id).expect("Mesh ID not found");
                let center = glam::Vec3::from(inst.instance.position);
                let radius = mesh.bounding_radius * inst.instance.scale + 0.5;

                if !sphere_inside_frustum(center, radius, planes) {
                    self.frame_stats.frustum_culled_instances += 1;
                    continue;
                }
            }

            self.transform_buffer
                .write_instance(transform_offset, &inst.instance);
            self.frame_scratch.visible_indices.push(i);
            transform_offset += 1;
        }

        let actual = transform_offset - start_offset;
        self.frame_stats.submitted_instances += actual as u64;
        if actual == 0 {
            return transform_offset;
        }

        unsafe {
            self.gl
                .memory_barrier(glow::CLIENT_MAPPED_BUFFER_BARRIER_BIT);
        }

        self.frame_scratch.pending_draws.clear();

        let mut vi = 0;
        while vi < actual {
            let first_si = self.frame_scratch.visible_indices[vi];
            let mat_id = self.frame_scratch.sorted_instances[first_si].material_id;
            let mesh_id = self.frame_scratch.sorted_instances[first_si].mesh_id;

            let mut group_end = vi + 1;
            while group_end < actual {
                let si = self.frame_scratch.visible_indices[group_end];
                let inst = &self.frame_scratch.sorted_instances[si];
                if inst.material_id != mat_id || inst.mesh_id != mesh_id {
                    break;
                }
                group_end += 1;
            }
            let group_size = group_end - vi;

            let material_pipeline_id = self
                .materials
                .get(mat_id)
                .map(|material| material.pipeline_id)
                .expect("Material ID not found");
            let mesh = self.meshes.get(&mesh_id).expect("Mesh ID not found");
            let base_instance = (start_offset + vi) as u32;

            let cmd = DrawElementsIndirectCommand {
                count: mesh.index_count as u32,
                instance_count: group_size as u32,
                first_index: mesh.first_index,
                base_vertex: mesh.base_vertex,
                base_instance,
            };

            self.frame_scratch
                .pending_draws
                .push((material_pipeline_id, mat_id, cmd));
            vi = group_end;
        }

        self.frame_scratch
            .pending_draws
            .sort_by_key(|(pipeline_id, material_id, _)| (pipeline_id.0, material_id.0));

        unsafe {
            self.gl.bind_vertex_array(Some(self.geometry_pool.vao));
        }

        let mut pi = 0;
        while pi < self.frame_scratch.pending_draws.len() {
            let pipeline_id = self.frame_scratch.pending_draws[pi].0;
            let material_id = self.frame_scratch.pending_draws[pi].1;

            let mut run_end = pi + 1;
            while run_end < self.frame_scratch.pending_draws.len()
                && self.frame_scratch.pending_draws[run_end].0 == pipeline_id
                && self.frame_scratch.pending_draws[run_end].1 == material_id
            {
                run_end += 1;
            }

            let pipeline_state = *self
                .pipeline_cache
                .get_by_id(pipeline_id)
                .expect("Pipeline state not found");
            self.apply_pipeline(pipeline_id, &pipeline_state);

            let shader = self
                .shaders
                .get(ShaderId(pipeline_state.shader_id))
                .expect("Shader ID not found");
            shader.set_vp(vp);
            self.bind_material(material_id, shader);

            self.frame_scratch.indirect_cmds.clear();
            self.frame_scratch.indirect_cmds.extend(
                self.frame_scratch.pending_draws[pi..run_end]
                    .iter()
                    .map(|(_, _, cmd)| *cmd),
            );
            let cmd_count = self
                .indirect_buffer
                .upload(&self.frame_scratch.indirect_cmds);
            self.frame_stats.draw_calls += cmd_count as u64;
            self.frame_stats.triangles += self.frame_scratch.indirect_cmds[..cmd_count]
                .iter()
                .map(|cmd| (cmd.count as u64 / 3) * cmd.instance_count as u64)
                .sum::<u64>();
            if self.mdi_strategy == MdiStrategy::MultiCount {
                self.indirect_buffer.upload_count(cmd_count as u32);
            }

            self.indirect_buffer.dispatch(
                self.mdi_strategy,
                glow::UNSIGNED_INT,
                0,
                cmd_count,
                cmd_count as u32,
            );

            pi = run_end;
        }

        unsafe {
            self.gl.bind_vertex_array(None);
        }

        transform_offset
    }

    pub(super) fn apply_pipeline(
        &mut self,
        requested_id: PipelineStateId,
        pipeline: &PipelineState,
    ) {
        if self.current_pipeline_id == Some(requested_id) {
            return;
        }
        self.frame_stats.pipeline_changes += 1;

        let shader = self
            .shaders
            .get(ShaderId(pipeline.shader_id))
            .expect("Shader not found in apply_pipeline");

        unsafe {
            self.gl.use_program(Some(shader.program));

            match pipeline.cull_mode {
                crate::render::pipeline::CullMode::None => self.gl.disable(glow::CULL_FACE),
                crate::render::pipeline::CullMode::Front => {
                    self.gl.enable(glow::CULL_FACE);
                    self.gl.cull_face(glow::FRONT);
                }
                crate::render::pipeline::CullMode::Back => {
                    self.gl.enable(glow::CULL_FACE);
                    self.gl.cull_face(glow::BACK);
                }
            }

            if pipeline.depth_test {
                self.gl.enable(glow::DEPTH_TEST);
                self.gl.depth_func(pipeline.depth_func.to_glow());
            } else {
                self.gl.disable(glow::DEPTH_TEST);
            }

            self.gl.depth_mask(pipeline.depth_write);

            if pipeline.blend_enabled {
                self.gl.enable(glow::BLEND);
                self.gl
                    .blend_func(pipeline.src_factor.to_glow(), pipeline.dst_factor.to_glow());
            } else {
                self.gl.disable(glow::BLEND);
            }
        }

        self.current_pipeline_id = Some(requested_id);
    }

    pub fn upload_shader_vec3(&mut self, shader_id: ShaderId, name: &str, v: glam::Vec3) -> bool {
        if let Some(shader) = self.shaders.get(shader_id) {
            unsafe {
                self.gl.use_program(Some(shader.program));
            }
            shader.set_vec3(name, v)
        } else {
            false
        }
    }

    pub fn upload_shader_f32(&mut self, shader_id: ShaderId, name: &str, v: f32) -> bool {
        if let Some(shader) = self.shaders.get(shader_id) {
            unsafe {
                self.gl.use_program(Some(shader.program));
            }
            shader.set_f32(name, v)
        } else {
            false
        }
    }
}
