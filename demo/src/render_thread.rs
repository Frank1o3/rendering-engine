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

/// Demo-only thread-control enum — distinct from
/// `rendering_engine::frame_data::RenderCommand`, which stays untouched.
/// `AddChunk`/`RemoveChunk` carry exactly what the engine needs (a mesh, a
/// position) and nothing voxel-specific leaks past this file.
pub enum RenderCommand {
    Render,
    Resize(u32, u32),
    AddChunk { pos: ChunkPos, mesh: MeshData },
    RemoveChunk { pos: ChunkPos },
    Shutdown,
}
enum PendingChunkOp {
    Add { pos: ChunkPos, mesh: MeshData },
    Remove { pos: ChunkPos },
}

const MAX_CHUNK_UPLOADS_PER_FRAME: usize = 1; // tune this — see note below

pub fn start_render_thread(
    not_current: NotCurrentContext,
    surface: Surface<WindowSurface>,
    read_handle: ReadHandle<FrameData>,
    vsync_enabled: Arc<AtomicBool>,
    frame_counter: Arc<AtomicU64>,
) -> (
    SyncSender<RenderCommand>,
    Receiver<Assets>,
    thread::JoinHandle<()>,
) {
    // Capacity raised from 1: chunk streaming can burst several
    // AddChunk/RemoveChunk sends per frame, unlike the old single Render ping.
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

        // UI-only startup assets — terrain streams in per-chunk below.
        let quad_mesh = renderer.load_mesh(create_quad_mesh());
        let button_quad_mesh = renderer.load_mesh(create_button_quad_mesh());
        let vsync_button_mesh = renderer.load_mesh(create_vsync_button_mesh());

        let shader_map = renderer
            .load_shaders_from_include_dir(&SHADERS)
            .expect("Failed to load shaders");

        let lit_shader = *shader_map.get("lit").expect("Missing lit shader");
        let ui_shader = *shader_map.get("ui").expect("Missing ui shader");

        // Terrain reuses the lit pipeline as-is — a greedy-meshed chunk is
        // just another opaque, per-vertex-colored mesh to the engine.
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

        let assets = Assets {
            quad_mesh: quad_mesh.expect("quad mesh should be loaded"),
            button_quad_mesh: button_quad_mesh.expect("button quad mesh should be loaded"),
            vsync_button_mesh: vsync_button_mesh.expect("vsync button quad mesh should be loaded"),
            terrain_material,
            ui_material,
            lit_shader,
        };

        if assets_tx.send(assets).is_err() {
            return;
        }

        let mut current_vsync = false;
        // Tracks each loaded chunk's GPU-side identity so RemoveChunk can
        // tear it down again. Lives here, not in Renderer — the engine has
        // no concept of "chunk," only meshes and scene objects.
        let mut chunk_objects: HashMap<ChunkPos, (MeshId, ObjectHandle)> = HashMap::new();

        let mut pending: std::collections::VecDeque<PendingChunkOp> =
            std::collections::VecDeque::new();

        loop {
            match cmd_rx.recv() {
                Ok(RenderCommand::Render) => {
                    let mut budget = MAX_CHUNK_UPLOADS_PER_FRAME;
                    while budget > 0 {
                        let Some(op) = pending.pop_front() else { break };
                        match op {
                            PendingChunkOp::Add { pos, mesh } => {
                                if let Some(mesh_id) = renderer.load_mesh_trusted_winding(mesh) {
                                    let obj = renderer.add_object(
                                        mesh_id,
                                        terrain_material,
                                        ObjectKind::Static,
                                    );
                                    renderer.set_position(obj, pos.world_origin());
                                    chunk_objects.insert(pos, (mesh_id, obj));
                                }
                            }
                            PendingChunkOp::Remove { pos } => {
                                if let Some((mesh_id, obj)) = chunk_objects.remove(&pos) {
                                    renderer.remove_object(obj);
                                    renderer.unload_mesh(mesh_id);
                                }
                            }
                        }
                        budget -= 1;
                    }

                    renderer.render();

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
                    let _ = surface.swap_buffers(&gl_context);
                    frame_counter.fetch_add(1, Ordering::Relaxed); // moved here — counts real presents only
                }

                Ok(RenderCommand::Resize(w, h)) => {
                    if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                        surface.resize(&gl_context, nw, nh);
                        renderer.resize(w as i32, h as i32);
                    }
                }

                Ok(RenderCommand::AddChunk { pos, mesh }) => {
                    pending.push_back(PendingChunkOp::Add { pos, mesh });
                }

                Ok(RenderCommand::RemoveChunk { pos }) => {
                    pending.push_back(PendingChunkOp::Remove { pos });
                }

                Ok(RenderCommand::Shutdown) | Err(_) => break,
            }
        }
    });

    (cmd_tx, assets_rx, handle)
}
