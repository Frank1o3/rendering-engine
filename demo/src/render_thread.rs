use std::collections::HashMap;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use glow::Context;
use glutin::display::GetGlDisplay;
use glutin::{
    context::{NotCurrentContext, NotCurrentGlContext},
    display::GlDisplay,
    surface::{GlSurface, Surface, SwapInterval, WindowSurface},
};

use rendering_engine::core::triple_buffer::ReadHandle;
use rendering_engine::render::{BlendFactor, CullMode, DepthFunc};
use rendering_engine::render::frame_data::FrameData;
use rendering_engine::render::pipeline::PipelineState;
use rendering_engine::render::renderer::{MeshId, Renderer};
use rendering_engine::render::scene::{ObjectHandle, ObjectKind};
use rendering_engine::resources::mesh::MeshData;

use crate::meshes::create_wireframe_button_mesh;
use crate::{
    meshes::{create_button_quad_mesh, create_quad_mesh, create_vsync_button_mesh},
    shaders::SHADERS,
    state::Assets,
    voxel::chunk::ChunkPos,
};

pub enum RenderCommand {
    Render,
    Resize(u32, u32),
    AddChunk { pos: ChunkPos, mesh: MeshData },
    RemoveChunks(Vec<ChunkPos>),
    Shutdown,
}

struct ChunkEntry {
    mesh: MeshData,
    gpu: Option<(MeshId, ObjectHandle)>,
    invisible_frames: u32,
}

const MAX_CHUNK_GPU_UPLOADS_PER_FRAME: usize = 3;
const MAX_CHUNK_GPU_EVICTIONS_PER_FRAME: usize = 3;
const INVISIBLE_GRACE_FRAMES: u32 = 5;

pub fn start_render_thread(
    not_current: NotCurrentContext,
    surface: Surface<WindowSurface>,
    read_handle: ReadHandle<FrameData>,
    vsync_enabled: Arc<AtomicBool>,
    frame_counter: Arc<AtomicU64>,
    wireframe_enabled: Arc<AtomicBool>,
) -> (
    SyncSender<RenderCommand>,
    Receiver<Assets>,
    thread::JoinHandle<()>,
) {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel(64);
    let (assets_tx, assets_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let gl_context = not_current
            .make_current(&surface)
            .expect("Failed to make GL context current on render thread");

        surface
            .set_swap_interval(&gl_context, SwapInterval::DontWait)
            .ok();

        let gl_display = surface.display();
        let gl = unsafe {
            Context::from_loader_function(|sym| {
                let c = CString::new(sym).unwrap();
                gl_display.get_proc_address(&c)
            })
        };

        let mut renderer = Renderer::new(gl, read_handle);

        let quad_mesh = renderer.load_mesh(create_quad_mesh());
        let button_quad_mesh = renderer.load_mesh(create_button_quad_mesh());
        let vsync_button_mesh = renderer.load_mesh(create_vsync_button_mesh());
        let wireframe_button_mesh = renderer.load_mesh(create_wireframe_button_mesh());

        let shader_map = renderer
            .load_shaders_from_include_dir(&SHADERS)
            .expect("Failed to load shaders");

        let lit_shader = *shader_map.get("lit").expect("Missing lit shader");
        let ui_shader = *shader_map.get("ui").expect("Missing ui shader");

        // ─── Skybox setup ──────────────────────────────────────────────
        let skybox_shader_id = *shader_map.get("skybox").expect("Missing skybox shader");

        // Create the skybox pipeline
        use rendering_engine::render::skybox::SkyboxPipeline;
        renderer.skybox = Some(SkyboxPipeline::new(renderer.context(), skybox_shader_id));
        renderer.enable_skybox(true);
        renderer.set_skybox_color(glam::Vec3::new(0.5, 0.7, 1.0));

        // Sun parameters
        let sun_dir = glam::Vec3::new(0.6, 0.8, 0.4).normalize();
        let sun_color = glam::Vec3::new(1.0, 0.9, 0.6);
        let sun_size = 0.1;

        // Upload skybox uniforms
        renderer.upload_shader_vec3(skybox_shader_id, "uSunDir", sun_dir);
        renderer.upload_shader_vec3(skybox_shader_id, "uSunColor", sun_color);
        renderer.upload_shader_f32(skybox_shader_id, "uSunSize", sun_size);

        // Upload lit shader uniforms
        renderer.upload_shader_vec3(lit_shader, "uSunDir", sun_dir);
        renderer.upload_shader_f32(lit_shader, "uAmbient", 0.38);
        renderer.upload_shader_f32(lit_shader, "uWireframe", 0.0);

        // ─── Materials ────────────────────────────────────────────────────
        let terrain_material =
            renderer.create_material(lit_shader, PipelineState::default_opaque(lit_shader.0));
        let ui_pipeline = PipelineState {
            shader_id: ui_shader.0,
            cull_mode: CullMode::None,
            depth_test: true,
            depth_write: false,
            depth_func: DepthFunc::Less,
            blend_enabled: true,
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
        };
        let ui_material = renderer.create_material(ui_shader, ui_pipeline);

        let assets = Assets {
            quad_mesh: quad_mesh.expect("quad mesh should be loaded"),
            button_quad_mesh: button_quad_mesh.expect("button quad mesh should be loaded"),
            vsync_button_mesh: vsync_button_mesh.expect("vsync button quad mesh should be loaded"),
            wireframe_button_mesh: wireframe_button_mesh
                .expect("wireframe button quad mesh should be loaded"),
            terrain_material,
            ui_material,
            lit_shader,
        };

        if assets_tx.send(assets).is_err() {
            return;
        }

        let mut current_vsync = false;
        let mut current_wireframe = false;

        let mut chunk_entries: HashMap<ChunkPos, ChunkEntry> = HashMap::new();
        let mut scan_counter: u32 = 0;

        loop {
            match cmd_rx.recv() {
                Ok(RenderCommand::Render) => {
                    renderer.render();

                    scan_counter = scan_counter.wrapping_add(1);
                    if scan_counter % 4 == 0 {
                        if let Some(frustum) = renderer.current_frustum() {
                            let mut upload_budget = MAX_CHUNK_GPU_UPLOADS_PER_FRAME;
                            let mut evict_budget = MAX_CHUNK_GPU_EVICTIONS_PER_FRAME;

                            for (pos, entry) in chunk_entries.iter_mut() {
                                let visible = pos.is_visible(&frustum);

                                if entry.gpu.is_none() {
                                    if visible && upload_budget > 0 {
                                        if let Some(mesh_id) =
                                            renderer.load_mesh_trusted_winding(entry.mesh.clone())
                                        {
                                            let obj = renderer.add_object(
                                                mesh_id,
                                                terrain_material,
                                                ObjectKind::Static,
                                            );
                                            renderer.set_position(obj, pos.world_origin());
                                            entry.gpu = Some((mesh_id, obj));
                                            entry.invisible_frames = 0;
                                            upload_budget -= 1;
                                        } else {
                                            log::warn!(
                                                "GeometryPool exhausted: failed to upload chunk {:?} \
                                                 (will retry next scan cycle)",
                                                pos
                                            );
                                        }
                                    }
                                } else if visible {
                                    entry.invisible_frames = 0;
                                } else {
                                    entry.invisible_frames += 1;
                                    if entry.invisible_frames > INVISIBLE_GRACE_FRAMES
                                        && evict_budget > 0
                                    {
                                        if let Some((mesh_id, obj)) = entry.gpu.take() {
                                            renderer.remove_object(obj);
                                            renderer.unload_mesh(mesh_id);
                                        }
                                        evict_budget -= 1;
                                    }
                                }
                            }
                        }
                    }

                    let new_vsync = vsync_enabled.load(Ordering::Relaxed);
                    if new_vsync != current_vsync {
                        let interval = if new_vsync {
                            SwapInterval::Wait(NonZeroU32::new(1).unwrap())
                        } else {
                            SwapInterval::DontWait
                        };
                        if surface.set_swap_interval(&gl_context, interval).is_ok() {
                            current_vsync = new_vsync;
                        }
                    }

                    let new_wireframe = wireframe_enabled.load(Ordering::Relaxed);
                    if new_wireframe != current_wireframe {
                        renderer.upload_shader_f32(
                            lit_shader,
                            "uWireframe",
                            if new_wireframe { 1.0 } else { 0.0 },
                        );
                        current_wireframe = new_wireframe;
                    }

                    let _ = surface.swap_buffers(&gl_context);
                    frame_counter.fetch_add(1, Ordering::Relaxed);
                }

                Ok(RenderCommand::Resize(w, h)) => {
                    if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                        surface.resize(&gl_context, nw, nh);
                        renderer.resize(w as i32, h as i32);
                    }
                }

                Ok(RenderCommand::AddChunk { pos, mesh }) => {
                    chunk_entries.insert(
                        pos,
                        ChunkEntry {
                            mesh,
                            gpu: None,
                            invisible_frames: 0,
                        },
                    );
                }

                Ok(RenderCommand::RemoveChunks(positions)) => {
                    for pos in positions {
                        if let Some(entry) = chunk_entries.remove(&pos) {
                            if let Some((mesh_id, obj)) = entry.gpu {
                                renderer.remove_object(obj);
                                renderer.unload_mesh(mesh_id);
                            }
                        }
                    }
                }

                Ok(RenderCommand::Shutdown) | Err(_) => break,
            }
        }
    });

    (cmd_tx, assets_rx, handle)
}
