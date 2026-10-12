use super::ShaderProgram;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct ShaderId(pub u32);
pub struct ShaderManager {
    shaders: HashMap<ShaderId, ShaderProgram>,
    next_id: u32,
}

impl Default for ShaderManager {
    fn default() -> Self {
        Self::new()
    }
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

    /// Recompile a file-backed shader and replace it only if compilation and
    /// linking succeed. The existing program remains usable after an error.
    pub fn reload_shader_from_files(
        &mut self,
        id: ShaderId,
        gl: Arc<glow::Context>,
        vs_path: &Path,
        gs_path: Option<&Path>,
        fs_path: &Path,
    ) -> Result<(), String> {
        if !self.shaders.contains_key(&id) {
            return Err(format!("Cannot reload unknown shader id {}", id.0));
        }

        let replacement = ShaderProgram::from_files(gl, vs_path, gs_path, fs_path)?;
        self.shaders.insert(id, replacement);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_manager_default_is_empty() {
        let manager = ShaderManager::default();
        assert!(manager.shaders.is_empty());
        assert_eq!(manager.next_id, 0);
    }
}
