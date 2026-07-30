use glow::HasContext;
use std::sync::Arc;

pub struct ComputeShader {
    gl: Arc<glow::Context>,
    pub program: glow::Program,
}

impl ComputeShader {
    pub fn new(gl: Arc<glow::Context>, src: &str) -> Result<Self, String> {
        unsafe {
            let shader = gl.create_shader(glow::COMPUTE_SHADER).map_err(|e| e.to_string())?;
            gl.shader_source(shader, src);
            gl.compile_shader(shader);

            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                return Err(format!("Compute Shader Compile Error:\n{}", log));
            }

            let program = gl.create_program().map_err(|e| e.to_string())?;
            gl.attach_shader(program, shader);
            gl.link_program(program);
            gl.delete_shader(shader);

            if !gl.get_program_link_status(program) {
                let log = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(format!("Compute Shader Link Error:\n{}", log));
            }

            Ok(Self { gl, program })
        }
    }

    pub fn dispatch(&self, num_groups_x: u32, num_groups_y: u32, num_groups_z: u32) {
        unsafe {
            self.gl.use_program(Some(self.program));
            self.gl.dispatch_compute(num_groups_x, num_groups_y, num_groups_z);
        }
    }

    pub fn memory_barrier(&self, barriers: u32) {
        unsafe {
            self.gl.memory_barrier(barriers);
        }
    }
}

impl Drop for ComputeShader {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
        }
    }
}
