use crate::core::buffer::PersistentMappedBuffer;
use crate::core::math::{self, sphere_inside_frustum};
use crate::core::triple_buffer::ReadHandle;
use crate::render::draw_indirect::{DrawElementsIndirectCommand, IndirectBuffer, MdiStrategy};
use crate::render::frame_data::{FrameData, InstanceData};
use crate::render::pipeline::{PipelineCache, PipelineState, PipelineStateId};
use crate::render::scene::{ObjectHandle, ObjectKind, Scene, SortedInstance};
use crate::render::skybox::SkyboxPipeline;
use crate::resources::geometry_pool::GeometryPool;
use crate::resources::material::{MaterialId, MaterialManager};
use crate::resources::mesh::{Mesh, MeshData};
use crate::resources::shader::{ShaderId, ShaderManager};
use crate::resources::texture::{TextureId, TextureManager};

use bytemuck::{Pod, Zeroable};
use glam::Vec4;
use glow::HasContext;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

const MAX_OBJECTS: usize = 65536;
const TRANSFORM_REGIONS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct MeshId(pub u32);

pub struct Renderer {
    gl: Arc<glow::Context>,
    read_handle: ReadHandle<FrameData>,
    pub shaders: ShaderManager,
    pub materials: MaterialManager,
    pub textures: TextureManager,
    pub skybox: Option<SkyboxPipeline>,
    meshes: HashMap<MeshId, Mesh>,
    sorted_instances: Vec<SortedInstance>,
    indirect_cmds: Vec<DrawElementsIndirectCommand>,
    next_mesh_id: u32,
    transform_buffer: PersistentMappedBuffer,
    geometry_pool: GeometryPool,
    width: i32,
    height: i32,

    pub scene: Scene,
    mdi_strategy: MdiStrategy,
    indirect_buffer: IndirectBuffer,

    pipeline_cache: PipelineCache,
    current_pipeline_id: Option<PipelineStateId>,

    last_frame: FrameData,
    has_frame: bool,

    region_fences: [Option<glow::Fence>; TRANSFORM_REGIONS],
    frame_index: u64,
    pending_draws: Vec<(PipelineStateId, DrawElementsIndirectCommand)>,
    last_frustum: Option<[Vec4; 6]>,
}

impl Renderer {
    pub fn new(gl: glow::Context, read_handle: ReadHandle<FrameData>) -> Self {
        let gl = Arc::new(gl);

        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
        }

        let transform_buffer = PersistentMappedBuffer::new(
            gl.clone(),
            MAX_OBJECTS * TRANSFORM_REGIONS * std::mem::size_of::<InstanceData>(),
        );

        let geometry_pool = GeometryPool::new(gl.clone(), transform_buffer.handle);
        let indirect_buffer = IndirectBuffer::new(gl.clone(), true);

        Self {
            gl,
            read_handle,
            shaders: ShaderManager::new(),
            materials: MaterialManager::new(),
            textures: TextureManager::new(),
            skybox: None,
            meshes: HashMap::new(),
            sorted_instances: Vec::with_capacity(MAX_OBJECTS),
            indirect_cmds: Vec::with_capacity(1024),
            transform_buffer,
            geometry_pool,
            next_mesh_id: 0,
            width: 1280,
            height: 720,
            scene: Scene::new(),
            mdi_strategy: MdiStrategy::Multi,
            indirect_buffer,
            pipeline_cache: PipelineCache::new(),
            current_pipeline_id: None,
            last_frame: FrameData::default(),
            has_frame: false,
            region_fences: [None; TRANSFORM_REGIONS],
            frame_index: 0,
            pending_draws: Vec::with_capacity(1024),
            last_frustum: None,
        }
    }

    pub fn context(&self) -> Arc<glow::Context> {
        self.gl.clone()
    }

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
            self.geometry_pool.free(crate::resources::geometry_pool::MeshRange {
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

    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        unsafe {
            self.gl.viewport(0, 0, width, height);
        }
    }

    // ── Transform-region synchronization ────────────────────────────────────

    fn wait_for_region(&mut self, region: usize) {
        let Some(fence) = self.region_fences[region].take() else {
            return;
        };
        unsafe {
            const TIMEOUT_NS: i32 = 1_000_000_000;
            loop {
                let status =
                    self.gl
                        .client_wait_sync(fence, glow::SYNC_FLUSH_COMMANDS_BIT, TIMEOUT_NS);
                match status {
                    glow::ALREADY_SIGNALED | glow::CONDITION_SATISFIED => break,
                    glow::TIMEOUT_EXPIRED => {
                        log::warn!(
                            "GPU still busy with transform region {} after 1s — waiting again",
                            region
                        );
                        continue;
                    }
                    glow::WAIT_FAILED => {
                        log::error!("client_wait_sync failed for transform region {}", region);
                        break;
                    }
                    _ => break,
                }
            }
            self.gl.delete_sync(fence);
        }
    }

    fn fence_region(&mut self, region: usize) {
        unsafe {
            match self.gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0) {
                Ok(fence) => self.region_fences[region] = Some(fence),
                Err(e) => log::error!("fence_sync failed: {}", e),
            }
        }
    }

    pub fn current_frustum(&self) -> Option<[Vec4; 6]> {
        self.last_frustum
    }

    // ── Render ───────────────────────────────────────────────────────────────

    pub fn render(&mut self) {
        if self.read_handle.consume(&mut self.last_frame) {
            self.has_frame = true;
        }

        unsafe {
            self.gl.depth_mask(true);
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        if !self.has_frame {
            return;
        }

        let region = (self.frame_index % TRANSFORM_REGIONS as u64) as usize;
        self.wait_for_region(region);
        let region_base = region * MAX_OBJECTS;
        let region_limit = region_base + MAX_OBJECTS;

        self.current_pipeline_id = None;

        // ── PASS 0: Skybox ────────────────────────────────────────────────────
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

        if let Some(skybox) = &self.skybox {
            skybox.draw(&self.shaders, &inv_vp);
        }

        // ── PASS 1: 3D scene ──────────────────────────────────────────────────
        let frustum = math::extract_frustum_planes(vp);
        self.last_frustum = Some(frustum);

        self.scene.flush_dirty();
        self.scene.collect_sorted_into(&mut self.sorted_instances);

        let dynamic_commands_exist = !self.last_frame.commands.is_empty();
        for cmd in &self.last_frame.commands {
            self.sorted_instances.push(SortedInstance {
                material_id: cmd.material_id,
                mesh_id: cmd.mesh_id,
                instance: cmd.to_instance_data(),
            });
        }
        if dynamic_commands_exist {
            self.sorted_instances
                .sort_unstable_by_key(|o| (o.material_id, o.mesh_id));
        }

        let mut transform_offset = region_base;
        transform_offset =
            self.render_instances(&vp, transform_offset, region_limit, Some(&frustum));

        // ── PASS 2: UI overlay ────────────────────────────────────────────────
        if !self.last_frame.ui_commands.is_empty() {
            let w = self.width as f32;
            let h = self.height as f32;
            let ui_proj = glam::Mat4::from_translation(glam::Vec3::new(-1.0, 1.0, 0.0))
                * glam::Mat4::from_scale(glam::Vec3::new(2.0 / w, -2.0 / h, 1.0));

            self.sorted_instances.clear();
            for cmd in &self.last_frame.ui_commands {
                self.sorted_instances.push(SortedInstance {
                    material_id: cmd.material_id,
                    mesh_id: cmd.mesh_id,
                    instance: cmd.to_instance_data(),
                });
            }
            self.sorted_instances
                .sort_unstable_by_key(|o| (o.material_id, o.mesh_id));

            self.render_instances(&ui_proj, transform_offset, region_limit, None);
        }

        self.fence_region(region);
        self.frame_index = self.frame_index.wrapping_add(1);
    }

    fn render_instances(
        &mut self,
        vp: &glam::Mat4,
        mut transform_offset: usize,
        region_limit: usize,
        frustum: Option<&[Vec4; 6]>,
    ) -> usize {
        if self.sorted_instances.is_empty() {
            return transform_offset;
        }

        let start_offset = transform_offset;
        let total = self.sorted_instances.len();

        let mut visible_indices: Vec<usize> = Vec::with_capacity(total);

        for i in 0..total {
            if transform_offset >= region_limit {
                log::warn!("Max transform capacity ({}) exceeded", MAX_OBJECTS);
                break;
            }

            let inst = &self.sorted_instances[i];

            if let Some(planes) = frustum {
                let mesh = self.meshes.get(&inst.mesh_id).expect("Mesh ID not found");
                let center = glam::Vec3::from(inst.instance.position);
                let radius = mesh.bounding_radius * inst.instance.scale + 0.5;

                if !sphere_inside_frustum(center, radius, planes) {
                    continue;
                }
            }

            self.transform_buffer
                .write_instance(transform_offset, &inst.instance);
            visible_indices.push(i);
            transform_offset += 1;
        }

        let actual = transform_offset - start_offset;
        if actual == 0 {
            return transform_offset;
        }

        unsafe {
            self.gl
                .memory_barrier(glow::CLIENT_MAPPED_BUFFER_BARRIER_BIT);
        }

        self.pending_draws.clear();

        let mut vi = 0;
        while vi < actual {
            let first_si = visible_indices[vi];
            let mat_id = self.sorted_instances[first_si].material_id;
            let mesh_id = self.sorted_instances[first_si].mesh_id;

            let mut group_end = vi + 1;
            while group_end < actual {
                let si = visible_indices[group_end];
                let inst = &self.sorted_instances[si];
                if inst.material_id != mat_id || inst.mesh_id != mesh_id {
                    break;
                }
                group_end += 1;
            }
            let group_size = group_end - vi;

            let material_entry = self
                .materials
                .get(mat_id)
                .cloned()
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

            self.pending_draws.push((material_entry.pipeline_id, cmd));
            vi = group_end;
        }

        self.pending_draws
            .sort_by_key(|(pipeline_id, _)| pipeline_id.0);

        unsafe {
            self.gl.bind_vertex_array(Some(self.geometry_pool.vao));
        }

        let mut pi = 0;
        while pi < self.pending_draws.len() {
            let pipeline_id = self.pending_draws[pi].0;

            let mut run_end = pi + 1;
            while run_end < self.pending_draws.len() && self.pending_draws[run_end].0 == pipeline_id
            {
                run_end += 1;
            }

            let pipeline_state = *self
                .pipeline_cache
                .get_by_id(pipeline_id)
                .expect("Pipeline state not found");
            self.apply_pipeline(&pipeline_state);

            let shader = self
                .shaders
                .get(ShaderId(pipeline_state.shader_id))
                .expect("Shader ID not found");
            shader.set_vp(vp);

            self.indirect_cmds.clear();
            self.indirect_cmds
                .extend(self.pending_draws[pi..run_end].iter().map(|(_, cmd)| *cmd));
            let cmd_count = self.indirect_buffer.upload(&self.indirect_cmds);
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

    fn apply_pipeline(&mut self, pipeline: &PipelineState) {
        let requested_id = PipelineStateId(pipeline.hash());
        if self.current_pipeline_id == Some(requested_id) {
            return;
        }

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

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            for fence in self.region_fences.iter().flatten() {
                self.gl.delete_sync(*fence);
            }
        }
    }
}
