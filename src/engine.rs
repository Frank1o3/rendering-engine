// src/renderer/engine.rs
use crate::buffer::PersistentMappedBuffer;
use crate::draw_indirect::{DrawElementsIndirectCommand, IndirectBuffer, MdiStrategy};
use crate::frame_data::{FrameData, InstanceData};
use crate::geometry_pool::GeometryPool;
use crate::math::{self, sphere_inside_frustum};
use crate::mesh::{Mesh, MeshData};
use crate::pipeline::{PipelineCache, PipelineState, PipelineStateId};
use crate::scene::{ObjectHandle, ObjectKind, Scene, SortedInstance};
use crate::shader::ShaderProgram;
use crate::triple_buffer::ReadHandle;
use bytemuck::{Pod, Zeroable};
use glam::Vec4;
use glow::HasContext;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

const MAX_OBJECTS: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct MeshId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct ShaderId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct MaterialId(pub u32);

/// Material entry storing both shader and pipeline state.
#[derive(Debug, Clone, Copy)]
pub struct MaterialEntry {
    pub shader_id: ShaderId,
    pub pipeline_id: PipelineStateId,
}

pub struct Renderer {
    gl: Arc<glow::Context>,
    read_handle: ReadHandle<FrameData>,
    shaders: HashMap<ShaderId, ShaderProgram>,
    meshes: HashMap<MeshId, Mesh>,
    materials: HashMap<MaterialId, MaterialEntry>,
    sorted_instances: Vec<SortedInstance>,
    indirect_cmds: Vec<DrawElementsIndirectCommand>,
    next_mesh_id: u32,
    next_shader_id: u32,
    next_material_id: u32,
    transform_buffer: PersistentMappedBuffer,
    geometry_pool: GeometryPool,
    width: i32,
    height: i32,

    pub scene: Scene,
    mdi_strategy: MdiStrategy,
    indirect_buffer: IndirectBuffer,

    pipeline_cache: PipelineCache,
    current_pipeline_id: Option<PipelineStateId>,

    /// Most recently consumed frame. Re-rendered when the producer is silent.
    last_frame: FrameData,
    has_frame: bool,
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
            MAX_OBJECTS * std::mem::size_of::<InstanceData>(),
        );

        let geometry_pool = GeometryPool::new(gl.clone(), transform_buffer.handle);
        let indirect_buffer = IndirectBuffer::new(gl.clone(), true);

        Self {
            gl,
            read_handle,
            shaders: HashMap::new(),
            meshes: HashMap::new(),
            materials: HashMap::new(),
            sorted_instances: Vec::with_capacity(MAX_OBJECTS),
            indirect_cmds: Vec::with_capacity(1024),
            transform_buffer,
            geometry_pool,
            next_mesh_id: 0,
            next_shader_id: 0,
            next_material_id: 0,
            width: 1280,
            height: 720,
            scene: Scene::new(),
            mdi_strategy: MdiStrategy::Multi,
            indirect_buffer,
            pipeline_cache: PipelineCache::new(),
            current_pipeline_id: None,
            last_frame: FrameData::default(),
            has_frame: false,
        }
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

    // ── Asset loading ─────────────────────────────────────────────────────────

    pub fn load_mesh(&mut self, mut data: MeshData) -> MeshId {
        let radius = data.bounding_radius();
        data.fix_winding();
        let range = self.geometry_pool.upload(&data);

        let id = MeshId(self.next_mesh_id);
        self.next_mesh_id += 1;
        self.meshes.insert(
            id,
            Mesh {
                base_vertex: range.base_vertex,
                first_index: range.first_index,
                index_count: range.index_count,
                bounding_radius: radius,
            },
        );
        id
    }

    pub fn load_shader(&mut self, vs: &str, gs: Option<&str>, fs: &str) -> ShaderId {
        let id = ShaderId(self.next_shader_id);
        self.next_shader_id += 1;
        let shader =
            ShaderProgram::new(self.gl.clone(), vs, gs, fs).expect("Failed to compile shader");
        self.shaders.insert(id, shader);
        id
    }

    pub fn load_shader_from_files(
        &mut self,
        vs_path: &Path,
        gs_path: Option<&Path>,
        fs_path: &Path,
    ) -> Result<ShaderId, String> {
        let id = ShaderId(self.next_shader_id);
        self.next_shader_id += 1;
        let shader = ShaderProgram::from_files(self.gl.clone(), vs_path, gs_path, fs_path)?;
        self.shaders.insert(id, shader);
        Ok(id)
    }

    /// Scan a directory for `<name>.vert` / `<name>.frag` pairs.
    /// An optional `<name>.geom` is loaded automatically if present.
    pub fn load_shaders_from_dir(
        &mut self,
        dir: &Path,
    ) -> Result<HashMap<String, ShaderId>, String> {
        let mut loaded = HashMap::new();

        if !dir.is_dir() {
            return Err(format!("Shader directory {:?} does not exist.", dir));
        }

        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();

            if path.extension().and_then(|s| s.to_str()) == Some("vert") {
                let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
                let frag_path = path.with_extension("frag");
                let geom_path = path.with_extension("geom");

                if frag_path.exists() {
                    let gs = if geom_path.exists() {
                        Some(geom_path.as_path())
                    } else {
                        None
                    };
                    match self.load_shader_from_files(&path, gs, &frag_path) {
                        Ok(id) => {
                            log::info!(
                                "Loaded shader: '{}'{}",
                                stem,
                                if gs.is_some() {
                                    " (+ geometry stage)"
                                } else {
                                    ""
                                }
                            );
                            loaded.insert(stem, id);
                        }
                        Err(e) => {
                            log::error!("Failed to compile shader '{}': {}", stem, e);
                            return Err(e);
                        }
                    }
                } else {
                    log::warn!("Found {}.vert but no matching .frag — skipped.", stem);
                }
            }
        }

        Ok(loaded)
    }

    pub fn create_material(
        &mut self,
        shader_id: ShaderId,
        pipeline_state: PipelineState,
    ) -> MaterialId {
        let id = MaterialId(self.next_material_id);
        self.next_material_id += 1;
        let pipeline_id = self.pipeline_cache.register(pipeline_state);
        self.materials.insert(
            id,
            MaterialEntry {
                shader_id,
                pipeline_id,
            },
        );
        id
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        unsafe {
            self.gl.viewport(0, 0, width, height);
        }
    }

    // ── Render ───────────────────────────────────────────────────────────────

    /// Main render entry point.
    ///
    /// Depth mask is forced `true` before clearing because the UI pipeline
    /// sets it to `false`, and OpenGL silently ignores
    /// `glClear(GL_DEPTH_BUFFER_BIT)` when the mask is off.
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

        self.current_pipeline_id = None;

        // ── PASS 1: 3D scene (perspective + frustum culling) ─────────────────
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

        // Extract frustum planes from the VP matrix for CPU-side sphere tests.
        let frustum = math::extract_frustum_planes(vp);

        self.scene.flush_dirty();
        self.scene.collect_sorted_into(&mut self.sorted_instances);

        // Dynamic commands from the triple buffer are appended after scene objects.
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

        let mut transform_offset = 0;
        transform_offset = self.render_instances(&vp, transform_offset, Some(&frustum));

        // ── PASS 2: UI overlay (orthographic, no frustum culling) ────────────
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

            // UI elements are 2-D overlays — no 3-D frustum culling.
            self.render_instances(&ui_proj, transform_offset, None);
        }
    }

    /// Writes instances into the transform buffer and issues draw calls,
    /// optionally skipping objects whose bounding sphere is outside the frustum.
    ///
    /// Returns the next free slot in the transform buffer.
    fn render_instances(
        &mut self,
        vp: &glam::Mat4,
        mut transform_offset: usize,
        frustum: Option<&[Vec4; 6]>,
    ) -> usize {
        if self.sorted_instances.is_empty() {
            return transform_offset;
        }

        let start_offset = transform_offset;
        let total = self.sorted_instances.len();

        // Build a compact list of (sorted_instances index → transform slot)
        // after frustum culling.  We can't reuse sorted_instances in-place
        // because it would mess up the group boundaries computed below.
        let mut visible_indices: Vec<usize> = Vec::with_capacity(total);

        for i in 0..total {
            if transform_offset >= MAX_OBJECTS {
                log::warn!("Max transform capacity ({}) exceeded", MAX_OBJECTS);
                break;
            }

            let inst = &self.sorted_instances[i];

            // ── CPU frustum cull ──────────────────────────────────────────────
            if let Some(planes) = frustum {
                let mesh = self.meshes.get(&inst.mesh_id).expect("Mesh ID not found");
                let center = glam::Vec3::from(inst.instance.position);
                let radius = mesh.bounding_radius * inst.instance.scale;

                if !sphere_inside_frustum(center, radius, planes) {
                    continue; // skip — entirely outside the frustum
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
            self.gl.bind_vertex_array(Some(self.geometry_pool.vao));
        }

        let mut vi = 0; // cursor into visible_indices
        while vi < actual {
            let first_si = visible_indices[vi];
            let mat_id = self.sorted_instances[first_si].material_id;
            let mesh_id = self.sorted_instances[first_si].mesh_id;

            // Grow the group as long as material AND mesh match.
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

            let material_entry = *self.materials.get(&mat_id).expect("Material ID not found");
            let pipeline_state = *self
                .pipeline_cache
                .get_by_id(material_entry.pipeline_id)
                .expect("Pipeline state not found");

            self.apply_pipeline(&pipeline_state);

            let mesh = self.meshes.get(&mesh_id).expect("Mesh ID not found");
            let shader = self
                .shaders
                .get(&material_entry.shader_id)
                .expect("Shader ID not found");

            let base_instance = (start_offset + vi) as u32;

            let cmd = DrawElementsIndirectCommand {
                count: mesh.index_count as u32,
                instance_count: group_size as u32,
                first_index: mesh.first_index,
                base_vertex: mesh.base_vertex,
                base_instance,
            };

            self.indirect_cmds.clear();
            self.indirect_cmds.push(cmd);
            self.indirect_buffer.upload(&self.indirect_cmds);

            if self.mdi_strategy == MdiStrategy::MultiCount {
                self.indirect_buffer.upload_count(1);
            }

            shader.set_vp(vp);

            self.indirect_buffer
                .dispatch(self.mdi_strategy, glow::UNSIGNED_INT, 0, 1, 1);

            vi = group_end;
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
            .get(&ShaderId(pipeline.shader_id))
            .expect("Shader not found in apply_pipeline");

        unsafe {
            self.gl.use_program(Some(shader.program));

            match pipeline.cull_mode {
                crate::pipeline::CullMode::None => self.gl.disable(glow::CULL_FACE),
                crate::pipeline::CullMode::Front => {
                    self.gl.enable(glow::CULL_FACE);
                    self.gl.cull_face(glow::FRONT);
                }
                crate::pipeline::CullMode::Back => {
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

            // depth_mask must always be applied — it is NOT skipped by the cache
            // because render() resets it to true before clearing, so on re-entry
            // the pipeline's value would be stale if we relied on the cached ID.
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
}
