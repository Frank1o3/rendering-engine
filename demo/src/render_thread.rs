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

use rendering_engine::{
    MeshData, ObjectHandle, ObjectKind,
    engine::{MeshId, Renderer},
    frame_data::FrameData,
    pipeline::{BlendFactor, CullMode, DepthFunc, PipelineState},
    triple_buffer::ReadHandle,
};

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
    RemoveChunk { pos: ChunkPos },
    Shutdown,
}

/// A chunk's CPU/GPU residency state. The mesh is always kept on the CPU
/// once generated; `gpu` is only `Some` while the chunk is actually
/// uploaded and drawn. `invisible_frames` tracks how long it's been out of
/// the frustum, used for eviction hysteresis so a brief flick of the
/// camera doesn't thrash upload/free every frame.
struct ChunkEntry {
    mesh: MeshData,
    gpu: Option<(MeshId, ObjectHandle)>,
    invisible_frames: u32,
}

/// GPU uploads (buffer_sub_data into the geometry pool, VAO setup) are the
/// expensive part — budget them per frame so a big burst of newly-visible
/// chunks (e.g. spinning around) can't spike a single frame.
const MAX_CHUNK_GPU_UPLOADS_PER_FRAME: usize = 1;
/// Evictions are cheaper than uploads but still touch the free-list
/// allocator and scene bookkeeping — budget lightly to smooth out the case
/// where many chunks cross the grace threshold in the same frame.
const MAX_CHUNK_GPU_EVICTIONS_PER_FRAME: usize = 2;
/// How many consecutive invisible frames before a chunk's GPU resources are
/// freed. ~1.5s at 60 FPS — long enough that a quick glance away and back
/// doesn't cause a re-upload, short enough to actually save GPU work when
/// you're facing away for a while.
const INVISIBLE_GRACE_FRAMES: u32 = 90;

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
        let wireframe_button_mesh = renderer.load_mesh(create_vsync_button_mesh());

        let shader_map = renderer
            .load_shaders_from_include_dir(&SHADERS)
            .expect("Failed to load shaders");

        let lit_shader = *shader_map.get("lit").expect("Missing lit shader");
        let ui_shader = *shader_map.get("ui").expect("Missing ui shader");

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

        renderer.upload_shader_vec3(
            lit_shader,
            "uSunDir",
            glam::Vec3::new(0.6, 0.8, 0.4).normalize(),
        );
        renderer.upload_shader_f32(lit_shader, "uAmbient", 0.28);
        renderer.upload_shader_f32(lit_shader, "uWireframe", 0.0);

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

        // Every generated chunk lives here on the CPU. GPU residency is
        // tracked per-entry via `gpu` and streamed in/out based on the
        // frustum each frame — see the promote/demote step below.
        let mut chunk_entries: HashMap<ChunkPos, ChunkEntry> = HashMap::new();

        loop {
            match cmd_rx.recv() {
                Ok(RenderCommand::Render) => {
                    renderer.render();

                    // ── Streaming: promote visible cached chunks to GPU,
                    // evict long-invisible uploaded chunks back to CPU-only ──
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
                    // Cache only — GPU upload is decided by the streaming
                    // step above, next time a Render command runs.
                    chunk_entries.insert(
                        pos,
                        ChunkEntry {
                            mesh,
                            gpu: None,
                            invisible_frames: 0,
                        },
                    );
                }

                Ok(RenderCommand::RemoveChunk { pos }) => {
                    // Truly out of render distance — drop both GPU and CPU
                    // copies. (Distinct from eviction above, which keeps
                    // the CPU mesh cached for a fast re-upload.)
                    if let Some(entry) = chunk_entries.remove(&pos) {
                        if let Some((mesh_id, obj)) = entry.gpu {
                            renderer.remove_object(obj);
                            renderer.unload_mesh(mesh_id);
                        }
                    }
                }

                Ok(RenderCommand::Shutdown) | Err(_) => break,
            }
        }
    });

    (cmd_tx, assets_rx, handle)
}
