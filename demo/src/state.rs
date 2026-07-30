use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
        mpsc::SyncSender,
    },
    thread::JoinHandle,
    time::Instant,
};

use glam::Vec3;
use winit::{event_loop::ActiveEventLoop, window::Window};

use rendering_engine::{
    engine::{MaterialId, MeshId, ShaderId},
    frame_data::FrameData,
    triple_buffer::WriteHandle,
};

use crate::{render_thread::RenderCommand, touch::TouchKind, voxel::world::World};

#[derive(Default, Debug)]
pub struct Keys {
    pub w: bool,
    pub a: bool,
    pub s: bool,
    pub d: bool,
    pub space: bool,
    pub lctrl: bool,
}

/// Startup-only GPU asset IDs. Terrain meshes are NOT here — they stream in
/// per-chunk and are tracked entirely on the render thread.
pub struct Assets {
    pub quad_mesh: MeshId,
    pub button_quad_mesh: MeshId,
    pub vsync_button_mesh: MeshId,
    pub wireframe_button_mesh: MeshId,
    pub terrain_material: MaterialId,
    pub ui_material: MaterialId,
    pub lit_shader: ShaderId,
}

pub struct DemoState {
    pub window: Arc<Window>,

    pub render_tx: SyncSender<RenderCommand>,
    pub render_thread_handle: Option<JoinHandle<()>>,
    pub write_handle: WriteHandle<FrameData>,

    pub assets: Assets,

    pub width: u32,
    pub height: u32,

    // Fly camera — no gravity or collision. Exploration only.
    pub camera_pos: Vec3,
    pub camera_yaw: f32,
    pub camera_pitch: f32,

    pub keys: Keys,
    pub cursor_grabbed: bool,
    pub touches: HashMap<u64, TouchKind>,

    pub world: World,

    pub last_frame: Instant,
    pub current_fps: f32,
    pub last_fps_update: Instant,
    pub vsync_enabled: Arc<AtomicBool>,
    pub config: crate::config::Config,
    pub frame_counter: Arc<AtomicU64>,
    pub last_frame_counter: u64,
    pub wireframe_enabled: Arc<AtomicBool>,
}

impl DemoState {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        crate::renderer_setup::create_demo_state(event_loop)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let _ = self.render_tx.send(RenderCommand::Resize(width, height));
    }

    pub fn request_render(&mut self) {
        use std::sync::mpsc::TrySendError;
        match self.render_tx.try_send(RenderCommand::Render) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => log::error!("Render thread has disconnected"),
        }
    }

    pub fn shutdown_render_thread(&mut self) {
        let _ = self.render_tx.send(RenderCommand::Shutdown);
        if let Some(handle) = self.render_thread_handle.take() {
            let _ = handle.join();
        }
    }
}
