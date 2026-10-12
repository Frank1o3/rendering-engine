use super::*;

impl Renderer {
    pub fn frame_stats(&self) -> FrameStats {
        self.frame_stats
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        unsafe {
            self.gl.viewport(0, 0, width, height);
        }
    }

    pub fn default_pass_order() -> Vec<PassGroup> {
        let mut passes = vec![
            PassGroup {
                pass_type: PassType::Skybox,
                index: 0,
            },
            PassGroup {
                pass_type: PassType::Opaque,
                index: 1,
            },
            PassGroup {
                pass_type: PassType::UI,
                index: 2,
            },
        ];
        passes.sort();
        passes
    }

    pub fn apply_pass_config(&self, pass: &PassConfig) {
        let viewport = pass.viewport.unwrap_or((0, 0, self.width, self.height));
        unsafe {
            self.gl
                .viewport(viewport.0, viewport.1, viewport.2, viewport.3);
            if pass.depth_test {
                self.gl.enable(glow::DEPTH_TEST);
            } else {
                self.gl.disable(glow::DEPTH_TEST);
            }
        }

        if let Some(color) = pass.clear_color {
            unsafe {
                self.gl.clear_color(color.x, color.y, color.z, color.w);
            }
        }

        if pass.clear_flags.color || pass.clear_flags.depth || pass.clear_flags.stencil {
            unsafe {
                // OpenGL applies the depth write mask to depth clears too.
                // Clear with writes enabled, then restore this pass's setting.
                if pass.clear_flags.depth {
                    self.gl.depth_mask(true);
                }
                self.gl.clear(pass.clear_flags.to_gl_bits());
                self.gl.depth_mask(pass.depth_write);
            }
        } else {
            unsafe {
                self.gl.depth_mask(pass.depth_write);
            }
        }
    }
}
