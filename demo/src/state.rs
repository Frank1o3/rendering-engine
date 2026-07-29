// demo/src/state.rs
use std::{
    collections::HashMap,
    sync::{mpsc::SyncSender, Arc},
    thread::JoinHandle,
    time::Instant,
};

use glam::Vec3;
use winit::window::Window;

use rendering_engine::{
    engine::{MaterialId, MeshId, ShaderId},
    frame_data::FrameData,
    triple_buffer::WriteHandle,
};
use winit::event_loop::ActiveEventLoop;

use crate::{render_thread::RenderCommand, touch::TouchKind};

/// Current keyboard movement state.
#[derive(Default, Debug)]
pub struct Keys {
    pub w: bool,
    pub a: bool,
    pub s: bool,
    pub d: bool,
    pub space: bool,
    pub lctrl: bool,
}

/// A single collectible object.
#[derive(Debug, Clone)]
pub struct Collectible {
    pub position: Vec3,
    pub scale: f32,
}

/// All game‑specific state.
#[derive(Debug)]
pub struct GameState {
    pub score: u32,
    pub collectibles: Vec<Collectible>,
    pub spawn_radius: f32,
    pub collect_distance: f32,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            score: 0,
            collectibles: Vec::new(),
            spawn_radius: 8.0,
            collect_distance: 0.8,
        }
    }
}

/// GPU asset IDs handed back by the render thread once it finishes loading.
/// These are plain `Copy` identifiers — no GL resources cross this boundary.
pub struct Assets {
    pub cube_mesh: MeshId,
    pub quad_mesh: MeshId,
    pub button_quad_mesh: MeshId,
    pub collectible_mesh: MeshId,

    pub lit_material: MaterialId,
    pub ui_material: MaterialId,

    pub lit_shader: ShaderId,
}

/// Everything owned by the main/game thread. Notably: no `Renderer`, no GL
/// context, no surface — those live exclusively on the render thread now.
pub struct DemoState {
    // Window
    pub window: Arc<Window>,

    // Render-thread communication
    pub render_tx: SyncSender<RenderCommand>,
    pub render_thread_handle: Option<JoinHandle<()>>,
    pub write_handle: WriteHandle<FrameData>,

    // Assets
    pub assets: Assets,

    // Window size
    pub width: u32,
    pub height: u32,

    // Camera
    pub camera_pos: Vec3,
    pub camera_yaw: f32,
    pub camera_pitch: f32,

    // Input
    pub keys: Keys,
    pub cursor_grabbed: bool,
    pub touches: HashMap<u64, TouchKind>,

    // Game
    pub game: GameState,

    // Demo animation
    pub dyn_angle: f32,

    // Timing
    pub last_frame: Instant,

    // FPS counter
    pub current_fps: f32,
    pub frame_count: u32,
    pub last_fps_update: Instant,
}

impl DemoState {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        crate::renderer_setup::create_demo_state(event_loop)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        // Rare event — fine to block briefly if the render thread is busy.
        let _ = self.render_tx.send(RenderCommand::Resize(width, height));
    }

    /// Pings the render thread to draw the most recently published frame.
    /// Non-blocking: if a render request is already queued, this one is
    /// dropped rather than piling up — the triple buffer guarantees the
    /// render thread always sees the latest `FrameData` regardless.
    pub fn request_render(&mut self) {
        use std::sync::mpsc::TrySendError;
        match self.render_tx.try_send(RenderCommand::Render) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                log::error!("Render thread has disconnected");
            }
        }
    }

    pub fn shutdown_render_thread(&mut self) {
        let _ = self.render_tx.send(RenderCommand::Shutdown);
        if let Some(handle) = self.render_thread_handle.take() {
            let _ = handle.join();
        }
    }
}
