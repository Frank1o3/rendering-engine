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
use rendering_engine::render::frame_data::FrameData;
use rendering_engine::render::pipeline::PipelineState;
use rendering_engine::render::renderer::{MeshId, Renderer};
use rendering_engine::render::scene::{ObjectHandle, ObjectKind};
use rendering_engine::resources::mesh::MeshData;

use crate::{
    shaders::SHADERS,
    state::Assets,
    voxel::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z, ChunkPos},
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
}

const MAX_CHUNK_GPU_UPLOADS_PER_FRAME: usize = 3;

pub fn start_render_thread(
    not_current: NotCurrentContext,
    surface: Surface<WindowSurface>,
    read_handle: ReadHandle<FrameData>,
    vsync_enabled: Arc<AtomicBool>,
    frame_counter: Arc<AtomicU64>,
    render_pending: Arc<AtomicBool>,
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

        let shader_map = renderer
            .load_shaders_from_include_dir(&SHADERS)
            .expect("Failed to load shaders");

        let lit_shader = *shader_map.get("lit").expect("Missing lit shader");

        // ─── Skybox setup ──────────────────────────────────────────────
        let skybox_shader_id = *shader_map.get("skybox").expect("Missing skybox shader");

        // Create the skybox pipeline
        use rendering_engine::render::skybox::SkyboxPipeline;
        renderer.skybox = Some(SkyboxPipeline::new(renderer.context(), skybox_shader_id));
        renderer.enable_skybox(true);
        renderer.set_skybox_color(glam::Vec3::new(0.5, 0.7, 1.0));
        renderer.set_skybox_cloud_coverage(0.42);

        // Sun parameters
        // Keep the demo sun inside the initial forward view (camera faces -Z).
        let sun_dir = glam::Vec3::new(0.25, 0.3, -0.92).normalize();
        let sun_color = glam::Vec3::new(1.0, 0.9, 0.6);
        let sun_size = 0.1;

        // Upload skybox uniforms
        renderer.upload_shader_vec3(skybox_shader_id, "uSunDir", sun_dir);
        renderer.upload_shader_vec3(skybox_shader_id, "uSunColor", sun_color);
        renderer.upload_shader_f32(skybox_shader_id, "uSunSize", sun_size);

        // The engine uploads this light through its fixed std140 UBO each frame.
        renderer.lights.create_directional(-sun_dir, sun_color, 1.0);
        renderer.bind_light_block(lit_shader);

        // Upload remaining demo-specific lit shader uniforms.
        renderer.upload_shader_f32(lit_shader, "uAmbient", 0.38);
        renderer.upload_shader_f32(lit_shader, "uWireframe", 0.0);

        // ─── Materials ────────────────────────────────────────────────────
        let terrain_material =
            renderer.create_material(lit_shader, PipelineState::default_opaque(lit_shader.0));
        let assets = Assets { terrain_material };

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
                    // Release the queued-request slot as soon as this frame is
                    // dequeued. The game thread can then queue at most one
                    // newer frame while this one is being rendered.
                    render_pending.store(false, Ordering::Release);
                    renderer.render();

                    scan_counter = scan_counter.wrapping_add(1);
                    if scan_counter % 4 == 0 {
                        if let Some(frustum) = renderer.current_frustum() {
                            let mut upload_budget = MAX_CHUNK_GPU_UPLOADS_PER_FRAME;
                            let camera = renderer.current_camera_position();
                            let camera_chunk = ChunkPos {
                                x: (camera.x / CHUNK_SIZE_X as f32).floor() as i32,
                                z: (camera.z / CHUNK_SIZE_Z as f32).floor() as i32,
                            };
                            let mut ordered_positions: Vec<_> =
                                chunk_entries.keys().copied().collect();
                            ordered_positions.sort_by_key(|pos| {
                                let dx = pos.x - camera_chunk.x;
                                let dz = pos.z - camera_chunk.z;
                                (dx * dx + dz * dz, dz, dx)
                            });

                            for pos in ordered_positions {
                                let Some(entry) = chunk_entries.get_mut(&pos) else {
                                    continue;
                                };
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
                                            upload_budget -= 1;
                                        } else {
                                            log::warn!(
                                                "GeometryPool exhausted: failed to upload chunk {:?} \
                                                 (will retry next scan cycle)",
                                                pos
                                            );
                                        }
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
                    chunk_entries.insert(pos, ChunkEntry { mesh, gpu: None });
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
