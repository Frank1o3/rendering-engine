/// Frame scratch buffers for non-allocating per-frame rendering.
///
/// These buffers are allocated once and reused across frames to avoid
/// the allocator pressure of recreating them every frame.
use crate::render::draw_indirect::DrawElementsIndirectCommand;
use crate::render::pipeline::PipelineStateId;
use crate::render::scene::SortedInstance;

/// Container for reusable scratch buffers used during a single frame.
///
/// The renderer clears these at the start of each frame and refills them
/// with temporary data during the render pass. This avoids repeated allocations
/// and reduces GC-like pressure on the allocator.
pub struct FrameScratchBuffers {
    /// Instances sorted by (material_id, mesh_id).
    pub sorted_instances: Vec<SortedInstance>,

    /// Indices into `sorted_instances` for instances that passed culling.
    pub visible_indices: Vec<usize>,

    /// Indirect draw commands built during rendering.
    pub indirect_cmds: Vec<DrawElementsIndirectCommand>,

    /// Pending draws grouped by pipeline state.
    pub pending_draws: Vec<(PipelineStateId, DrawElementsIndirectCommand)>,
}

impl FrameScratchBuffers {
    /// Create scratch buffers with conservative pre-allocation.
    pub fn new(capacity: usize) -> Self {
        Self {
            sorted_instances: Vec::with_capacity(capacity),
            visible_indices: Vec::with_capacity(capacity),
            indirect_cmds: Vec::with_capacity(capacity / 4),
            pending_draws: Vec::with_capacity(capacity / 4),
        }
    }

    /// Clear all buffers for reuse in the next frame.
    pub fn clear(&mut self) {
        self.sorted_instances.clear();
        self.visible_indices.clear();
        self.indirect_cmds.clear();
        self.pending_draws.clear();
    }

    /// Check if all buffers are currently empty.
    pub fn is_empty(&self) -> bool {
        self.sorted_instances.is_empty()
            && self.visible_indices.is_empty()
            && self.indirect_cmds.is_empty()
            && self.pending_draws.is_empty()
    }

    /// Reserve additional capacity if needed, without reallocating if already sufficient.
    pub fn reserve(&mut self, capacity: usize) {
        self.sorted_instances.reserve(capacity);
        self.visible_indices.reserve(capacity);
        self.indirect_cmds.reserve(capacity / 4);
        self.pending_draws.reserve(capacity / 4);
    }
}

impl Default for FrameScratchBuffers {
    fn default() -> Self {
        Self::new(1024)
    }
}
