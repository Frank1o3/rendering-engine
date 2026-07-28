// src/state.rs

use std::{collections::HashMap, sync::Arc, time::Instant};

use glam::Vec3;
use glutin::{
    config::ConfigTemplateBuilder,
    context::PossiblyCurrentContext,
    surface::{Surface, WindowSurface},
};
use winit::window::Window;

use rendering_engine::{
    engine::{MaterialId, MeshId, Renderer, ShaderId},
    frame_data::FrameData,
    triple_buffer::WriteHandle,
};
use winit::event_loop::ActiveEventLoop;

use crate::touch::TouchKind;

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

/// GPU assets used by the demo.
pub struct Assets {
    pub cube_mesh: MeshId,
    pub quad_mesh: MeshId,
    pub button_quad_mesh: MeshId,
    pub collectible_mesh: MeshId,

    pub lit_material: MaterialId,
    pub ui_material: MaterialId,

    pub lit_shader: ShaderId,
}

/// Everything owned while the application is running.
pub struct DemoState {
    // Window / OpenGL
    pub window: Arc<Window>,
    pub gl_context: PossiblyCurrentContext,
    pub gl_surface: Surface<WindowSurface>,

    // Renderer
    pub renderer: Renderer,
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
        let template = ConfigTemplateBuilder::new();

        crate::renderer_setup::create_demo_state(event_loop, template)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        use glutin::surface::GlSurface;

        self.width = width;
        self.height = height;

        self.gl_surface.resize(
            &self.gl_context,
            width.try_into().unwrap(),
            height.try_into().unwrap(),
        );

        self.renderer.resize(width as i32, height as i32);
    }

    pub fn render(&mut self) {
        use glutin::surface::GlSurface;

        self.gl_surface.swap_buffers(&self.gl_context).unwrap();
    }
}
