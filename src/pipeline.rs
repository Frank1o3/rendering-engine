// src/renderer/pipeline.rs
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Unique identifier for a pipeline state object
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PipelineStateId(pub u64);

impl From<u64> for PipelineStateId {
    fn from(value: u64) -> Self {
        PipelineStateId(value)
    }
}

/// Describes the complete OpenGL pipeline state for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineState {
    pub shader_id: u32,
    pub cull_mode: CullMode,
    pub depth_test: bool,
    pub depth_write: bool,
    pub depth_func: DepthFunc,
    pub blend_enabled: bool,
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
}

/// Face culling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    Back,
}

/// Depth comparison function
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthFunc {
    Never,
    Less,
    Equal,
    LessOrEqual,
    Greater,
    NotEqual,
    GreaterOrEqual,
    Always,
}

impl DepthFunc {
    pub fn to_glow(&self) -> u32 {
        match self {
            DepthFunc::Never => glow::NEVER,
            DepthFunc::Less => glow::LESS,
            DepthFunc::Equal => glow::EQUAL,
            DepthFunc::LessOrEqual => glow::LEQUAL,
            DepthFunc::Greater => glow::GREATER,
            DepthFunc::NotEqual => glow::NOTEQUAL,
            DepthFunc::GreaterOrEqual => glow::GEQUAL,
            DepthFunc::Always => glow::ALWAYS,
        }
    }
}

/// Blend factor for alpha blending
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColor,
    OneMinusSrcColor,
    DstColor,
    OneMinusDstColor,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
}

impl BlendFactor {
    pub fn to_glow(&self) -> u32 {
        match self {
            BlendFactor::Zero => glow::ZERO,
            BlendFactor::One => glow::ONE,
            BlendFactor::SrcColor => glow::SRC_COLOR,
            BlendFactor::OneMinusSrcColor => glow::ONE_MINUS_SRC_COLOR,
            BlendFactor::DstColor => glow::DST_COLOR,
            BlendFactor::OneMinusDstColor => glow::ONE_MINUS_DST_COLOR,
            BlendFactor::SrcAlpha => glow::SRC_ALPHA,
            BlendFactor::OneMinusSrcAlpha => glow::ONE_MINUS_SRC_ALPHA,
            BlendFactor::DstAlpha => glow::DST_ALPHA,
            BlendFactor::OneMinusDstAlpha => glow::ONE_MINUS_DST_ALPHA,
        }
    }
}

impl PipelineState {
    /// Creates a default pipeline state suitable for opaque 3D rendering
    pub fn default_opaque(shader_id: u32) -> Self {
        Self {
            shader_id,
            cull_mode: CullMode::Back, // Cull the BACK faces, not the front!
            depth_test: true,
            depth_write: true,
            depth_func: DepthFunc::Less,
            blend_enabled: false, // Opaque objects should NOT have blending enabled
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
        }
    }

    /// Creates a pipeline state suitable for UI/transparent rendering
    pub fn default_alpha(shader_id: u32) -> Self {
        Self {
            shader_id,
            cull_mode: CullMode::None,
            depth_test: true,
            depth_write: false,
            depth_func: DepthFunc::Less,
            blend_enabled: true,
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
        }
    }

    /// Computes a hash of this pipeline state for use as a cache key
    pub fn hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.shader_id.hash(&mut hasher);
        self.cull_mode.hash(&mut hasher);
        self.depth_test.hash(&mut hasher);
        self.depth_write.hash(&mut hasher);
        self.depth_func.hash(&mut hasher);
        self.blend_enabled.hash(&mut hasher);
        self.src_factor.hash(&mut hasher);
        self.dst_factor.hash(&mut hasher);
        hasher.finish()
    }
}

/// Cache that maps pipeline state hashes to their IDs.
/// The PipelineStateId is simply the hash of the state, ensuring O(1) lookups
/// and automatically deduplicating identical states.
pub struct PipelineCache {
    states: HashMap<u64, PipelineState>,
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Registers a pipeline state and returns its ID.
    /// If an identical state already exists, returns the existing ID.
    pub fn register(&mut self, state: PipelineState) -> PipelineStateId {
        let hash = state.hash();
        self.states.entry(hash).or_insert(state);
        PipelineStateId(hash) // The ID is just the full hash!
    }

    /// Retrieves a pipeline state by its hash
    pub fn get(&self, hash: u64) -> Option<&PipelineState> {
        self.states.get(&hash)
    }

    /// Retrieves a pipeline state by its ID
    pub fn get_by_id(&self, id: PipelineStateId) -> Option<&PipelineState> {
        self.states.get(&id.0) // Correctly looks up the full u64 hash
    }
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}
