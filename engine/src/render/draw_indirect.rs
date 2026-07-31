// src/renderer/draw_indirect.rs
//
// Multi-Draw Indirect infrastructure supporting multiple MDI dispatch strategies.
// Each strategy has different performance characteristics:
//
//   Single      — glDrawElementsIndirect per command (simplest, best for debugging)
//   Multi       — loop of glDrawElementsIndirect (emulated; glow lacks the true MDI entry point)
//   MultiCount  — same emulation but also binds a count buffer (ready for a future glow update)
//
// NOTE on glow and glMultiDrawElementsIndirect:
//   As of glow 0.17, `multi_draw_elements_indirect` is not exposed on all backends.
//   The `Multi` and `MultiCount` variants therefore emulate the batch via a loop of
//   `draw_elements_indirect_offset` calls. The MDI command buffer is still uploaded to
//   GL_DRAW_INDIRECT_BUFFER as a contiguous array — so a future upgrade that exposes the
//   real entry point only needs to change the `dispatch` call here, not the upload path.
//
// The command buffer uses persistent mapping (buffer_storage + MAP_PERSISTENT_BIT) to
// avoid per-frame buffer_data_u8_slice re-allocation. Same pattern as PersistentMappedBuffer
// in core/buffer.rs.

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
    /// Offset into the shared EBO in units of indices.
    /// Set from `MeshRange::first_index` — non-zero when using GeometryPool.
    pub first_index: u32,
    /// Added to every index before fetching from the VBO.
    /// Set from `MeshRange::base_vertex` — non-zero when using GeometryPool.
    pub base_vertex: i32,
    /// Byte offset into the instanced vertex-attribute buffer where this
    /// draw's instance data starts. Equals the object's slot in the
    /// persistent-mapped transform buffer.
    pub base_instance: u32,
}

/// Selects which MDI dispatch path the renderer uses.
///
/// All three variants upload draw commands to the same `GL_DRAW_INDIRECT_BUFFER`.
/// The difference is only in how the GPU reads the command count:
///
///   `Single`     — Issues one `glDrawElementsIndirect` call per command. Best for
///                   debugging because each call shows up individually in a frame capture.
///   `Multi`      — Loops `glDrawElementsIndirect` once per command but processes the
///                   GPU buffer sequentially (same net effect as the real MDI, minus the
///                   potential driver batching benefit). Use this by default.
///   `MultiCount` — Like `Multi`, but also uploads the draw count to a
///                   `GL_PARAMETER_BUFFER` so it is in the right shape for a future
///                   `glMultiDrawElementsIndirectCount` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MdiStrategy {
    /// `glDrawElementsIndirect` — one GL call per command.
    /// Good for RenderDoc debugging: each draw shows as a separate event.
    Single,

    /// Loop of `glDrawElementsIndirect` — emulates the batch.
    /// Identical results to `Single` but the intent is one logical batch.
    #[default]
    Multi,

    /// Like `Multi` but also writes the draw count to a `GL_PARAMETER_BUFFER`.
    /// Keeps the infrastructure ready for `glMultiDrawElementsIndirectCount`
    /// once glow exposes it.
    MultiCount,
}

/// Maximum number of indirect draw commands the persistently-mapped buffer can hold.
/// 4096 commands × 20 bytes = 80 KiB — plenty for thousands of chunks.
const MAX_INDIRECT_COMMANDS: usize = 4096;

/// GPU-side buffer for indirect draw commands.
///
/// Uses persistent mapping (`buffer_storage` + `MAP_PERSISTENT_BIT`) to eliminate
/// per-frame `buffer_data_u8_slice` re-allocation. Commands are written directly
/// into the mapped region via pointer copy.
///
/// Manages a `GL_DRAW_INDIRECT_BUFFER` and an optional `GL_PARAMETER_BUFFER`
/// (for `MultiCount` strategy).
pub struct IndirectBuffer {
    gl: Arc<glow::Context>,
    /// The indirect command buffer (GL_DRAW_INDIRECT_BUFFER).
    pub cmd_buffer: glow::Buffer,
    /// CPU-visible pointer into the persistently-mapped command buffer.
    cmd_ptr: *mut DrawElementsIndirectCommand,
    /// Capacity of the mapped region in number of commands.
    cmd_capacity: usize,
    /// Optional parameter buffer for MultiCount strategy (GL_PARAMETER_BUFFER).
    /// Stores the draw count as a u32 that the GPU reads.
    pub count_buffer: Option<glow::Buffer>,
}

// SAFETY: The mapped pointer is valid for the lifetime of the GL buffer and is
// only accessed from the render thread.
unsafe impl Send for IndirectBuffer {}

impl IndirectBuffer {
    pub fn new(gl: Arc<glow::Context>, support_multi_count: bool) -> Self {
        unsafe {
            let cmd_buffer = gl
                .create_buffer()
                .expect("Failed to create indirect command buffer");

            // Allocate immutable storage with persistent + coherent mapping
            let byte_size = MAX_INDIRECT_COMMANDS * std::mem::size_of::<DrawElementsIndirectCommand>();
            let flags = glow::MAP_WRITE_BIT | glow::MAP_PERSISTENT_BIT | glow::MAP_COHERENT_BIT;

            gl.bind_buffer(glow::DRAW_INDIRECT_BUFFER, Some(cmd_buffer));
            gl.buffer_storage(glow::DRAW_INDIRECT_BUFFER, byte_size as i32, None, flags);

            let cmd_ptr = gl.map_buffer_range(
                glow::DRAW_INDIRECT_BUFFER,
                0,
                byte_size as i32,
                flags,
            ) as *mut DrawElementsIndirectCommand;

            if cmd_ptr.is_null() {
                panic!("Failed to map persistent indirect command buffer");
            }

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
                cmd_ptr,
                cmd_capacity: MAX_INDIRECT_COMMANDS,
                count_buffer,
            }
        }
    }

    /// Write draw commands directly into the persistently-mapped buffer.
    /// Returns the number of commands actually written (clamped to capacity).
    pub fn upload(&self, commands: &[DrawElementsIndirectCommand]) -> usize {
        let count = commands.len().min(self.cmd_capacity);
        if count < commands.len() {
            log::warn!(
                "IndirectBuffer capacity exceeded: {} commands, capacity {}",
                commands.len(),
                self.cmd_capacity
            );
        }
        unsafe {
            std::ptr::copy_nonoverlapping(commands.as_ptr(), self.cmd_ptr, count);
        }
        count
    }

    /// Upload the command count to the parameter buffer (for MultiCount strategy).
    pub fn upload_count(&self, count: u32) {
        if let Some(buf) = self.count_buffer {
            unsafe {
                self.gl.bind_buffer(glow::PARAMETER_BUFFER, Some(buf));
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
    /// * `cmd_offset`   — Byte offset into the uploaded command buffer where
    ///                     the first command to dispatch lives.
    /// * `cmd_count`    — Number of commands to dispatch.
    /// * `max_count`    — Upper bound for MultiCount (ignored by Single/Multi).
    pub fn dispatch(
        &self,
        strategy: MdiStrategy,
        element_type: u32,
        cmd_offset: usize,
        cmd_count: usize,
        max_count: u32,
    ) {
        let cmd_stride = std::mem::size_of::<DrawElementsIndirectCommand>();

        unsafe {
            // Ensure writes to the persistent mapping are visible to the GPU
            self.gl
                .memory_barrier(glow::COMMAND_BARRIER_BIT);
            self.gl
                .bind_buffer(glow::DRAW_INDIRECT_BUFFER, Some(self.cmd_buffer));

            match strategy {
                MdiStrategy::Single => {
                    // One GL call per command — each shows up individually in a frame capture.
                    // This is semantically identical to Multi but easier to debug.
                    for i in 0..cmd_count {
                        let byte_offset = (cmd_offset + i * cmd_stride) as i32;
                        self.gl.draw_elements_indirect_offset(
                            glow::TRIANGLES,
                            element_type,
                            byte_offset,
                        );
                    }
                }

                MdiStrategy::Multi => {
                    // Emulated batch: loop through all commands in the buffer.
                    // When glow exposes multi_draw_elements_indirect this becomes a
                    // single-line replacement:
                    //   self.gl.multi_draw_elements_indirect(
                    //       glow::TRIANGLES, element_type,
                    //       cmd_offset as i32, cmd_count as i32, cmd_stride as i32);
                    for i in 0..cmd_count {
                        let byte_offset = (cmd_offset + i * cmd_stride) as i32;
                        self.gl.draw_elements_indirect_offset(
                            glow::TRIANGLES,
                            element_type,
                            byte_offset,
                        );
                    }
                }

                MdiStrategy::MultiCount => {
                    // Same loop as Multi, but the count buffer is already bound by
                    // upload_count(). When glow exposes the GL 4.6 count variant:
                    //   self.gl.multi_draw_elements_indirect_count(
                    //       glow::TRIANGLES, element_type,
                    //       cmd_offset as i32, 0, max_count as i32, cmd_stride as i32);
                    let _ = max_count; // suppress unused warning until the real call lands
                    if let Some(buf) = self.count_buffer {
                        self.gl.bind_buffer(glow::PARAMETER_BUFFER, Some(buf));
                    }
                    for i in 0..cmd_count {
                        let byte_offset = (cmd_offset + i * cmd_stride) as i32;
                        self.gl.draw_elements_indirect_offset(
                            glow::TRIANGLES,
                            element_type,
                            byte_offset,
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
            self.gl
                .bind_buffer(glow::DRAW_INDIRECT_BUFFER, Some(self.cmd_buffer));
            self.gl.unmap_buffer(glow::DRAW_INDIRECT_BUFFER);
            self.gl.delete_buffer(self.cmd_buffer);
            if let Some(buf) = self.count_buffer {
                self.gl.delete_buffer(buf);
            }
        }
    }
}
