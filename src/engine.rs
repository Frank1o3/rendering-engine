// src/renderer/engine.rs
use crate::buffer::PersistentMappedBuffer;
use crate::draw_indirect::{DrawElementsIndirectCommand, IndirectBuffer, MdiStrategy};
use crate::frame_data::{FrameData, InstanceData};
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
    read_handle: ReadHandle<FrameData>, // <--- The renderer owns its data source
    shaders: HashMap<ShaderId, ShaderProgram>,
    meshes: HashMap<MeshId, Mesh>,
    materials: HashMap<MaterialId, MaterialEntry>,
    sorted_instances: Vec<SortedInstance>, // Pre-allocated scratch buffer for sorting
    indirect_cmds: Vec<DrawElementsIndirectCommand>, // Pre-allocated scratch buffer for indirect commands
    next_mesh_id: u32,
    next_shader_id: u32,
    next_material_id: u32,
    transform_buffer: PersistentMappedBuffer,
    width: i32,  // Window width for orthographic projection
    height: i32, // Window height for orthographic projection

    // Scene Object registry
    pub scene: Scene,
    mdi_strategy: MdiStrategy,
    indirect_buffer: IndirectBuffer,

    // Phase 3: Pipeline State Caching
    pipeline_cache: PipelineCache,
    current_pipeline_id: Option<PipelineStateId>,
}

impl Renderer {
    /// The game engine passes the GL context AND the ReadHandle on creation.
    pub fn new(gl: glow::Context, read_handle: ReadHandle<FrameData>) -> Self {
        let gl = Arc::new(gl);

        // Allocate persistent buffer for compact transform InstanceData (65536 * 32 bytes = 2MB)
        let transform_buffer = PersistentMappedBuffer::new(
            gl.clone(),
            MAX_OBJECTS * std::mem::size_of::<InstanceData>(),
        );

        let indirect_buffer = IndirectBuffer::new(gl.clone(), true);

        Self {
            gl: gl.clone(),
            read_handle,
            shaders: HashMap::new(),
            meshes: HashMap::new(),
            materials: HashMap::new(),
            sorted_instances: Vec::with_capacity(MAX_OBJECTS),
            indirect_cmds: Vec::with_capacity(1024),
            transform_buffer,
            next_mesh_id: 0,
            next_shader_id: 0,
            next_material_id: 0,
            width: 1280, // Default window width
            height: 720, // Default window height
            scene: Scene::new(),
            mdi_strategy: MdiStrategy::Multi,
            indirect_buffer,
            pipeline_cache: PipelineCache::new(),
            current_pipeline_id: None,
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

    pub fn load_mesh(&mut self, data: MeshData) -> MeshId {
        let id = MeshId(self.next_mesh_id);
        self.next_mesh_id += 1;
        let mesh = Mesh::new(self.gl.clone(), &data, self.transform_buffer.handle);
        self.meshes.insert(id, mesh);
        id
    }

    pub fn load_shader(&mut self, vs: &str, fs: &str) -> ShaderId {
        let id = ShaderId(self.next_shader_id);
        self.next_shader_id += 1;
        let shader = ShaderProgram::new(self.gl.clone(), vs, fs).expect("Failed to compile shader");
        self.shaders.insert(id, shader);
        id
    }

    /// Loads a single shader pair from files
    pub fn load_shader_from_files(
        &mut self,
        vs_path: &Path,
        fs_path: &Path,
    ) -> Result<ShaderId, String> {
        let id = ShaderId(self.next_shader_id);
        self.next_shader_id += 1;
        let shader = ShaderProgram::from_files(self.gl.clone(), vs_path, fs_path)?;
        self.shaders.insert(id, shader);
        Ok(id)
    }

    /// Scans a directory for `.vert` and `.frag` pairs, loads them all,
    /// and returns a map of `ShaderName -> ShaderId`.
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

            // Look for vertex shaders
            if path.extension().and_then(|s| s.to_str()) == Some("vert") {
                let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
                let frag_path = path.with_extension("frag");

                if frag_path.exists() {
                    match self.load_shader_from_files(&path, &frag_path) {
                        Ok(id) => {
                            log::info!("Loaded shader: '{}'", stem);
                            loaded_shaders.insert(stem, id);
                        }
                        Err(e) => {
                            log::error!("Failed to compile shader '{}': {}", stem, e);
                            return Err(e); // Fail fast on startup if a shader is broken
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

    /// The main render loop. No arguments needed!
    /// It pulls camera and dynamic data directly from the lock-free buffer,
    /// combines them with static/dynamic scene registry states, and dispatches MDI.
    pub fn render(&mut self) {
        let mut current_frame = FrameData::default();
        if !self.read_handle.consume(&mut current_frame) {
            return;
        }

        // Reset pipeline state at start of frame to ensure clean state
        self.current_pipeline_id = None;

        unsafe {
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        // ==========================================
        // PASS 1: 3D SCENE (Perspective)
        // ==========================================
        let view = math::camera_to_view_matrix(
            current_frame.camera_position,
            current_frame.camera_rotation,
        );
        let proj = math::camera_to_projection_matrix(
            current_frame.camera_fov,
            current_frame.camera_aspect_ratio,
            current_frame.camera_near,
            current_frame.camera_far,
        );
        let vp = proj * view;

        // 1. Recompute cached transforms for dirty objects in Scene
        self.scene.flush_dirty();

        // 2. Collect sorted instances from the Scene registry
        self.scene.collect_sorted_into(&mut self.sorted_instances);

        // 3. Append dynamic 3D commands from FrameData
        let dynamic_commands_exist = !current_frame.commands.is_empty();
        for cmd in &current_frame.commands {
            self.sorted_instances.push(SortedInstance {
                material_id: cmd.material_id,
                mesh_id: cmd.mesh_id,
                instance: cmd.to_instance_data(),
            });
        }

        // 4. Sort the combined list if dynamic commands were added
        if dynamic_commands_exist {
            self.sorted_instances
                .sort_unstable_by_key(|o| (o.material_id, o.mesh_id));
        }

        // 5. Render standard 3D instances
        let mut transform_offset = 0;
        transform_offset = self.render_instances(&vp, transform_offset);

        // ==========================================
        // PASS 2: UI OVERLAY (Orthographic)
        // ==========================================
        if !current_frame.ui_commands.is_empty() {
            // Build a manual orthographic projection matrix
            // Maps [0, width] to [-1, 1] and [0, height] to [-1, 1] (Y inverted)
            let w = self.width as f32;
            let h = self.height as f32;
            let ui_proj = glam::Mat4::from_translation(glam::Vec3::new(-1.0, 1.0, 0.0))
                * glam::Mat4::from_scale(glam::Vec3::new(2.0 / w, -2.0 / h, 1.0));

            // Load UI elements into sorted list
            self.sorted_instances.clear();
            for cmd in &current_frame.ui_commands {
                self.sorted_instances.push(SortedInstance {
                    material_id: cmd.material_id,
                    mesh_id: cmd.mesh_id,
                    instance: cmd.to_instance_data(),
                });
            }
            self.sorted_instances
                .sort_unstable_by_key(|o| (o.material_id, o.mesh_id));

            // Render UI overlay using dedicated orthographic pass
            self.render_instances(&ui_proj, transform_offset);
        }
    }

    /// Helper method to batch and draw a slice of sorted instances.
    /// Returns the new transform_offset so the next pass knows where to start writing.
    fn render_instances(&mut self, vp: &glam::Mat4, mut transform_offset: usize) -> usize {
        if self.sorted_instances.is_empty() {
            return transform_offset;
        }

        let start_offset = transform_offset;
        let total_instances = self.sorted_instances.len();

        // 1. Write instance attributes to persistent GPU memory contiguously
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

        // 2. Iterate and group by (material_id, mesh_id)
        let actual_instances = transform_offset - start_offset;
        let mut i = 0;
        while i < actual_instances {
            let start_inst = &self.sorted_instances[i];
            let mat_id = start_inst.material_id;
            let mesh_id = start_inst.mesh_id;

            // Find the end of this contiguous (material, mesh) group
            let mut group_end = i + 1;
            while group_end < actual_instances {
                let inst = &self.sorted_instances[group_end];
                if inst.material_id != mat_id || inst.mesh_id != mesh_id {
                    break;
                }
                group_end += 1;
            }

            let group_size = group_end - i;

            // 1. Extract Copy data to release all borrows on `self` immediately
            let material_entry = *self.materials.get(&mat_id).expect("Material ID not found");
            let pipeline_state = *self
                .pipeline_cache
                .get_by_id(material_entry.pipeline_id)
                .expect("Pipeline state ID not found in cache");

            // 2. Apply pipeline state (borrows `self` mutably, but no outstanding borrows exist now)
            self.apply_pipeline(&pipeline_state);

            // 3. Now it is safe to borrow `self.meshes` and `self.shaders` again
            let mesh = self.meshes.get(&mesh_id).expect("Mesh ID not found");
            let shader = self
                .shaders
                .get(&material_entry.shader_id)
                .expect("Shader ID not found");

            // Build the indirect command for this group
            let base_instance = (start_offset + i) as u32;
            let cmd = DrawElementsIndirectCommand {
                count: mesh.index_count as u32,
                instance_count: group_size as u32,
                first_index: 0,
                base_vertex: 0,
                base_instance,
            };

            // Upload command to DRAW_INDIRECT_BUFFER
            self.indirect_cmds.clear();
            self.indirect_cmds.push(cmd);
            self.indirect_buffer.upload(&self.indirect_cmds);

            // If MultiCount strategy, upload count
            if self.mdi_strategy == MdiStrategy::MultiCount {
                self.indirect_buffer.upload_count(1);
            }

            unsafe {
                // Set VP matrix uniform
                shader.set_vp(vp);
                // Bind VAO
                self.gl.bind_vertex_array(Some(mesh.vao));
                // Dispatch indirect drawing using chosen MDI strategy
                self.indirect_buffer
                    .dispatch(self.mdi_strategy, glow::UNSIGNED_INT, 0, 1, 1);
            }

            i = group_end;
        }

        unsafe {
            self.gl.bind_vertex_array(None);
        }

        transform_offset
    }

    /// Applies a pipeline state, skipping redundant GL calls if the state is already bound
    fn apply_pipeline(&mut self, pipeline: &PipelineState) {
        // Check if we already have this pipeline bound
        if let Some(current_id) = self.current_pipeline_id {
            if current_id == pipeline.hash().into() {
                return; // State already bound, skip redundant calls
            }
        }

        // FIX: Look up the actual ShaderProgram to get its real OpenGL handle!
        let shader = self
            .shaders
            .get(&ShaderId(pipeline.shader_id))
            .expect("Shader not found in apply_pipeline");

        unsafe {
            // Bind the ACTUAL OpenGL program handle
            self.gl.use_program(Some(shader.program));

            // Set face culling
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

            // Set depth test
            if pipeline.depth_test {
                self.gl.enable(glow::DEPTH_TEST);
                self.gl.depth_func(pipeline.depth_func.to_glow());
            } else {
                self.gl.disable(glow::DEPTH_TEST);
            }

            // Set depth write mask
            self.gl.depth_mask(pipeline.depth_write);

            // Set blending
            if pipeline.blend_enabled {
                self.gl.enable(glow::BLEND);
                self.gl
                    .blend_func(pipeline.src_factor.to_glow(), pipeline.dst_factor.to_glow());
            } else {
                self.gl.disable(glow::BLEND);
            }
        }

        // Update current pipeline ID
        self.current_pipeline_id = Some(PipelineStateId(pipeline.hash()));
    }
}
