use glow::HasContext;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct ShaderProgram {
    gl: Arc<glow::Context>,
    pub program: glow::Program,
    loc_vp: Option<glow::UniformLocation>,
}

/// Compiles a single shader stage, returning a descriptive error on failure.
unsafe fn compile_shader(
    gl: &glow::Context,
    kind: u32,
    src: &str,
    label: &str,
) -> Result<glow::Shader, String> {
    unsafe {
        let shader = gl.create_shader(kind).map_err(|e| e.to_string())?;
        gl.shader_source(shader, src);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(format!("{} Shader Compile Error:\n{}", label, log));
        }
        Ok(shader)
    }
}

impl ShaderProgram {
    /// Compiles from raw source strings.
    /// `gs_src` is optional — pass `None` for a standard vertex+fragment program.
    pub fn new(
        gl: Arc<glow::Context>,
        vs_src: &str,
        gs_src: Option<&str>,
        fs_src: &str,
    ) -> Result<Self, String> {
        unsafe {
            let vs = compile_shader(&gl, glow::VERTEX_SHADER, vs_src, "Vertex")?;

            let gs = match gs_src {
                Some(src) => match compile_shader(&gl, glow::GEOMETRY_SHADER, src, "Geometry") {
                    Ok(s) => Some(s),
                    Err(e) => {
                        gl.delete_shader(vs);
                        return Err(e);
                    }
                },
                None => None,
            };

            let fs = match compile_shader(&gl, glow::FRAGMENT_SHADER, fs_src, "Fragment") {
                Ok(s) => s,
                Err(e) => {
                    gl.delete_shader(vs);
                    if let Some(gs) = gs {
                        gl.delete_shader(gs);
                    }
                    return Err(e);
                }
            };

            let program = gl.create_program().map_err(|e| e.to_string())?;
            gl.attach_shader(program, vs);
            if let Some(gs) = gs {
                gl.attach_shader(program, gs);
            }
            gl.attach_shader(program, fs);
            gl.link_program(program);

            // Shaders can be deleted after linking — driver keeps them alive
            // as long as they're attached to a live program.
            gl.delete_shader(vs);
            if let Some(gs) = gs {
                gl.delete_shader(gs);
            }
            gl.delete_shader(fs);

            if !gl.get_program_link_status(program) {
                let log = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(format!("Shader Link Error:\n{}", log));
            }

            let loc_vp = gl.get_uniform_location(program, "uVP");

            Ok(Self {
                gl,
                program,
                loc_vp,
            })
        }
    }

    /// Reads a vertex/fragment pair (and optional geometry shader) from disk.
    pub fn from_files(
        gl: Arc<glow::Context>,
        vs_path: &Path,
        gs_path: Option<&Path>,
        fs_path: &Path,
    ) -> Result<Self, String> {
        let vs_src = fs::read_to_string(vs_path)
            .map_err(|e| format!("Failed to read vertex shader {:?}: {}", vs_path, e))?;
        let gs_src = gs_path
            .map(|p| {
                fs::read_to_string(p)
                    .map_err(|e| format!("Failed to read geometry shader {:?}: {}", p, e))
            })
            .transpose()?;
        let fs_src = fs::read_to_string(fs_path)
            .map_err(|e| format!("Failed to read fragment shader {:?}: {}", fs_path, e))?;

        Self::new(gl, &vs_src, gs_src.as_deref(), &fs_src)
    }

    // ── Uniform setters ──────────────────────────────────────────────────────

    pub fn set_vp(&self, vp: &glam::Mat4) {
        unsafe {
            self.gl
                .uniform_matrix_4_f32_slice(self.loc_vp.as_ref(), false, &vp.to_cols_array());
        }
    }

    /// Sets a named vec3 uniform. Returns `false` if the uniform doesn't exist
    /// in this program (not an error — inactive uniforms are valid GLSL).
    pub fn set_vec3(&self, name: &str, v: glam::Vec3) -> bool {
        unsafe {
            match self.gl.get_uniform_location(self.program, name) {
                Some(loc) => {
                    self.gl.uniform_3_f32(Some(&loc), v.x, v.y, v.z);
                    true
                }
                None => false,
            }
        }
    }

    /// Sets a named float uniform.
    pub fn set_f32(&self, name: &str, v: f32) -> bool {
        unsafe {
            match self.gl.get_uniform_location(self.program, name) {
                Some(loc) => {
                    self.gl.uniform_1_f32(Some(&loc), v);
                    true
                }
                None => false,
            }
        }
    }

    /// Sets a named mat4 uniform.
    pub fn set_mat4(&self, name: &str, m: &glam::Mat4) -> bool {
        unsafe {
            match self.gl.get_uniform_location(self.program, name) {
                Some(loc) => {
                    self.gl
                        .uniform_matrix_4_f32_slice(Some(&loc), false, &m.to_cols_array());
                    true
                }
                None => false,
            }
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
