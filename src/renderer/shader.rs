use glam::Mat4;
use glow::HasContext;

pub struct ShaderProgram {
    gl: glow::Context,
    pub program: glow::Program,
    loc_mvp: Option<glow::UniformLocation>, // Cached uniform location
}

impl ShaderProgram {
    pub fn new(gl: &glow::Context, vs_src: &str, fs_src: &str) -> Result<Self, String> {
        unsafe {
            let vs = gl
                .create_shader(glow::VERTEX_SHADER)
                .map_err(|e| e.to_string())?;
            gl.shader_source(vs, vs_src);
            gl.compile_shader(vs);
            if !gl.get_shader_compile_status(vs) {
                return Err(gl.get_shader_info_log(vs));
            }

            let fs = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .map_err(|e| e.to_string())?;
            gl.shader_source(fs, fs_src);
            gl.compile_shader(fs);
            if !gl.get_shader_compile_status(fs) {
                return Err(gl.get_shader_info_log(fs));
            }

            let program = gl.create_program().map_err(|e| e.to_string())?;
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);

            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }

            gl.delete_shader(vs);
            gl.delete_shader(fs);

            // Cache the uniform location ONCE
            let loc_mvp = gl.get_uniform_location(program, "uMVP");

            Ok(Self {
                gl: gl.clone(),
                program,
                loc_mvp,
            })
        }
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
