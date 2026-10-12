/// CPU-side counters from the most recently rendered frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameStats {
    pub input_instances: u64,
    pub submitted_instances: u64,
    pub frustum_culled_instances: u64,
    pub draw_calls: u64,
    pub pipeline_changes: u64,
    pub triangles: u64,
}
