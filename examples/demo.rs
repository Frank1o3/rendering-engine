use glow::Context;
use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext,
        PossiblyCurrentContext, Version,
    },
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, WindowSurface},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use log::info;
use raw_window_handle::HasWindowHandle;
use std::{ffi::CString, num::NonZeroU32, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

use glam::{Quat, Vec3};
use rendering_engine::{
    engine::{MaterialId, MeshId, Renderer},
    frame_data::{FrameData, RenderCommand},
    math::{Transform, transform_to_model_matrix},
    mesh::{MeshData, Vertex},
    triple_buffer::{WriteHandle, new_triple_buffer},
};

// ==========================================
// 1. STATE STRUCTS
// ==========================================

struct Keys {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    space: bool,
    shift: bool,
}
impl Default for Keys {
    fn default() -> Self {
        Self {
            w: false,
            a: false,
            s: false,
            d: false,
            space: false,
            shift: false,
        }
    }
}

struct DemoState {
    window: Arc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    renderer: Renderer,
    write_handle: WriteHandle<FrameData>,

    mesh_id: MeshId,
    material_id: MaterialId,

    width: u32,
    height: u32,

    // Camera state
    camera_pos: Vec3,
    camera_yaw: f32,
    camera_pitch: f32,

    // Input state
    keys: Keys,
    cursor_grabbed: bool,

    // Timing
    last_frame: Instant,
    frame_count: u32,
    last_fps_update: Instant,
}

pub struct DemoApp {
    template: ConfigTemplateBuilder,
    state: Option<DemoState>,
}

impl DemoApp {
    pub fn new(template: ConfigTemplateBuilder) -> Self {
        Self {
            state: None,
            template,
        }
    }
}

// ==========================================
// 2. 3D CUBE MESH GENERATION
// ==========================================

fn create_cube_mesh() -> MeshData {
    // 24 vertices (4 per face) to allow for distinct face colors
    let positions = [
        // Front face (Z+)
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
        // Back face (Z-)
        [0.5, -0.5, -0.5],
        [-0.5, -0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [0.5, 0.5, -0.5],
        // Top face (Y+)
        [-0.5, 0.5, 0.5],
        [0.5, 0.5, 0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        // Bottom face (Y-)
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, -0.5, 0.5],
        [-0.5, -0.5, 0.5],
        // Right face (X+)
        [0.5, -0.5, 0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [0.5, 0.5, 0.5],
        // Left face (X-)
        [-0.5, -0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [-0.5, 0.5, 0.5],
        [-0.5, 0.5, -0.5],
    ];

    let colors = [
        [255, 0, 0, 255],   // Red
        [0, 255, 0, 255],   // Green
        [0, 0, 255, 255],   // Blue
        [255, 255, 0, 255], // Yellow
        [255, 0, 255, 255], // Magenta
        [0, 255, 255, 255], // Cyan
    ];

    let mut vertices = Vec::with_capacity(24);
    for (i, face) in positions.chunks(4).enumerate() {
        for &pos in face {
            vertices.push(Vertex {
                position: pos,
                color: colors[i],
            });
        }
    }

    // 36 indices (6 faces * 2 triangles * 3 vertices)
    let indices = vec![
        0, 1, 2, 2, 3, 0, // Front
        4, 5, 6, 6, 7, 4, // Back
        8, 9, 10, 10, 11, 8, // Top
        12, 13, 14, 14, 15, 12, // Bottom
        16, 17, 18, 18, 19, 16, // Right
        20, 21, 22, 22, 23, 20, // Left
    ];

    MeshData { vertices, indices }
}

// ==========================================
// 3. APPLICATION HANDLER (GAME LOOP)
// ==========================================

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("3D Demo - Click ESC to capture/release mouse")
            .with_inner_size(PhysicalSize::new(1280, 720));

        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

        let (window, gl_config) = display_builder
            .build(event_loop, self.template.clone(), |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .expect("Failed to perform GL bootstrapping...");

        let window = Arc::new(window.expect("Failed to create winit window..."));
        let gl_display = gl_config.display();

        let window_handle = window.window_handle().unwrap();
        let context_attributes = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .with_context_api(ContextApi::OpenGl(Some(Version { major: 4, minor: 6 })))
            .build(Some(window_handle.as_raw()));

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .expect("Failed to create GL context...")
        };

        let attrs = window.build_surface_attributes(Default::default()).unwrap();
        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
                .expect("Failed to create window surface")
        };

        let gl_context = not_current_gl_context
            .make_current(&gl_surface)
            .expect("Failed to make Gl context current...");

        let gl = unsafe {
            Context::from_loader_function(|symbol| {
                let c_str = CString::new(symbol).unwrap();
                gl_display.get_proc_address(&c_str)
            })
        };

        info!("OpenGL context successfully initialized!");

        // Create Triple Buffer & Renderer
        let (write_handle, read_handle) = new_triple_buffer::<FrameData>();
        let mut renderer = Renderer::new(gl, read_handle);

        // Load Assets
        let mesh_id = renderer.load_mesh(create_cube_mesh());
        let shader_map = renderer
            .load_shaders_from_dir(std::path::Path::new("shaders"))
            .expect("Failed to load shaders");
        let shader_id = *shader_map.get("basic").expect("Missing 'basic' shader");
        let material_id = renderer.create_material(shader_id);

        self.state = Some(DemoState {
            window,
            gl_context,
            gl_surface,
            renderer,
            write_handle,
            mesh_id,
            material_id,
            width: 1280,
            height: 720,
            camera_pos: Vec3::new(0.0, 2.0, 10.0), // Start slightly above and back
            camera_yaw: 0.0,
            camera_pitch: 0.0,
            keys: Keys::default(),
            cursor_grabbed: false,
            last_frame: Instant::now(),
            frame_count: 0,
            last_fps_update: Instant::now(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    state.width = size.width;
                    state.height = size.height;
                    state.gl_surface.resize(
                        &state.gl_context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );
                    state.renderer.resize(size.width as i32, size.height as i32);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match keycode {
                        KeyCode::KeyW => state.keys.w = pressed,
                        KeyCode::KeyA => state.keys.a = pressed,
                        KeyCode::KeyS => state.keys.s = pressed,
                        KeyCode::KeyD => state.keys.d = pressed,
                        KeyCode::Space => state.keys.space = pressed,
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => state.keys.shift = pressed,
                        KeyCode::Escape => {
                            if state.cursor_grabbed {
                                let _ = state.window.set_cursor_grab(CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                state.cursor_grabbed = false;
                            } else {
                                let _ = state
                                    .window
                                    .set_cursor_grab(CursorGrabMode::Confined)
                                    .or_else(|_| {
                                        state.window.set_cursor_grab(CursorGrabMode::Locked)
                                    });
                                state.window.set_cursor_visible(false);
                                state.cursor_grabbed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // FPS Tracking
                state.frame_count += 1;
                let now = Instant::now();
                let elapsed = (now - state.last_fps_update).as_secs_f32();
                if elapsed >= 1.0 {
                    let fps = state.frame_count as f32 / elapsed;
                    log::info!("FPS: {:.2} | Objects: {}", fps, 121); // 121 cubes in the grid
                    state.frame_count = 0;
                    state.last_fps_update = now;
                }

                // 1. Delta Time
                let now = Instant::now();
                let delta_time = (now - state.last_frame).as_secs_f32();
                state.last_frame = now;

                // 2. Update Camera Transform
                let camera_rotation = Quat::from_euler(
                    glam::EulerRot::YXZ,
                    state.camera_yaw,
                    state.camera_pitch,
                    0.0,
                );
                let forward = camera_rotation * Vec3::NEG_Z;
                let right = camera_rotation * Vec3::X;

                let mut velocity = Vec3::ZERO;
                if state.keys.w {
                    velocity += forward;
                }
                if state.keys.s {
                    velocity -= forward;
                }
                if state.keys.d {
                    velocity += right;
                }
                if state.keys.a {
                    velocity -= right;
                }
                if state.keys.space {
                    velocity += Vec3::Y;
                }
                if state.keys.shift {
                    velocity -= Vec3::Y;
                }

                if velocity.length_squared() > 0.0 {
                    velocity = velocity.normalize();
                }
                state.camera_pos += velocity * 5.0 * delta_time; // 5.0 units per second

                // 3. Build FrameData
                let frame = state.write_handle.write_slot();
                frame.commands.clear(); // Keep capacity, zero allocation!

                // Spawn an 11x11 grid of cubes (121 objects)
                for x in -5..=5 {
                    for z in -5..=5 {
                        let transform = Transform {
                            position: Vec3::new(x as f32 * 2.5, 0.0, z as f32 * 2.5),
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                        };
                        let model_matrix = transform_to_model_matrix(&transform);

                        frame.commands.push(RenderCommand {
                            mesh_id: state.mesh_id,
                            material_id: state.material_id,
                            model_matrix,
                            _padding: [0; 2],
                        });
                    }
                }

                // Set Camera Data
                frame.camera_position = state.camera_pos;
                frame.camera_rotation = camera_rotation;
                frame.camera_fov = std::f32::consts::FRAC_PI_4;
                frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
                frame.camera_near = 0.1;
                frame.camera_far = 100.0;

                state.write_handle.publish();

                // 4. Render & Swap
                state.renderer.render();
                state.gl_surface.swap_buffers(&state.gl_context).unwrap();
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(state) = &mut self.state {
            if let DeviceEvent::MouseMotion { delta } = event {
                if state.cursor_grabbed {
                    // Note: If the mouse feels inverted, change -= to +=
                    state.camera_yaw -= delta.0 as f32 * 0.002;
                    state.camera_pitch -= delta.1 as f32 * 0.002;
                    state.camera_pitch = state.camera_pitch.clamp(-1.5, 1.5); // Prevent flipping
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

// ==========================================
// 4. ENTRY POINT
// ==========================================

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(false); // Opaque background for 3D

    let mut app = DemoApp::new(template);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
