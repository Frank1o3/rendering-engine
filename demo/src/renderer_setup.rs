// demo/src/renderer_setup.rs
use std::sync::{Arc, atomic::AtomicBool};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, Version},
    display::{GetGlDisplay, GlDisplay},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

use rendering_engine::{frame_data::FrameData, triple_buffer::new_triple_buffer};

use crate::{
    render_thread::start_render_thread,
    state::{DemoState, Keys},
    voxel::world::World,
};

use glam::Vec3;
use std::collections::HashMap;
use std::time::Instant;

/// Builds the window/GL surface/context on the main thread, then immediately
/// hands the context off to a dedicated render thread. The main thread never
/// makes the context current and never issues a GL call itself.
pub fn create_demo_state(event_loop: &ActiveEventLoop) -> DemoState {
    let config = crate::config::Config::load_or_default();
    let vsync_enabled = Arc::new(AtomicBool::new(config.vsync_default));
    let template = ConfigTemplateBuilder::new();

    let window_attributes = Window::default_attributes()
        .with_title("Rendering Engine — OpenGL ES 3.2 3D Demo")
        .with_inner_size(PhysicalSize::new(config.window_width, config.window_height))
        .with_resizable(false);

    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

    let (window, gl_config) = display_builder
        .build(event_loop, template, |configs| {
            configs
                .reduce(|a, b| {
                    if b.num_samples() > a.num_samples() {
                        b
                    } else {
                        a
                    }
                })
                .unwrap()
        })
        .expect("Failed to create display");

    let window = Arc::new(window.expect("Failed to create window"));

    let display = gl_config.display();

    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(Some(Version { major: 3, minor: 2 })))
        .build(Some(window.window_handle().unwrap().as_raw()));

    // NOT current here — ownership + make_current happen on the render thread.
    let not_current = unsafe {
        display
            .create_context(&gl_config, &context_attributes)
            .expect("Failed to create GL context")
    };

    let attrs = window.build_surface_attributes(Default::default()).unwrap();
    let gl_surface = unsafe { display.create_window_surface(&gl_config, &attrs).unwrap() };

    let (write_handle, read_handle) = new_triple_buffer::<FrameData>();

    let (render_tx, assets_rx, render_thread_handle) =
        start_render_thread(not_current, gl_surface, read_handle, vsync_enabled.clone());

    // Block once, briefly, until the render thread has compiled shaders and
    // uploaded the starting meshes — the game thread needs those IDs before
    // it can build its first FrameData.
    let assets = assets_rx
        .recv()
        .expect("Render thread closed before sending initial assets");

    let world = World::new(config.render_distance, render_tx.clone());

    DemoState {
        window,
        render_tx,
        render_thread_handle: Some(render_thread_handle),
        write_handle,
        assets,
        width: config.window_width,
        height: config.window_height,
        camera_pos: Vec3::new(0.0, 80.0, 0.0), // above typical terrain height so you spawn looking down at the world
        camera_yaw: 0.0,
        camera_pitch: 0.0,
        keys: Keys::default(),
        cursor_grabbed: false,
        touches: HashMap::new(),
        world,
        last_frame: Instant::now(),
        current_fps: 0.0,
        frame_count: 0,
        last_fps_update: Instant::now(),
        vsync_enabled,
        config,
    }
}
