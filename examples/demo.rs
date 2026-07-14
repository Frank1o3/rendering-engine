use glow::Context;
use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext,
        PossiblyCurrentContext, Version,
    },
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SwapInterval, WindowSurface},
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
    MdiStrategy,
    engine::{MaterialId, MeshId, Renderer},
    frame_data::{FrameData, RenderCommand},
    mesh::{MeshData, Vertex},
    pipeline::PipelineState,
    scene::ObjectKind,
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
    lctrl: bool,
}
impl Default for Keys {
    fn default() -> Self {
        Self {
            w: false,
            a: false,
            s: false,
            d: false,
            space: false,
            lctrl: false,
        }
    }
}

struct DemoState {
    window: Arc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    renderer: Renderer,
    write_handle: WriteHandle<FrameData>,

    // UI State
    ui_mesh_id: MeshId,
    ui_material_id: MaterialId,
    current_fps: f32,

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
    pending_grab: bool,
}

impl DemoApp {
    pub fn new(template: ConfigTemplateBuilder) -> Self {
        Self {
            state: None,
            template,
            pending_grab: true,
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
    let face_normals = [
        [0, 0, 127, 0],  // Front (+Z)
        [0, 0, -127, 0], // Back (-Z)
        [0, 127, 0, 0],  // Top (+Y)
        [0, -127, 0, 0], // Bottom (-Y)
        [127, 0, 0, 0],  // Right (+X)
        [-127, 0, 0, 0], // Left (-X)
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
                normal: face_normals[i],
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

// A simple 1x1 white quad for UI rendering
fn create_quad_mesh() -> MeshData {
    const QUAD_NORMAL: [i8; 4] = [0, 0, 127, 0];

    let vertices = vec![
        Vertex {
            position: [0.0, 0.0, 0.0],
            normal: QUAD_NORMAL,
            color: [255, 255, 255, 255],
        },
        Vertex {
            position: [1.0, 0.0, 0.0],
            normal: QUAD_NORMAL,
            color: [255, 255, 255, 255],
        },
        Vertex {
            position: [1.0, 1.0, 0.0],
            normal: QUAD_NORMAL,
            color: [255, 255, 255, 255],
        },
        Vertex {
            position: [0.0, 1.0, 0.0],
            normal: QUAD_NORMAL,
            color: [255, 255, 255, 255],
        },
    ];
    let indices = vec![0, 1, 2, 2, 3, 0];
    MeshData { vertices, indices }
}

// A simple 3x5 bitmap font for digits 0-9
const FONT: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b010, 0b010, 0b010, 0b010], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

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
            .with_inner_size(PhysicalSize::new(1280, 720))
            .with_resizable(true);

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

        gl_surface
            .set_swap_interval(&gl_context, SwapInterval::DontWait)
            .expect("Failed to disable VSync");

        let gl = unsafe {
            Context::from_loader_function(|symbol| {
                let c_str = CString::new(symbol).unwrap();
                gl_display.get_proc_address(&c_str)
            })
        };

        info!("OpenGL context successfully initialized!");

        let grid_size = 50;
        let offset: f32 = 1.1;

        // Create Triple Buffer & Renderer
        let (write_handle, read_handle) = new_triple_buffer::<FrameData>();
        let mut renderer = Renderer::new(gl, read_handle);

        // Set Multi Draw Indirect Protocol
        renderer.set_mdi_strategy(MdiStrategy::Multi);

        // Load Assets
        let mesh_id = renderer.load_mesh(create_cube_mesh());
        let ui_mesh_id = renderer.load_mesh(create_quad_mesh());

        let shader_map = renderer
            .load_shaders_from_dir(std::path::Path::new("shaders"))
            .expect("Failed to load shaders");
        let shader_id = *shader_map.get("basic").expect("Missing 'basic' shader");
        let ui_shader_id = *shader_map.get("ui").expect("Missing 'ui' shader");

        let opaque_pipeline = PipelineState::default_opaque(shader_id.0);
        let material_id = renderer.create_material(shader_id, opaque_pipeline);

        let alpha_pipeline = PipelineState::default_alpha(ui_shader_id.0);
        let ui_material_id = renderer.create_material(ui_shader_id, alpha_pipeline);

        for x in -grid_size..=grid_size {
            for z in -grid_size..=grid_size {
                let position = Vec3::new(x as f32 * offset, 0.0, z as f32 * offset);
                let handle = renderer.add_object(mesh_id, material_id, ObjectKind::Static);
                renderer.set_transform(handle, position, Quat::IDENTITY, 1.0);
            }
        }

        self.state = Some(DemoState {
            window,
            gl_context,
            gl_surface,
            renderer,
            write_handle,
            ui_mesh_id,
            ui_material_id,
            current_fps: 0.0,
            width: 1280,
            height: 720,
            camera_pos: Vec3::new(0.0, 2.0, 10.0),
            camera_yaw: 0.0,
            camera_pitch: 0.0,
            keys: Keys::default(),
            cursor_grabbed: true,
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
                        KeyCode::ControlLeft => state.keys.lctrl = pressed,

                        KeyCode::Escape if pressed => {
                            if state.cursor_grabbed {
                                match state.window.set_cursor_grab(CursorGrabMode::None) {
                                    Ok(_) => {
                                        state.window.set_cursor_visible(true);
                                        state.cursor_grabbed = false;
                                    }
                                    Err(e) => {
                                        println!("failed to release cursor: {e:?}");
                                    }
                                }
                            } else {
                                self.pending_grab = true;
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
                    state.current_fps = state.frame_count as f32 / elapsed;
                    state.frame_count = 0;
                    state.last_fps_update = now;
                }

                if self.pending_grab {
                    self.pending_grab = false;

                    match state.window.set_cursor_grab(CursorGrabMode::Confined) {
                        Ok(_) => {
                            println!("confined");
                            state.window.set_cursor_visible(false);
                            state.cursor_grabbed = true;
                        }
                        Err(e) => {
                            println!("confined failed: {e:?}");

                            match state.window.set_cursor_grab(CursorGrabMode::Locked) {
                                Ok(_) => {
                                    println!("locked");
                                    state.window.set_cursor_visible(false);
                                    state.cursor_grabbed = true;
                                }
                                Err(e) => {
                                    println!("locked failed: {e:?}");
                                }
                            }
                        }
                    }
                }

                // 1. Delta Time
                let now = Instant::now();
                // Clamp to 100 ms max: if the app was paused or the first frame
                // took a long time to initialise, an unbounded delta would
                // teleport the camera by many units in one step.
                let delta_time = (now - state.last_frame).as_secs_f32().min(0.1);
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
                if state.keys.lctrl {
                    velocity -= Vec3::Y;
                }

                if velocity.length_squared() > 0.0 {
                    velocity = velocity.normalize();
                }
                state.camera_pos += velocity * 5.0 * delta_time;

                // 3. Build FrameData
                let frame = state.write_handle.write_slot();
                frame.commands.clear();
                frame.ui_commands.clear();

                // ==========================================
                // BUILD UI (Zero-Allocation Bitmap Font)
                // ==========================================
                let fps = state.current_fps as u32;
                let digits = [(fps / 100) % 10, (fps / 10) % 10, fps % 10];

                let pixel_size = 8.0;
                let square_size = 7.9;
                let mut cursor_x = 10.0;
                let cursor_y = 10.0;

                let mut started = false;
                for &d in &digits {
                    if d != 0 || started {
                        started = true;
                        let glyph = FONT[d as usize];

                        for (row_idx, &row) in glyph.iter().enumerate() {
                            for col_idx in 0..3 {
                                if (row & (1 << (2 - col_idx))) != 0 {
                                    let x = cursor_x + col_idx as f32 * pixel_size;
                                    let y = cursor_y + row_idx as f32 * pixel_size;

                                    frame.ui_commands.push(RenderCommand {
                                        mesh_id: state.ui_mesh_id,
                                        material_id: state.ui_material_id,
                                        position: glam::Vec3::new(x, y, 0.0),
                                        rotation: glam::Quat::IDENTITY,
                                        scale: square_size,
                                    });
                                }
                            }
                        }
                        cursor_x += 4.0 * pixel_size;
                    }
                }

                // Set Camera Data
                frame.camera_position = state.camera_pos;
                frame.camera_rotation = camera_rotation;
                frame.camera_fov = std::f32::consts::FRAC_PI_3;
                frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
                frame.camera_near = 0.1;
                frame.camera_far = 255.5;

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
                    state.camera_yaw -= delta.0 as f32 * 0.001;
                    state.camera_pitch -= delta.1 as f32 * 0.001;
                    state.camera_pitch = state.camera_pitch.clamp(-1.5, 1.5);
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

    // Request a 24-bit depth buffer explicitly. Without this the driver may
    // or may not give you one depending on the platform and GPU. The depth
    // test silently does nothing without a depth attachment.
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_depth_size(24) // ← ensure we always get a depth buffer
        .with_transparency(false);

    let mut app = DemoApp::new(template);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
