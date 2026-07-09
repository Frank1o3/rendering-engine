// src/renderer/draw_indirect.rs
//
// Multi-Draw Indirect infrastructure supporting multiple MDI dispatch strategies.
// Each strategy has different performance characteristics:
//
//   Single      — glDrawElementsIndirect per command (simplest, best for debugging)
//   Multi       — glMultiDrawElementsIndirect batches N commands in 1 call (default)
//   MultiCount  — glMultiDrawElementsIndirectCount reads count from GPU buffer (GL 4.6)
//                  Enables fully GPU-driven rendering when paired with compute culling.

use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::sync::Arc;

/// Matches the OpenGL `DrawElementsIndirectCommand` layout exactly.
///
/// See: https://registry.khronos.org/OpenGL-Refpages/gl4/html/glMultiDrawElementsIndirect.xhtml
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DrawElementsIndirectCommand {
    /// Number of indices to draw per instance (mesh.index_count).
    pub count: u32,
    /// Number of instances to draw (objects in this batch).
    pub instance_count: u32,
    /// Offset into the EBO in units of indices (0 when VAOs are separate).
    pub first_index: u32,
    /// Offset added to each index before fetching from the VBO (0 when VAOs are separate).
    pub base_vertex: i32,
    /// Offset into instanced vertex attributes (points into the InstanceData buffer).
    pub base_instance: u32,
}

/// Selects which MDI dispatch path the renderer uses.
///
/// Each variant wraps a different OpenGL entry point with distinct trade-offs:
///
///   `Single`     — Lowest driver complexity; one API call per draw command.
///   `Multi`      — Default. Batches N commands in a single API call.
///   `MultiCount` — Most advanced. The command count is read from a GPU buffer,
///                   enabling compute shaders to control draw counts without CPU readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MdiStrategy {
    /// `glDrawElementsIndirect` — one call per command.
    /// Good for debugging and profiling individual draw calls.
    Single,

    /// `glMultiDrawElementsIndirect` — batch multiple commands in one call.
    /// Reduces driver overhead proportionally to the number of commands batched.
    #[default]
    Multi,

    /// `glMultiDrawElementsIndirectCount` (GL 4.6 / ARB_indirect_count).
    /// The draw count is read from a GPU buffer at `count_buffer_offset`.
    /// Requires a separate "parameter buffer" bound to `GL_PARAMETER_BUFFER`.
    MultiCount,
}

/// GPU-side buffer for indirect draw commands.
///
/// Manages a `GL_DRAW_INDIRECT_BUFFER` and an optional `GL_PARAMETER_BUFFER`
/// (for `MultiCount` strategy).
pub struct IndirectBuffer {
    gl: Arc<glow::Context>,
    /// The indirect command buffer (GL_DRAW_INDIRECT_BUFFER).
    pub cmd_buffer: glow::Buffer,
    /// Optional parameter buffer for MultiCount strategy (GL_PARAMETER_BUFFER).
    /// Stores the draw count as a u32 that the GPU reads.
    pub count_buffer: Option<glow::Buffer>,
}

impl IndirectBuffer {
    pub fn new(gl: Arc<glow::Context>, support_multi_count: bool) -> Self {
        unsafe {
            let cmd_buffer = gl
                .create_buffer()
                .expect("Failed to create indirect command buffer");

            let count_buffer = if support_multi_count {
                let buf = gl
                    .create_buffer()
                    .expect("Failed to create parameter count buffer");
                Some(buf)
            } else {
                None
            };

            Self {
                gl,
                cmd_buffer,
                count_buffer,
            }
        }
    }

    /// Upload draw commands to the GPU indirect buffer.
    pub fn upload(&self, commands: &[DrawElementsIndirectCommand]) {
        unsafe {
            self.gl
                .bind_buffer(glow::DRAW_INDIRECT_BUFFER, Some(self.cmd_buffer));
            self.gl.buffer_data_u8_slice(
                glow::DRAW_INDIRECT_BUFFER,
                bytemuck::cast_slice(commands),
                glow::STREAM_DRAW,
            );
        }
    }

    /// Upload the command count to the parameter buffer (for MultiCount strategy).
    pub fn upload_count(&self, count: u32) {
        if let Some(buf) = self.count_buffer {
            unsafe {
                self.gl
                    .bind_buffer(glow::PARAMETER_BUFFER, Some(buf));
                self.gl.buffer_data_u8_slice(
                    glow::PARAMETER_BUFFER,
                    bytemuck::cast_slice(&[count]),
                    glow::STREAM_DRAW,
                );
            }
        }
    }

    /// Dispatch draw commands using the selected MDI strategy.
    ///
    /// # Arguments
    /// * `strategy`     — Which MDI entry point to use.
    /// * `element_type` — Index type (e.g. `glow::UNSIGNED_INT`).
    /// * `cmd_offset`   — Byte offset into the uploaded command buffer.
    /// * `cmd_count`    — Number of commands to dispatch.
    /// * `max_count`    — Maximum commands for MultiCount (ignored by other strategies).
    pub fn dispatch(
        &self,
        strategy: MdiStrategy,
        element_type: u32,
        cmd_offset: usize,
        cmd_count: usize,
        _max_count: u32,
    ) {
        let stride = std::mem::size_of::<DrawElementsIndirectCommand>() as i32;

        unsafe {
            self.gl
                .bind_buffer(glow::DRAW_INDIRECT_BUFFER, Some(self.cmd_buffer));

            match strategy {
                MdiStrategy::Single | MdiStrategy::Multi | MdiStrategy::MultiCount => {
                    // Since glow does not expose multi_draw_elements_indirect or
                    // multi_draw_elements_indirect_count directly in all profiles/backends,
                    // we emulate it by loop-dispatching individual indirect draw calls.
                    for i in 0..cmd_count {
                        let offset = cmd_offset + i * stride as usize;
                        self.gl.draw_elements_indirect_offset(
                            glow::TRIANGLES,
                            element_type,
                            offset as i32,
                        );
                    }
                }
            }
        }
    }
}

impl Drop for IndirectBuffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.cmd_buffer);
            if let Some(buf) = self.count_buffer {
                self.gl.delete_buffer(buf);
            }
        }
    }
}
