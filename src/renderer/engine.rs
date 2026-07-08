use glam::Mat4;
use glow::HasContext;
use std::collections::HashMap;
use std::sync::Arc;

use crate::renderer::math;
use crate::renderer::mesh::{Mesh, MeshData};
use crate::renderer::shader::ShaderProgram;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShaderId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialId(u32);

pub struct Camera {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

pub struct Transform {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

pub struct RenderObject {
    pub transform: Transform,
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
}

pub struct RenderInput {
    pub camera: Camera,
    pub objects: Vec<RenderObject>,
}

pub struct Renderer {
    gl: Arc<glow::Context>,
    shaders: HashMap<ShaderId, ShaderProgram>,
    meshes: HashMap<MeshId, Mesh>,
    materials: HashMap<MaterialId, ShaderId>,

    // Pre-allocated scratch buffers to avoid per-frame heap allocations
    mvp_matrices: Vec<Mat4>,
    draw_indices: Vec<usize>,

    next_mesh_id: u32,
    next_shader_id: u32,
    next_material_id: u32,
}

impl Renderer {
    pub fn new(gl: glow::Context) -> Self {
        let gl = Arc::new(gl);

        unsafe {
            // Default pipeline state
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
        }

        Self {
            gl: gl.clone(),
            shaders: HashMap::new(),
            meshes: HashMap::new(),
            materials: HashMap::new(),
            mvp_matrices: Vec::with_capacity(1024),
            draw_indices: Vec::with_capacity(1024),
            next_mesh_id: 0,
            next_shader_id: 0,
            next_material_id: 0,
        }
    }

    pub fn load_mesh(&mut self, data: MeshData) -> MeshId {
        let id = MeshId(self.next_mesh_id);
        self.next_mesh_id += 1;
        let mesh = Mesh::new(self.gl.clone(), &data);
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

    pub fn render(&mut self, input: &RenderInput) {
        unsafe {
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        let view = math::camera_to_view_matrix(&input.camera);
        let proj = math::camera_to_projection_matrix(&input.camera, input.camera.aspect_ratio);
        let vp = proj * view;

        // 1. Reuse pre-allocated scratch memory
        self.mvp_matrices.clear();
        self.draw_indices.clear();
        self.draw_indices.extend(0..input.objects.len());

        // 2. Sort objects by material to BATCH draw calls (minimizes shader state changes)
        self.draw_indices
            .sort_by_key(|&i| input.objects[i].material_id);

        let mut current_material_id = None;

        for &idx in &self.draw_indices {
            let obj = &input.objects[idx];

            // 3. Bind shader ONLY if the material changed
            if current_material_id != Some(obj.material_id) {
                if let Some(&shader_id) = self.materials.get(&obj.material_id) {
                    if let Some(shader) = self.shaders.get(&shader_id) {
                        unsafe {
                            self.gl.use_program(Some(shader.program));
                        }
                        current_material_id = Some(obj.material_id);
                    }
                }
            }

            // 4. Compute MVP
            let model = math::transform_to_model_matrix(&obj.transform);
            let mvp = vp * model;
            self.mvp_matrices.push(mvp); // Push to pre-allocated buffer

            // 5. Bind mesh and draw
            if let Some(mesh) = self.meshes.get(&obj.mesh_id) {
                unsafe {
                    self.gl.bind_vertex_array(Some(mesh.vao));

                    // Set MVP uniform (Shader is already bound)
                    if let Some(&shader_id) = self.materials.get(&obj.material_id) {
                        if let Some(shader) = self.shaders.get(&shader_id) {
                            shader.set_mvp(&mvp);
                        }
                    }

                    self.gl
                        .draw_elements(glow::TRIANGLES, mesh.index_count, glow::UNSIGNED_INT, 0);
                }
            }
        }

        // Clean up state
        unsafe {
            self.gl.bind_vertex_array(None);
            self.gl.use_program(None);
        }
    }
}
