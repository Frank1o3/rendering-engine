use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct ShaderId(pub u32);

pub struct ShaderProgram {
    gl: Arc<glow::Context>,
    pub program: glow::Program,
    loc_vp: Option<glow::UniformLocation>,
}

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

pub struct ShaderManager {
    shaders: HashMap<ShaderId, ShaderProgram>,
    next_id: u32,
}

impl ShaderManager {
    pub fn new() -> Self {
        Self {
            shaders: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn load_shader(
        &mut self,
        gl: Arc<glow::Context>,
        vs: &str,
        gs: Option<&str>,
        fs: &str,
    ) -> ShaderId {
        let id = ShaderId(self.next_id);
        self.next_id += 1;
        let shader = ShaderProgram::new(gl, vs, gs, fs).expect("Failed to compile shader");
        self.shaders.insert(id, shader);
        id
    }

    pub fn load_shader_from_files(
        &mut self,
        gl: Arc<glow::Context>,
        vs_path: &Path,
        gs_path: Option<&Path>,
        fs_path: &Path,
    ) -> Result<ShaderId, String> {
        let id = ShaderId(self.next_id);
        self.next_id += 1;
        let shader = ShaderProgram::from_files(gl, vs_path, gs_path, fs_path)?;
        self.shaders.insert(id, shader);
        Ok(id)
    }

    pub fn load_shaders_from_dir(
        &mut self,
        gl: Arc<glow::Context>,
        dir: &Path,
    ) -> Result<HashMap<String, ShaderId>, String> {
        let mut loaded = HashMap::new();

        if !dir.is_dir() {
            return Err(format!("Shader directory {:?} does not exist.", dir));
        }

        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();

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
                    match self.load_shader_from_files(gl.clone(), &path, gs, &frag_path) {
                        Ok(id) => {
                            log::info!(
                                "Loaded shader: '{}'{}",
                                stem,
                                if gs.is_some() {
                                    " (+ geometry stage)"
                                } else {
                                    ""
                                }
                            );
                            loaded.insert(stem, id);
                        }
                        Err(e) => {
                            log::error!("Failed to compile shader '{}': {}", stem, e);
                            return Err(e);
                        }
                    }
                } else {
                    log::warn!("Found {}.vert but no matching .frag — skipped.", stem);
                }
            }
        }

        Ok(loaded)
    }

    pub fn load_shaders_from_include_dir(
        &mut self,
        gl: Arc<glow::Context>,
        dir: &include_dir::Dir,
    ) -> Result<HashMap<String, ShaderId>, String> {
        use include_dir::DirEntry;

        let mut loaded = HashMap::new();

        for entry in dir.entries() {
            let file = match entry {
                DirEntry::File(file) => file,
                DirEntry::Dir(_) => continue,
            };

            let path = file.path();

            if path.extension().and_then(|e| e.to_str()) != Some("vert") {
                continue;
            }

            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| "Invalid shader filename".to_string())?
                .to_string();

            let vert = file
                .contents_utf8()
                .ok_or_else(|| format!("{} is not UTF-8", path.display()))?;

            let frag_name = format!("{stem}.frag");
            let geom_name = format!("{stem}.geom");

            let frag = dir
                .get_file(&frag_name)
                .ok_or_else(|| format!("Missing fragment shader for '{stem}'"))?
                .contents_utf8()
                .ok_or_else(|| format!("{frag_name} is not UTF-8"))?;

            let geom = dir
                .get_file(&geom_name)
                .map(|f| {
                    f.contents_utf8()
                        .ok_or_else(|| format!("{geom_name} is not UTF-8"))
                })
                .transpose()?;

            let id = self.load_shader(gl.clone(), vert, geom, frag);

            log::info!(
                "Loaded shader: '{}'{}",
                stem,
                if geom.is_some() {
                    " (+ geometry stage)"
                } else {
                    ""
                }
            );

            loaded.insert(stem, id);
        }

        Ok(loaded)
    }

    pub fn get(&self, id: ShaderId) -> Option<&ShaderProgram> {
        self.shaders.get(&id)
    }
}
