use super::*;

impl Renderer {
    // ── Transform-region synchronization ────────────────────────────────────

    pub(super) fn wait_for_region(&mut self, region: usize) {
        let Some(fence) = self.region_fences[region].take() else {
            return;
        };
        unsafe {
            const TIMEOUT_NS: i32 = 1_000_000_000;
            loop {
                let status =
                    self.gl
                        .client_wait_sync(fence, glow::SYNC_FLUSH_COMMANDS_BIT, TIMEOUT_NS);
                match status {
                    glow::ALREADY_SIGNALED | glow::CONDITION_SATISFIED => break,
                    glow::TIMEOUT_EXPIRED => {
                        log::warn!(
                            "GPU still busy with transform region {} after 1s — waiting again",
                            region
                        );
                        continue;
                    }
                    glow::WAIT_FAILED => {
                        log::error!("client_wait_sync failed for transform region {}", region);
                        break;
                    }
                    _ => break,
                }
            }
            self.gl.delete_sync(fence);
        }
    }

    pub(super) fn fence_region(&mut self, region: usize) {
        unsafe {
            match self.gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0) {
                Ok(fence) => self.region_fences[region] = Some(fence),
                Err(e) => log::error!("fence_sync failed: {}", e),
            }
        }
    }

    pub fn current_frustum(&self) -> Option<[Vec4; 6]> {
        self.last_frustum
    }

    /// Camera position from the most recently consumed frame.
    pub fn current_camera_position(&self) -> glam::Vec3 {
        self.last_frame.camera_position
    }
}
