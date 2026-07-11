// src/renderer/engine.rs
use crate::buffer::PersistentMappedBuffer;
use crate::draw_indirect::{DrawElementsIndirectCommand, IndirectBuffer, MdiStrategy};
use crate::frame_data::{FrameData, InstanceData};
use crate::geometry_pool::GeometryPool;
use crate::math;
use crate::mesh::{Mesh, MeshData};
use crate::pipeline::{PipelineCache, PipelineState, PipelineStateId};
use crate::scene::{ObjectHandle, ObjectKind, Scene, SortedInstance};
use crate::shader::ShaderProgram;
use crate::triple_buffer::ReadHandle;
use bytemuck::{Pod, Zeroable};
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

/// Material entry storing both shader and pipeline state
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

    /// Most recently consumed frame. Re-rendered on frames where the producer
    /// hasn't published anything new yet, preventing blank frames.
    last_frame: FrameData,
    /// False until at least one frame has been consumed from the triple buffer.
    has_frame: bool,
}

impl Renderer {
    pub fn new(gl: glow::Context, read_handle: ReadHandle<FrameData>) -> Self {
        let gl = Arc::new(gl);

        // Establish the permanent OpenGL state that never changes between frames.
        // Doing this once here prevents any draw path from accidentally skipping it.
        unsafe {
            // Depth test on with standard less-than comparison.
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);

            // Depth writes on. MUST be true before the first clear (see render()).
            gl.depth_mask(true);

            // Background colour: dark grey so a blank frame is visually obvious.
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

    // --- Scene Object Registry delegation API ---

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
        position: glam::Vec3,
        rotation: glam::Quat,
        scale: f32,
    ) {
        self.scene.set_transform(handle, position, rotation, scale);
    }

    pub fn set_position(&mut self, handle: ObjectHandle, position: glam::Vec3) {
        self.scene.set_position(handle, position);
    }

    pub fn set_position_rotation(
        &mut self,
        handle: ObjectHandle,
        position: glam::Vec3,
        rotation: glam::Quat,
    ) {
        self.scene.set_position_rotation(handle, position, rotation);
    }

    pub fn set_mdi_strategy(&mut self, strategy: MdiStrategy) {
        self.mdi_strategy = strategy;
    }

    pub fn load_mesh(&mut self, mut data: MeshData) -> MeshId {
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

    pub fn load_shaders_from_dir(
        &mut self,
        dir: &Path,
    ) -> Result<HashMap<String, ShaderId>, String> {
        let mut loaded_shaders = HashMap::new();

        if !dir.is_dir() {
            return Err(format!("Shader directory {:?} does not exist.", dir));
        }

        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

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
                                if gs.is_some() { " (+ geometry stage)" } else { "" }
                            );
                            loaded_shaders.insert(stem, id);
                        }
                        Err(e) => {
                            log::error!("Failed to compile shader '{}': {}", stem, e);
                            return Err(e);
                        }
                    }
                } else {
                    log::warn!("Found {}.vert but no matching .frag file.", stem);
                }
            }
        }

        Ok(loaded_shaders)
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

    /// The main render function.
    ///
    /// Always clears both buffers before drawing. Depth mask is explicitly set
    /// to `true` before every clear because the UI pipeline sets it to `false`
    /// (transparent objects don't write depth), and OpenGL silently ignores
    /// `glClear(GL_DEPTH_BUFFER_BIT)` when the depth mask is off — meaning
    /// without this line the depth buffer would never clear after the first
    /// frame that draws UI, corrupting all subsequent depth comparisons.
    ///
    /// If no new frame has been published yet the last consumed frame is
    /// re-rendered rather than showing blank, which prevents flicker on frames
    /// where the producer thread is momentarily slower than the renderer.
    pub fn render(&mut self) {
        // Consume new frame data if the producer has published since last time.
        if self.read_handle.consume(&mut self.last_frame) {
            self.has_frame = true;
        }

        // ── THE CRITICAL FIX ──────────────────────────────────────────────────
        // The UI pipeline sets depth_write = false → gl.depth_mask(false).
        // In OpenGL, glClear(GL_DEPTH_BUFFER_BIT) is a no-op when the depth
        // mask is false. Without this line, after the first UI frame renders,
        // the depth buffer accumulates stale values forever: geometry that was
        // occluded last frame fails the depth test this frame even when it's
        // clearly in front, producing the "flickering in and out" symptom.
        // Forcing the mask to true here guarantees the clear always works,
        // regardless of what the previous frame's last pipeline state was.
        unsafe {
            self.gl.depth_mask(true);
            self.gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        if !self.has_frame {
            return;
        }

        // Reset cached pipeline so apply_pipeline issues fresh GL calls on the
        // first group — prevents state bleed from UI pass into 3D pass.
        self.current_pipeline_id = None;

        // ==========================================
        // PASS 1: 3D SCENE (Perspective)
        // ==========================================
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

        let mut transform_offset = 0;
        transform_offset = self.render_instances(&vp, transform_offset);

        // ==========================================
        // PASS 2: UI OVERLAY (Orthographic)
        // ==========================================
        if !self.last_frame.ui_commands.is_empty() {
            let w = self.width as f32;
            let h = self.height as f32;
            // Maps pixel space [0, w] × [0, h] to NDC [-1, 1] × [1, -1].
            // Y is inverted so pixel (0, 0) is the top-left corner.
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

            self.render_instances(&ui_proj, transform_offset);
        }
    }

    fn render_instances(&mut self, vp: &glam::Mat4, mut transform_offset: usize) -> usize {
        if self.sorted_instances.is_empty() {
            return transform_offset;
        }

        let start_offset = transform_offset;
        let total_instances = self.sorted_instances.len();

        for i in 0..total_instances {
            if transform_offset >= MAX_OBJECTS {
                log::warn!(
                    "Maximum transform instance capacity exceeded ({})",
                    MAX_OBJECTS
                );
                break;
            }
            let inst = &self.sorted_instances[i];
            self.transform_buffer
                .write_instance(transform_offset, &inst.instance);
            transform_offset += 1;
        }

        let actual_instances = transform_offset - start_offset;

        unsafe {
            self.gl.bind_vertex_array(Some(self.geometry_pool.vao));
        }

        let mut i = 0;
        while i < actual_instances {
            let start_inst = &self.sorted_instances[i];
            let mat_id = start_inst.material_id;
            let mesh_id = start_inst.mesh_id;

            let mut group_end = i + 1;
            while group_end < actual_instances {
                let inst = &self.sorted_instances[group_end];
                if inst.material_id != mat_id || inst.mesh_id != mesh_id {
                    break;
                }
                group_end += 1;
            }
            let group_size = group_end - i;

            let material_entry = *self.materials.get(&mat_id).expect("Material ID not found");
            let pipeline_state = *self
                .pipeline_cache
                .get_by_id(material_entry.pipeline_id)
                .expect("Pipeline state ID not found in cache");

            self.apply_pipeline(&pipeline_state);

            let mesh = self.meshes.get(&mesh_id).expect("Mesh ID not found");
            let shader = self
                .shaders
                .get(&material_entry.shader_id)
                .expect("Shader ID not found");

            let base_instance = (start_offset + i) as u32;

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

            i = group_end;
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

            // NOTE: depth_mask is intentionally NOT cached or skipped here.
            // It must always be applied because render() resets it to true
            // before clearing. If the UI pipeline sets it to false and we
            // skip it on re-entry, the 3D pass would see the wrong mask.
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
