use glam::Mat4;
use glow::HasContext;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct ShaderProgram {
    gl: Arc<glow::Context>,
    pub program: glow::Program,
    loc_mvp: Option<glow::UniformLocation>,
}

impl ShaderProgram {
    /// Compiles from raw source strings (kept for internal/procedural use)
    pub fn new(gl: Arc<glow::Context>, vs_src: &str, fs_src: &str) -> Result<Self, String> {
        unsafe {
            let vs = gl
                .create_shader(glow::VERTEX_SHADER)
                .map_err(|e| e.to_string())?;
            gl.shader_source(vs, vs_src);
            gl.compile_shader(vs);
            if !gl.get_shader_compile_status(vs) {
                let log = gl.get_shader_info_log(vs);
                gl.delete_shader(vs);
                return Err(format!("Vertex Shader Compile Error:\n{}", log));
            }

            let fs = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .map_err(|e| e.to_string())?;
            gl.shader_source(fs, fs_src);
            gl.compile_shader(fs);
            if !gl.get_shader_compile_status(fs) {
                let log = gl.get_shader_info_log(fs);
                gl.delete_shader(vs);
                gl.delete_shader(fs);
                return Err(format!("Fragment Shader Compile Error:\n{}", log));
            }

            let program = gl.create_program().map_err(|e| e.to_string())?;
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);

            if !gl.get_program_link_status(program) {
                let log = gl.get_program_info_log(program);
                gl.delete_program(program);
                gl.delete_shader(vs);
                gl.delete_shader(fs);
                return Err(format!("Shader Link Error:\n{}", log));
            }

            gl.delete_shader(vs);
            gl.delete_shader(fs);

            let loc_mvp = gl.get_uniform_location(program, "uMVP");

            Ok(Self {
                gl,
                program,
                loc_mvp,
            })
        }
    }

    /// Reads vertex and fragment shaders from file paths and compiles them
    pub fn from_files(
        gl: Arc<glow::Context>,
        vs_path: &Path,
        fs_path: &Path,
    ) -> Result<Self, String> {
        let vs_src = fs::read_to_string(vs_path)
            .map_err(|e| format!("Failed to read vertex shader {:?}: {}", vs_path, e))?;
        let fs_src = fs::read_to_string(fs_path)
            .map_err(|e| format!("Failed to read fragment shader {:?}: {}", fs_path, e))?;

        Self::new(gl, &vs_src, &fs_src)
    }

    // Assumes the program is already bound! (Minimizes state changes)
    pub fn set_mvp(&self, mvp: &Mat4) {
        unsafe {
            self.gl
                .uniform_matrix_4_f32_slice(self.loc_mvp.as_ref(), false, &mvp.to_cols_array());
        }
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
        }
    }
}
