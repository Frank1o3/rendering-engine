// demo/src/render_thread.rs
//
// Owns the GL context, the surface, and the Renderer for the entire life of
// the program. Nothing outside this thread is allowed to touch GL state.
//
// The game/main thread only ever talks to this thread through `RenderCommand`.
// `Render` requests are sent through a capacity-1 `sync_channel` and pushed
// with `try_send`: if the render thread hasn't drained the previous request
// yet, we simply drop the new one instead of queuing up. That's safe because
// the triple buffer always holds the *latest* published `FrameData` — a
// dropped `Render` ping just means "render whatever's current a little
// later," never stale or corrupted data.

use std::ffi::CString;
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
    engine::Renderer,
    frame_data::FrameData,
    pipeline::{BlendFactor, CullMode, DepthFunc, PipelineState},
    triple_buffer::ReadHandle,
};

use glam::Vec3;

use crate::{
    meshes::{
        create_button_quad_mesh, create_collectible_mesh, create_cube_mesh, create_quad_mesh,
    },
    shaders::SHADERS,
    state::Assets,
};

pub enum RenderCommand {
    Render,
    Resize(u32, u32),
    Shutdown,
}

/// Spawns the render thread.
///
/// Returns:
/// - a `SyncSender` for issuing `RenderCommand`s,
/// - a `Receiver` that yields exactly one `Assets` message once the render
///   thread has finished compiling shaders and uploading the initial mesh
///   set (the caller should block on this once before entering the event
///   loop — the game thread needs those IDs to build its first frame),
/// - the thread's `JoinHandle`.
pub fn start_render_thread(
    not_current: NotCurrentContext,
    surface: Surface<WindowSurface>,
    read_handle: ReadHandle<FrameData>,
) -> (
    SyncSender<RenderCommand>,
    Receiver<Assets>,
    thread::JoinHandle<()>,
) {
    // Capacity 1: at most one pending "please render" ping in flight.
    let (cmd_tx, cmd_rx) = mpsc::sync_channel(1);
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

        // ── Assets (all GL work — must happen here, not on the main thread) ──
        let cube_mesh = renderer.load_mesh(create_cube_mesh());
        let quad_mesh = renderer.load_mesh(create_quad_mesh());
        let button_quad_mesh = renderer.load_mesh(create_button_quad_mesh());
        let collectible_mesh = renderer.load_mesh(create_collectible_mesh());

        let shader_map = renderer
            .load_shaders_from_include_dir(&SHADERS)
            .expect("Failed to load shaders");

        let lit_shader = *shader_map.get("lit").expect("Missing lit shader");
        let ui_shader = *shader_map.get("ui").expect("Missing ui shader");

        let lit_material =
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

        // Static lighting uniforms — set once, not per frame.
        renderer.upload_shader_vec3(lit_shader, "uSunDir", Vec3::new(0.6, 0.8, 0.4).normalize());
        renderer.upload_shader_f32(lit_shader, "uAmbient", 0.18);

        let assets = Assets {
            cube_mesh,
            quad_mesh,
            button_quad_mesh,
            collectible_mesh,
            lit_material,
            ui_material,
            lit_shader,
        };

        if assets_tx.send(assets).is_err() {
            // Main thread already gone.
            return;
        }

        // ── Main render loop ────────────────────────────────────────────
        loop {
            match cmd_rx.recv() {
                Ok(RenderCommand::Render) => {
                    renderer.render();
                    if let Err(e) = surface.swap_buffers(&gl_context) {
                        log::error!("swap_buffers failed: {e:?}");
                    }
                }
                Ok(RenderCommand::Resize(w, h)) => {
                    use std::num::NonZeroU32;
                    if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                        surface.resize(&gl_context, nw, nh);
                        renderer.resize(w as i32, h as i32);
                    }
                }
                Ok(RenderCommand::Shutdown) | Err(_) => break,
            }
        }
        // `renderer` drops here, deleting GL resources on the thread that
        // owns the context — never on the main thread.
    });

    (cmd_tx, assets_rx, handle)
}
