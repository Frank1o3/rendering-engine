// examples/demo_gles.rs
//
// Minimal OpenGL ES 3.2 demo: a single spinning, lit cube.
// Same input scheme as demo.rs: WASD + Space/LCtrl to fly, mouse to look,
// ESC to toggle cursor grab.
//
// The only real app-creation difference vs. the desktop demo is the
// requested context API/version below. Everything else — shader loading,
// materials, scene, render loop — goes through the same engine API.
// Shader *source* differs because GLSL ES requires a different #version
// pragma and mandatory precision qualifiers — see shaders_gles/.
use glow::Context;
use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version,
    },
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SwapInterval, WindowSurface},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use log::info;
use raw_window_handle::HasWindowHandle;
use std::collections::HashMap;
use std::{ffi::CString, num::NonZeroU32, sync::Arc, time::Instant};
use winit::event::TouchPhase;
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
    engine::{MaterialId, MeshId, Renderer, ShaderId},
    frame_data::{FrameData, RenderCommand},
    mesh::{MeshData, Vertex},
    pipeline::PipelineState,
    triple_buffer::{WriteHandle, new_triple_buffer},
};

mod shaders;

use shaders::SHADERS;

#[derive(Default)]
struct Keys {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    space: bool,
    lctrl: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ButtonId {
    Forward,
    Back,
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

const BTN: f32 = 72.0;
const GAP: f32 = 10.0;
const MARGIN: f32 = 24.0;

/// Screen-space (pixel, origin top-left) rects for the on-screen D-pad
/// (bottom-left) and up/down cluster (bottom-right). Recomputed every call
/// so it stays correct across resizes — cheap enough to not bother caching.
fn button_rects(width: f32, height: f32) -> [(ButtonId, Rect); 6] {
    let row_y = height - MARGIN - BTN;
    let a_x = MARGIN;
    let s_x = MARGIN + BTN + GAP;
    let d_x = MARGIN + 2.0 * (BTN + GAP);
    let w_y = row_y - BTN - GAP;

    let down_x = width - MARGIN - BTN;
    let down_y = height - MARGIN - BTN;
    let up_y = down_y - BTN - GAP;

    [
        (
            ButtonId::Forward,
            Rect {
                x: s_x,
                y: w_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Left,
            Rect {
                x: a_x,
                y: row_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Back,
            Rect {
                x: s_x,
                y: row_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Right,
            Rect {
                x: d_x,
                y: row_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Up,
            Rect {
                x: down_x,
                y: up_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Down,
            Rect {
                x: down_x,
                y: down_y,
                w: BTN,
                h: BTN,
            },
        ),
    ]
}

fn set_button_key(state: &mut DemoState, btn: ButtonId, val: bool) {
    match btn {
        ButtonId::Forward => state.keys.w = val,
        ButtonId::Back => state.keys.s = val,
        ButtonId::Left => state.keys.a = val,
        ButtonId::Right => state.keys.d = val,
        ButtonId::Up => state.keys.space = val,
        ButtonId::Down => state.keys.lctrl = val,
    }
}

/// What an in-progress touch is doing: held on a virtual button, or
/// dragging to look around (any touch that didn't start on a button).
enum TouchKind {
    Button(ButtonId),
    Look { last: (f32, f32) },
}

const TOUCH_LOOK_SENSITIVITY: f32 = 0.004;

struct DemoState {
    window: Arc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    renderer: Renderer,
    write_handle: WriteHandle<FrameData>,

    cube_mesh_id: MeshId,
    mat_lit: MaterialId,
    shader_lit: ShaderId,

    dyn_angle: f32,

    width: u32,
    height: u32,

    camera_pos: Vec3,
    camera_yaw: f32,
    camera_pitch: f32,
    keys: Keys,
    cursor_grabbed: bool,

    last_frame: Instant,
    quad_mesh_id: MeshId,
    button_quad_mesh_id: MeshId,
    ui_material_id: MaterialId,

    current_fps: f32,
    frame_count: u32,
    last_fps_update: Instant,

    touches: HashMap<u64, TouchKind>,
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

// Identical geometry to demo.rs's create_cube_mesh — CCW winding,
// explicit per-face normals, one color per face.
fn create_cube_mesh() -> MeshData {
    let r = [220u8, 60, 60, 255];
    let g = [60u8, 200, 60, 255];
    let b = [60u8, 60, 220, 255];
    let y = [220u8, 200, 40, 255];
    let m = [180u8, 60, 220, 255];
    let c = [40u8, 200, 220, 255];

    let mut v = Vec::with_capacity(24);

    v.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, 0.0, 1.0], r));
    v.push(Vertex::new([0.5, -0.5, 0.5], [0.0, 0.0, 1.0], r));
    v.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], r));
    v.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 0.0, 1.0], r));

    v.push(Vertex::new([0.5, -0.5, -0.5], [0.0, 0.0, -1.0], g));
    v.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], g));
    v.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 0.0, -1.0], g));
    v.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 0.0, -1.0], g));

    v.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 1.0, 0.0], b));
    v.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], b));
    v.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 1.0, 0.0], b));
    v.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 1.0, 0.0], b));

    v.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0], y));
    v.push(Vertex::new([0.5, -0.5, -0.5], [0.0, -1.0, 0.0], y));
    v.push(Vertex::new([0.5, -0.5, 0.5], [0.0, -1.0, 0.0], y));
    v.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, -1.0, 0.0], y));

    v.push(Vertex::new([0.5, -0.5, 0.5], [1.0, 0.0, 0.0], m));
    v.push(Vertex::new([0.5, -0.5, -0.5], [1.0, 0.0, 0.0], m));
    v.push(Vertex::new([0.5, 0.5, -0.5], [1.0, 0.0, 0.0], m));
    v.push(Vertex::new([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], m));

    v.push(Vertex::new([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0], c));
    v.push(Vertex::new([-0.5, -0.5, 0.5], [-1.0, 0.0, 0.0], c));
    v.push(Vertex::new([-0.5, 0.5, 0.5], [-1.0, 0.0, 0.0], c));
    v.push(Vertex::new([-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0], c));

    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let b = face * 4;
        indices.extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 3, b]);
    }

    MeshData {
        vertices: v,
        indices,
    }
}

fn create_quad_mesh() -> MeshData {
    let n: [i8; 4] = [0, 0, 127, 0];
    let vertices = vec![
        Vertex {
            position: [0.0, 0.0, 0.0],
            normal: n,
            color: [255, 255, 255, 255],
        },
        Vertex {
            position: [1.0, 0.0, 0.0],
            normal: n,
            color: [255, 255, 255, 255],
        },
        Vertex {
            position: [1.0, 1.0, 0.0],
            normal: n,
            color: [255, 255, 255, 255],
        },
        Vertex {
            position: [0.0, 1.0, 0.0],
            normal: n,
            color: [255, 255, 255, 255],
        },
    ];
    MeshData {
        vertices,
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}

/// Same unit quad, but translucent dark gray — used for the on-screen
/// touch button backgrounds so they read as buttons without occluding
/// the scene behind them.
fn create_button_quad_mesh() -> MeshData {
    let n: [i8; 4] = [0, 0, 127, 0];
    let col = [70u8, 70, 80, 140]; // alpha < 255 → translucent via ui_material's blend state
    let vertices = vec![
        Vertex {
            position: [0.0, 0.0, 0.0],
            normal: n,
            color: col,
        },
        Vertex {
            position: [1.0, 0.0, 0.0],
            normal: n,
            color: col,
        },
        Vertex {
            position: [1.0, 1.0, 0.0],
            normal: n,
            color: col,
        },
        Vertex {
            position: [0.0, 1.0, 0.0],
            normal: n,
            color: col,
        },
    ];
    MeshData {
        vertices,
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}

const FONT: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b010, 0b010, 0b010, 0b010],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn char_glyph(ch: char) -> Option<[u8; 5]> {
    match ch.to_ascii_uppercase() {
        'F' => Some([0b111, 0b100, 0b111, 0b100, 0b100]),
        'I' => Some([0b111, 0b010, 0b010, 0b010, 0b111]),
        'P' => Some([0b111, 0b101, 0b111, 0b100, 0b100]),
        'S' => Some([0b111, 0b100, 0b111, 0b001, 0b111]),
        _ => None,
    }
}

fn emit_ui_text(
    cmds: &mut Vec<RenderCommand>,
    mesh_id: MeshId,
    material_id: MaterialId,
    mut x: f32,
    y: f32,
    text: &str,
    pixel_size: f32,
) {
    for ch in text.chars() {
        if ch == ' ' {
            x += 4.0 * pixel_size;
            continue;
        }
        if ch == ':' {
            for &row in &[1u32, 3] {
                cmds.push(RenderCommand {
                    mesh_id,
                    material_id,
                    position: Vec3::new(x + pixel_size, y + row as f32 * pixel_size, 0.0),
                    rotation: Quat::IDENTITY,
                    scale: pixel_size * 0.9,
                });
            }
            x += 3.0 * pixel_size;
            continue;
        }
        let glyph = if ch.is_ascii_digit() {
            Some(FONT[(ch as u8 - b'0') as usize])
        } else {
            char_glyph(ch)
        };
        if let Some(rows) = glyph {
            for (row_idx, &row) in rows.iter().enumerate() {
                for col in 0..3u32 {
                    if (row & (1 << (2 - col))) != 0 {
                        cmds.push(RenderCommand {
                            mesh_id,
                            material_id,
                            position: Vec3::new(
                                x + col as f32 * pixel_size,
                                y + row_idx as f32 * pixel_size,
                                0.0,
                            ),
                            rotation: Quat::IDENTITY,
                            scale: pixel_size * 0.9,
                        });
                    }
                }
            }
        }
        x += 4.0 * pixel_size;
    }
}

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("Rendering Engine — OpenGL ES 3.2 3D Demo")
            .with_inner_size(PhysicalSize::new(1280u32, 720u32))
            .with_resizable(false);

        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));
        info!("Creating DisplayBuilder");

        let (window, gl_config) = display_builder
            .build(event_loop, self.template.clone(), |configs| {
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
            .expect("Failed to bootstrap GL");

        let window = Arc::new(window.expect("Failed to create window"));
        let gl_display = gl_config.display();
        let window_handle = window.window_handle().unwrap();

        // ── The one real app-creation difference vs. the desktop demo ───────
        // Request OpenGL ES 3.2 instead of desktop GL 4.6. No GlProfile call —
        // ES has no Core/Compatibility distinction to select.
        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(Version { major: 3, minor: 2 })))
            .build(Some(window_handle.as_raw()));
        info!("Context created");

        let not_current = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .expect("Failed to create GLES 3.2 context — check driver/device support")
        };

        let attrs = window.build_surface_attributes(Default::default()).unwrap();
        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
                .expect("Failed to create window surface")
        };

        let gl_context = not_current
            .make_current(&gl_surface)
            .expect("Failed to make context current");

        gl_surface
            .set_swap_interval(&gl_context, SwapInterval::DontWait)
            .expect("Failed to disable VSync");

        let gl = unsafe {
            Context::from_loader_function(|sym| {
                let c = CString::new(sym).unwrap();
                gl_display.get_proc_address(&c)
            })
        };

        info!("OpenGL ES context initialised");

        let (write_handle, read_handle) = new_triple_buffer::<FrameData>();
        let mut renderer = Renderer::new(gl, read_handle);

        let cube_mesh_id = renderer.load_mesh(create_cube_mesh());

        let shader_map = renderer
            .load_shaders_from_include_dir(&SHADERS)
            .expect("Failed to load shaders from ./shaders_gles/");
        let shader_lit = *shader_map
            .get("lit")
            .expect("Missing 'lit' GLES shader — check shaders_gles/lit.vert + .frag exist");

        let mat_lit =
            renderer.create_material(shader_lit, PipelineState::default_opaque(shader_lit.0));
        let quad_mesh_id = renderer.load_mesh(create_quad_mesh());
        let button_quad_mesh_id = renderer.load_mesh(create_button_quad_mesh());

        let shader_ui = *shader_map
            .get("ui")
            .expect("Missing 'ui' GLES shader — check shaders_gles/ui.vert + .frag exist");
        let ui_pipeline = PipelineState {
            shader_id: shader_ui.0,
            cull_mode: rendering_engine::pipeline::CullMode::None,
            depth_test: true,
            depth_write: false,
            depth_func: rendering_engine::pipeline::DepthFunc::Less,
            blend_enabled: true,
            src_factor: rendering_engine::pipeline::BlendFactor::SrcAlpha,
            dst_factor: rendering_engine::pipeline::BlendFactor::OneMinusSrcAlpha,
        };
        let ui_material_id = renderer.create_material(shader_ui, ui_pipeline);

        self.state = Some(DemoState {
            window,
            gl_context,
            gl_surface,
            renderer,
            write_handle,

            cube_mesh_id,
            mat_lit,
            shader_lit,

            dyn_angle: 0.0,

            width: 1280,
            height: 720,

            camera_pos: Vec3::new(0.0, 1.0, 5.0),
            camera_yaw: 0.0,
            camera_pitch: 0.0,
            keys: Keys::default(),
            cursor_grabbed: true,

            last_frame: Instant::now(),
            quad_mesh_id,
            button_quad_mesh_id,
            ui_material_id,

            current_fps: 0.0,
            frame_count: 0,
            last_fps_update: Instant::now(),

            touches: HashMap::new(),
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
            WindowEvent::CloseRequested => event_loop.exit(),

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
                if let PhysicalKey::Code(kc) = event.physical_key {
                    match kc {
                        KeyCode::KeyW => state.keys.w = pressed,
                        KeyCode::KeyA => state.keys.a = pressed,
                        KeyCode::KeyS => state.keys.s = pressed,
                        KeyCode::KeyD => state.keys.d = pressed,
                        KeyCode::Space => state.keys.space = pressed,
                        KeyCode::ControlLeft => state.keys.lctrl = pressed,

                        KeyCode::Escape if pressed => {
                            if state.cursor_grabbed {
                                let _ = state.window.set_cursor_grab(CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                state.cursor_grabbed = false;
                            } else {
                                self.pending_grab = true;
                            }
                        }
                        _ => {}
                    }
                }
            }

            WindowEvent::Touch(touch) => {
                let x = touch.location.x as f32;
                let y = touch.location.y as f32;

                match touch.phase {
                    TouchPhase::Started => {
                        let hit = button_rects(state.width as f32, state.height as f32)
                            .into_iter()
                            .find(|(_, rect)| rect.contains(x, y));

                        if let Some((btn, _)) = hit {
                            set_button_key(state, btn, true);
                            state.touches.insert(touch.id, TouchKind::Button(btn));
                        } else {
                            state
                                .touches
                                .insert(touch.id, TouchKind::Look { last: (x, y) });
                        }
                    }
                    TouchPhase::Moved => {
                        if let Some(TouchKind::Look { last }) = state.touches.get_mut(&touch.id) {
                            let dx = x - last.0;
                            let dy = y - last.1;
                            state.camera_yaw -= dx * TOUCH_LOOK_SENSITIVITY;
                            state.camera_pitch -= dy * TOUCH_LOOK_SENSITIVITY;
                            state.camera_pitch = state.camera_pitch.clamp(-1.5, 1.5);
                            *last = (x, y);
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        if let Some(TouchKind::Button(btn)) = state.touches.remove(&touch.id) {
                            set_button_key(state, btn, false);
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => {
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
                    if state
                        .window
                        .set_cursor_grab(CursorGrabMode::Confined)
                        .is_ok()
                        || state.window.set_cursor_grab(CursorGrabMode::Locked).is_ok()
                    {
                        state.window.set_cursor_visible(false);
                        state.cursor_grabbed = true;
                    }
                }

                let now = Instant::now();
                let dt = (now - state.last_frame).as_secs_f32().min(0.1);
                state.last_frame = now;

                let cam_rot = Quat::from_euler(
                    glam::EulerRot::YXZ,
                    state.camera_yaw,
                    state.camera_pitch,
                    0.0,
                );
                let forward = cam_rot * Vec3::NEG_Z;
                let right = cam_rot * Vec3::X;

                let mut vel = Vec3::ZERO;
                if state.keys.w {
                    vel += forward;
                }
                if state.keys.s {
                    vel -= forward;
                }
                if state.keys.d {
                    vel += right;
                }
                if state.keys.a {
                    vel -= right;
                }
                if state.keys.space {
                    vel += Vec3::Y;
                }
                if state.keys.lctrl {
                    vel -= Vec3::Y;
                }
                if vel.length_squared() > 0.0 {
                    vel = vel.normalize();
                }
                state.camera_pos += vel * 5.0 * dt;

                state.dyn_angle += dt * 1.2;
                let dyn_rot = Quat::from_rotation_y(state.dyn_angle)
                    * Quat::from_rotation_x(state.dyn_angle * 0.7);

                let frame = state.write_handle.write_slot();
                frame.commands.clear();
                frame.ui_commands.clear();
                frame.commands.push(RenderCommand {
                    mesh_id: state.cube_mesh_id,
                    material_id: state.mat_lit,
                    position: Vec3::ZERO,
                    rotation: dyn_rot,
                    scale: 1.0,
                });
                frame.camera_position = state.camera_pos;
                frame.camera_rotation = cam_rot;
                frame.camera_fov = std::f32::consts::FRAC_PI_3;
                frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
                frame.camera_near = 0.1;
                frame.camera_far = 100.0;

                emit_ui_text(
                    &mut frame.ui_commands,
                    state.quad_mesh_id,
                    state.ui_material_id,
                    10.0,
                    10.0,
                    &format!("{:.0} FPS", state.current_fps),
                    8.0,
                );

                // On-screen touch controls — only meaningful (and only drawn)
                // when actually running on a touch device.
                if cfg!(target_os = "android") {
                    for (_, rect) in button_rects(state.width as f32, state.height as f32) {
                        frame.ui_commands.push(RenderCommand {
                            mesh_id: state.button_quad_mesh_id,
                            material_id: state.ui_material_id,
                            position: Vec3::new(rect.x, rect.y, 0.0),
                            rotation: Quat::IDENTITY,
                            scale: rect.w, // square buttons — InstanceData.scale is uniform
                        });
                    }
                }

                state.write_handle.publish();

                state.renderer.upload_shader_vec3(
                    state.shader_lit,
                    "uSunDir",
                    Vec3::new(0.6, 0.8, 0.4).normalize(),
                );
                state
                    .renderer
                    .upload_shader_f32(state.shader_lit, "uAmbient", 0.18);

                state.renderer.render();
                state.gl_surface.swap_buffers(&state.gl_context).unwrap();
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: winit::event::DeviceId, event: DeviceEvent) {
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

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }
}

#[unsafe(no_mangle)]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    env_logger::init();

    let event_loop = EventLoop::builder().with_android_app(app).build().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let template = ConfigTemplateBuilder::new().with_depth_size(24);

    let mut demo = DemoApp::new(template);
    event_loop.run_app(&mut demo).unwrap();
}
