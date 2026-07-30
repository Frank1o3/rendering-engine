use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::sync::Arc;
use crate::resources::shader::{ShaderId, ShaderManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct SkyboxId(pub u32);

pub struct SkyboxPipeline {
    gl: Arc<glow::Context>,
    pub shader_id: ShaderId,
    pub color: glam::Vec3,
    pub enabled: bool,
    dummy_vao: glow::VertexArray,
}

impl SkyboxPipeline {
    pub fn new(gl: Arc<glow::Context>, shader_id: ShaderId) -> Self {
        let dummy_vao = unsafe {
            gl.create_vertex_array()
                .expect("Failed to create skybox dummy VAO")
        };
        Self {
            gl,
            shader_id,
            color: glam::Vec3::ONE,
            enabled: false,
            dummy_vao,
        }
    }

    pub fn draw(&self, shaders: &ShaderManager, inv_vp: &glam::Mat4) {
        if !self.enabled {
            return;
        }

        let Some(shader) = shaders.get(self.shader_id) else {
            return;
        };

        unsafe {
            self.gl.use_program(Some(shader.program));
            shader.set_mat4("uInvVP", inv_vp);
            shader.set_vec3("uSkyColor", self.color);

            self.gl.disable(glow::CULL_FACE);
            self.gl.enable(glow::DEPTH_TEST);
            self.gl.depth_func(glow::LEQUAL);
            self.gl.depth_mask(false);

            self.gl.bind_vertex_array(Some(self.dummy_vao));
            // Fullscreen triangle (3 vertices generated procedurally in skybox vertex shader)
            self.gl.draw_arrays(glow::TRIANGLES, 0, 3);
            self.gl.bind_vertex_array(None);
        }
    }
}

impl Drop for SkyboxPipeline {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_vertex_array(self.dummy_vao);
        }
    }
}
