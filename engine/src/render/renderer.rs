use crate::core::buffer::PersistentMappedBuffer;
use crate::core::math::{self, sphere_inside_frustum};
use crate::core::triple_buffer::ReadHandle;
use crate::render::draw_indirect::{DrawElementsIndirectCommand, IndirectBuffer, MdiStrategy};
use crate::render::frame_data::{FrameData, InstanceData};
use crate::render::frame_scratch::FrameScratchBuffers;
use crate::render::lighting::LightManager;
use crate::render::lighting_buffer::LightingBuffer;
use crate::render::pass::{PassConfig, PassGroup, PassType};
use crate::render::pipeline::{PipelineCache, PipelineState, PipelineStateId};
use crate::render::scene::{ObjectHandle, ObjectKind, Scene, SortedInstance};
use crate::render::skybox::SkyboxPipeline;
use crate::render::stats::FrameStats;
use crate::render::ui::UiRenderer;
use crate::resources::geometry_pool::GeometryPool;
use crate::resources::material::{MaterialId, MaterialManager};
use crate::resources::mesh::{Mesh, MeshData};
use crate::resources::shader::{ShaderId, ShaderManager};
use crate::resources::texture::{TextureId, TextureManager};

use bytemuck::{Pod, Zeroable};
use glam::Vec4;
use glow::HasContext;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

const DEFAULT_MAX_OBJECTS: usize = 65536;
const DEFAULT_TRANSFORM_REGIONS: usize = 3;
pub const LIGHTS_UBO_BINDING: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererConfig {
    pub max_objects: usize,
    pub transform_regions: usize,
    pub indirect_command_capacity: usize,
    pub initial_draw_batch_capacity: usize,
    /// Number of scene-grid cells to query around the camera before frustum culling.
    pub visibility_radius_cells: i32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            max_objects: DEFAULT_MAX_OBJECTS,
            transform_regions: DEFAULT_TRANSFORM_REGIONS,
            indirect_command_capacity: 4096,
            initial_draw_batch_capacity: 1024,
            visibility_radius_cells: 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct MeshId(pub u32);

pub struct Renderer {
    gl: Arc<glow::Context>,
    read_handle: ReadHandle<FrameData>,
    pub shaders: ShaderManager,
    pub materials: MaterialManager,
    pub textures: TextureManager,
    /// Scene lights uploaded to the `EngineLights` uniform block each frame.
    pub lights: LightManager,
    pub skybox: Option<SkyboxPipeline>,
    ui_renderer: UiRenderer,
    meshes: HashMap<MeshId, Mesh>,
    next_mesh_id: u32,
    transform_buffer: PersistentMappedBuffer,
    lighting_buffer: LightingBuffer,
    geometry_pool: GeometryPool,
    max_objects: usize,
    visibility_radius_cells: i32,
    width: i32,
    height: i32,

    pub scene: Scene,
    frame_stats: FrameStats,
    mdi_strategy: MdiStrategy,
    indirect_buffer: IndirectBuffer,

    pipeline_cache: PipelineCache,
    current_pipeline_id: Option<PipelineStateId>,

    last_frame: FrameData,
    has_frame: bool,

    region_fences: Vec<Option<glow::Fence>>,
    frame_index: u64,
    start_time: std::time::Instant,
    last_frustum: Option<[Vec4; 6]>,

    /// Reusable scratch buffers for frame rendering to avoid per-frame allocations.
    frame_scratch: FrameScratchBuffers,

    // OpenGL contexts are current on one thread. Keep the renderer from being
    // sent to a different thread after construction; create it on the render
    // thread after making that thread's context current.
    _thread_affinity: PhantomData<Rc<()>>,
}

impl Renderer {
    pub fn new(gl: glow::Context, read_handle: ReadHandle<FrameData>) -> Self {
        Self::with_config(gl, read_handle, RendererConfig::default())
    }

    pub fn with_config(
        gl: glow::Context,
        read_handle: ReadHandle<FrameData>,
        config: RendererConfig,
    ) -> Self {
        let gl = Arc::new(gl);
        let max_objects = config.max_objects.max(1);
        let transform_regions = config.transform_regions.max(1);
        let draw_capacity = config.initial_draw_batch_capacity.max(1);

        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
        }

        let transform_buffer = PersistentMappedBuffer::new(
            gl.clone(),
            max_objects * transform_regions * std::mem::size_of::<InstanceData>(),
        );
        let lighting_buffer = LightingBuffer::new(gl.clone());

        let geometry_pool = GeometryPool::new(gl.clone(), transform_buffer.handle);
        let indirect_buffer =
            IndirectBuffer::with_capacity(gl.clone(), true, config.indirect_command_capacity);
        let ui_renderer = UiRenderer::new(gl.clone());

        Self {
            gl,
            read_handle,
            shaders: ShaderManager::new(),
            materials: MaterialManager::new(),
            textures: TextureManager::new(),
            lights: LightManager::new(),
            skybox: None,
            ui_renderer,
            meshes: HashMap::new(),
            transform_buffer,
            lighting_buffer,
            geometry_pool,
            max_objects,
            visibility_radius_cells: config.visibility_radius_cells.max(0),
            next_mesh_id: 0,
            width: 1280,
            height: 720,
            scene: Scene::new(),
            frame_stats: FrameStats::default(),
            mdi_strategy: MdiStrategy::Multi,
            indirect_buffer,
            pipeline_cache: PipelineCache::new(),
            current_pipeline_id: None,
            last_frame: FrameData::default(),
            has_frame: false,
            region_fences: vec![None; transform_regions],
            frame_index: 0,
            start_time: std::time::Instant::now(),
            last_frustum: None,
            frame_scratch: FrameScratchBuffers::with_capacities(max_objects, draw_capacity),
            _thread_affinity: PhantomData,
        }
    }

    pub fn context(&self) -> Arc<glow::Context> {
        self.gl.clone()
    }
}

mod passes;
mod resource_api;
mod scene_api;
mod submission;
mod sync;
mod transparent;
mod world;

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            for fence in self.region_fences.iter().flatten() {
                self.gl.delete_sync(*fence);
            }
        }
    }
}
