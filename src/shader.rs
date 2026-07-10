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
    /// Compiles from raw source strings. `gs_src` is optional — pass `None`
    /// for a standard vertex+fragment program.
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

            let cleanup = |gl: &glow::Context| {
                gl.delete_shader(vs);
                if let Some(gs) = gs {
                    gl.delete_shader(gs);
                }
                gl.delete_shader(fs);
            };

            if !gl.get_program_link_status(program) {
                let log = gl.get_program_info_log(program);
                gl.delete_program(program);
                cleanup(&gl);
                return Err(format!("Shader Link Error:\n{}", log));
            }

            cleanup(&gl);

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

    pub fn set_vp(&self, vp: &glam::Mat4) {
        unsafe {
            self.gl
                .uniform_matrix_4_f32_slice(self.loc_vp.as_ref(), false, &vp.to_cols_array());
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
