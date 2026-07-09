// src/renderer/engine.rs
use crate::buffer::PersistentMappedBuffer;
use crate::frame_data::FrameData;
use crate::math;
use crate::mesh::{Mesh, MeshData};
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

pub struct Renderer {
    gl: Arc<glow::Context>,
    read_handle: ReadHandle<FrameData>, // <--- The renderer owns its data source
    shaders: HashMap<ShaderId, ShaderProgram>,
    meshes: HashMap<MeshId, Mesh>,
    materials: HashMap<MaterialId, ShaderId>,
    draw_indices: Vec<usize>, // Pre-allocated scratch buffer for sorting
    next_mesh_id: u32,
    next_shader_id: u32,
    next_material_id: u32,
    transform_buffer: PersistentMappedBuffer,
}

impl Renderer {
    /// The game engine passes the GL context AND the ReadHandle on creation.
    pub fn new(gl: glow::Context, read_handle: ReadHandle<FrameData>) -> Self {
        let gl = Arc::new(gl);
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
        }

        // Allocate persistent buffer for transforms (65536 * 64 bytes = 4MB)
        let transform_buffer = PersistentMappedBuffer::new(
            gl.clone(),
            MAX_OBJECTS * std::mem::size_of::<glam::Mat4>(),
        );

        Self {
            gl: gl.clone(),
            read_handle,
            shaders: HashMap::new(),
            meshes: HashMap::new(),
            materials: HashMap::new(),
            draw_indices: Vec::with_capacity(MAX_OBJECTS),
            transform_buffer,
            next_mesh_id: 0,
            next_shader_id: 0,
            next_material_id: 0,
        }
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

    pub fn create_material(&mut self, shader_id: ShaderId) -> MaterialId {
        let id = MaterialId(self.next_material_id);
        self.next_material_id += 1;
        self.materials.insert(id, shader_id);
        id
    }

    pub fn resize(&self, width: i32, height: i32) {
        unsafe {
            self.gl.viewport(0, 0, width, height);
        }
    }

    /// The main render loop. No arguments needed!
    /// It pulls data directly from the lock-free buffer.
    pub fn render(&mut self) {
        let mut current_frame = FrameData::default();
        if !self.read_handle.consume(&mut current_frame) {
            return;
        }

        unsafe {
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        // 1. Compute VP once per frame
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

        // 2. Sort by (material, mesh) to batch draw calls
        self.draw_indices.clear();
        self.draw_indices.extend(0..current_frame.commands.len());
        self.draw_indices.sort_by_key(|&i| {
            let cmd = &current_frame.commands[i];
            (cmd.material_id, cmd.mesh_id) // Group by BOTH!
        });

        // 3. Group and Dispatch
        let mut i = 0;
        let mut transform_offset = 0;

        while i < self.draw_indices.len() {
            let start_idx = self.draw_indices[i];
            let mat_id = current_frame.commands[start_idx].material_id;
            let mesh_id = current_frame.commands[start_idx].mesh_id;

            // Find the end of this group
            let mut group_end = i + 1;
            while group_end < self.draw_indices.len() {
                let idx = self.draw_indices[group_end];
                let cmd = &current_frame.commands[idx];
                if cmd.material_id != mat_id || cmd.mesh_id != mesh_id {
                    break;
                }
                group_end += 1;
            }

            let mesh = self.meshes.get(&mesh_id).unwrap();
            let shader_id = self.materials.get(&mat_id).unwrap();
            let shader = self.shaders.get(shader_id).unwrap();

            unsafe {
                // Bind state ONCE for this entire group
                self.gl.use_program(Some(shader.program));
                shader.set_vp(&vp);
                self.gl.bind_vertex_array(Some(mesh.vao));

                let instance_count = (group_end - i) as i32;
                let base_instance = transform_offset as u32;

                // Write transforms directly to persistent GPU memory
                for j in i..group_end {
                    let cmd_idx = self.draw_indices[j];
                    let cmd = &current_frame.commands[cmd_idx];
                    self.transform_buffer
                        .write_mat4(transform_offset, &cmd.model_matrix);
                    transform_offset += 1;
                }

                // THE MAGIC CALL: Draw all instances in this group in exactly 1 CPU command!
                self.gl.draw_elements_instanced_base_vertex_base_instance(
                    glow::TRIANGLES,
                    mesh.index_count,   // count: i32
                    glow::UNSIGNED_INT, // element_type: u32
                    0, // offset: i32 (0 bytes into the EBO, since we bind the VAO per group)
                    instance_count, // instance_count: i32 (The number of objects in this batch)
                    0, // base_vertex: i32 (0, because we aren't merging meshes into one giant VBO)
                    base_instance, // base_instance: u32 (Offsets the instanced Mat4 attributes in our persistent buffer!)
                );
            }

            i = group_end;
        }

        unsafe {
            self.gl.bind_vertex_array(None);
            self.gl.use_program(None);
        }
    }
}
