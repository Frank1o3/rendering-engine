use super::*;

impl Renderer {
    // ── Shader & Material facade API ─────────────────────────────────────────

    pub fn load_shader(&mut self, vs: &str, gs: Option<&str>, fs: &str) -> ShaderId {
        self.shaders.load_shader(self.gl.clone(), vs, gs, fs)
    }

    pub fn load_shader_from_files(
        &mut self,
        vs_path: &Path,
        gs_path: Option<&Path>,
        fs_path: &Path,
    ) -> Result<ShaderId, String> {
        self.shaders
            .load_shader_from_files(self.gl.clone(), vs_path, gs_path, fs_path)
    }

    pub fn reload_shader_from_files(
        &mut self,
        shader_id: ShaderId,
        vs_path: &Path,
        gs_path: Option<&Path>,
        fs_path: &Path,
    ) -> Result<(), String> {
        self.shaders.reload_shader_from_files(
            shader_id,
            self.gl.clone(),
            vs_path,
            gs_path,
            fs_path,
        )?;
        self.current_pipeline_id = None;
        Ok(())
    }

    pub fn load_shaders_from_dir(
        &mut self,
        dir: &Path,
    ) -> Result<HashMap<String, ShaderId>, String> {
        self.shaders.load_shaders_from_dir(self.gl.clone(), dir)
    }

    pub fn load_shaders_from_include_dir(
        &mut self,
        dir: &include_dir::Dir,
    ) -> Result<HashMap<String, ShaderId>, String> {
        self.shaders
            .load_shaders_from_include_dir(self.gl.clone(), dir)
    }

    pub fn create_material(
        &mut self,
        shader_id: ShaderId,
        pipeline_state: PipelineState,
    ) -> MaterialId {
        let pipeline_id = self.pipeline_cache.register(pipeline_state);
        self.materials.create_material(shader_id, pipeline_id)
    }

    pub fn create_material_with_texture(
        &mut self,
        shader_id: ShaderId,
        pipeline_state: PipelineState,
        texture_id: Option<TextureId>,
    ) -> MaterialId {
        let pipeline_id = self.pipeline_cache.register(pipeline_state);
        self.materials
            .create_material_with_texture(shader_id, pipeline_id, texture_id)
    }

    /// Bind a shader's `EngineLights` block to the renderer-managed light UBO.
    /// The shader block layout must match `lighting::LIGHTS_BLOCK_GLSL`.
    pub fn bind_light_block(&self, shader_id: ShaderId) -> bool {
        self.shaders
            .get(shader_id)
            .is_some_and(|shader| shader.bind_uniform_block("EngineLights", LIGHTS_UBO_BINDING))
    }

    pub(super) fn bind_material(
        &self,
        material_id: MaterialId,
        shader: &crate::resources::shader::ShaderProgram,
    ) {
        let material = self
            .materials
            .get(material_id)
            .expect("Material ID not found");
        let texture_ids = [
            material.texture_id,
            material.normal_texture_id,
            material.metallic_roughness_texture_id,
            material.ao_texture_id,
            material.emissive_texture_id,
        ];
        let properties = (
            material.properties.albedo,
            material.properties.metallic,
            material.properties.roughness,
            material.properties.ao,
            material.properties.normal_strength,
            material.properties.emissive,
        );

        shader.set_vec3("uMaterialAlbedo", glam::Vec3::from_array(properties.0));
        shader.set_f32("uMetallicFactor", properties.1);
        shader.set_f32("uRoughnessFactor", properties.2);
        shader.set_f32("uAmbientOcclusion", properties.3);
        shader.set_f32("uNormalStrength", properties.4);
        shader.set_vec3("uEmissiveFactor", glam::Vec3::from_array(properties.5));

        let bindings = [
            ("uAlbedoTexture", "uHasAlbedoTexture"),
            ("uNormalTexture", "uHasNormalTexture"),
            ("uMetallicRoughnessTexture", "uHasMetallicRoughnessTexture"),
            ("uAOTexture", "uHasAOTexture"),
            ("uEmissiveTexture", "uHasEmissiveTexture"),
        ];
        for (slot, (texture_id, (sampler, has_texture))) in
            texture_ids.into_iter().zip(bindings).enumerate()
        {
            shader.set_i32(sampler, slot as i32);
            let texture = texture_id.and_then(|id| self.textures.get(id));
            shader.set_i32(has_texture, i32::from(texture.is_some()));
            if let Some(texture) = texture {
                texture.bind(slot as u32);
            } else {
                unsafe {
                    self.gl.active_texture(glow::TEXTURE0 + slot as u32);
                    self.gl.bind_texture(glow::TEXTURE_2D, None);
                }
            }
        }
    }

    // ── Mesh & Skybox API ────────────────────────────────────────────────────

    pub fn load_mesh(&mut self, mut data: MeshData) -> Option<MeshId> {
        data.fix_winding();
        self.insert_mesh(data)
    }

    pub fn load_mesh_trusted_winding(&mut self, data: MeshData) -> Option<MeshId> {
        self.insert_mesh(data)
    }

    fn insert_mesh(&mut self, data: MeshData) -> Option<MeshId> {
        let radius = data.bounding_radius();
        let range = self.geometry_pool.upload(&data)?;

        let id = MeshId(self.next_mesh_id);
        self.next_mesh_id += 1;
        self.meshes.insert(
            id,
            Mesh {
                base_vertex: range.base_vertex,
                first_index: range.first_index,
                index_count: range.index_count,
                bounding_radius: radius,
                vertex_alloc: range.vertex_alloc,
                index_alloc: range.index_alloc,
            },
        );
        Some(id)
    }

    pub fn unload_mesh(&mut self, mesh_id: MeshId) {
        if let Some(mesh) = self.meshes.remove(&mesh_id) {
            self.geometry_pool
                .free(crate::resources::geometry_pool::MeshRange {
                    base_vertex: mesh.base_vertex,
                    first_index: mesh.first_index,
                    index_count: mesh.index_count,
                    vertex_alloc: mesh.vertex_alloc,
                    index_alloc: mesh.index_alloc,
                });
        }
    }

    pub fn setup_skybox(&mut self, vs_src: &str, fs_src: &str) -> ShaderId {
        let shader_id = self.load_shader(vs_src, None, fs_src);
        self.skybox = Some(SkyboxPipeline::new(self.gl.clone(), shader_id));
        shader_id
    }

    pub fn enable_skybox(&mut self, enable: bool) {
        if let Some(skybox) = &mut self.skybox {
            skybox.enabled = enable;
        }
    }

    pub fn set_skybox_color(&mut self, color: glam::Vec3) {
        if let Some(skybox) = &mut self.skybox {
            skybox.color = color;
        }
    }

    /// Set animated cloud density for skybox shaders that use the built-in
    /// `uCloudCoverage` uniform. Zero disables the cloud layer; one is dense.
    pub fn set_skybox_cloud_coverage(&mut self, coverage: f32) {
        if let Some(skybox) = &mut self.skybox {
            skybox.cloud_coverage = coverage.clamp(0.0, 1.0);
        }
    }

    pub fn mesh_pool_stats(&self) -> ((usize, usize), (usize, usize)) {
        (
            (
                self.geometry_pool.free_vertex_space(),
                self.geometry_pool.largest_free_vertex_block(),
            ),
            (
                self.geometry_pool.free_index_space(),
                self.geometry_pool.largest_free_index_block(),
            ),
        )
    }
}
