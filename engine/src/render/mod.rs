pub mod draw_indirect;
pub mod frame_data;
pub mod frame_scratch;
pub mod framebuffer;
pub mod lighting;
mod lighting_buffer;
pub mod pass;
pub mod pipeline;
pub mod renderer;
pub mod scene;
pub mod skybox;
pub mod stats;
pub mod ui;

pub use draw_indirect::{DrawElementsIndirectCommand, IndirectBuffer, MdiStrategy};
pub use frame_data::{FrameData, InstanceData, RenderCommand};
pub use frame_scratch::FrameScratchBuffers;
pub use framebuffer::Framebuffer;
pub use lighting::{
    LIGHTS_BLOCK_GLSL, Light, LightData, LightId, LightManager, LightType, LightUniformBlock,
    MAX_LIGHTS,
};
pub use pass::{ClearFlags, PassConfig, PassGroup, PassType};
pub use pipeline::{
    BlendFactor, CullMode, DepthFunc, PipelineCache, PipelineState, PipelineStateId,
};
pub use renderer::{LIGHTS_UBO_BINDING, MeshId, Renderer, RendererConfig};
pub type RenderConfig = RendererConfig;
pub use scene::{ObjectHandle, ObjectKind, Scene, SortedInstance};
pub use skybox::{SkyboxId, SkyboxPipeline};
pub use stats::FrameStats;
pub use ui::{UiDrawList, UiVertex};
