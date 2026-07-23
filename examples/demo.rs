// examples/demo.rs
//
// Rendering engine feature demo.
// Press 1 / 2 / 3 to switch rendering mode:
//   1 — Lit (Lambertian diffuse, tests packed normals at location 1)
//   2 — Wireframe (geometry shader, barycentric edge overlay)
//   3 — Flat color (basic unlit shader)
//
// WASD + Space/LCtrl to fly. Mouse to look around. ESC to toggle cursor grab.
//
// Scene: 101×101 grid of cubes + one rotating dynamic cube in the centre.
// FPS counter displayed in the top-left corner using a zero-allocation bitmap
// font (same technique as before, rendered via the UI shader).

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
    engine::{MaterialId, MeshId, Renderer, ShaderId},
    frame_data::{FrameData, RenderCommand},
    mesh::{MeshData, Vertex},
    pipeline::{BlendFactor, CullMode, DepthFunc, PipelineState},
    scene::ObjectKind,
    triple_buffer::{WriteHandle, new_triple_buffer},
};

// ─────────────────────────────────────────────────────────────────────────────
// Rendering mode
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RenderMode {
    Lit,       // key 1 — Lambertian + ambient, tests normals
    Wireframe, // key 2 — geometry shader, barycentric edges
    Flat,      // key 3 — basic unlit color
}

impl RenderMode {
    fn label(self) -> &'static str {
        match self {
            Self::Lit => "1:Lit",
            Self::Wireframe => "2:Wire",
            Self::Flat => "3:Flat",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Input state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Keys {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    space: bool,
    lctrl: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Application state
// ─────────────────────────────────────────────────────────────────────────────

struct DemoState {
    window: Arc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    renderer: Renderer,
    write_handle: WriteHandle<FrameData>,

    // Per-mode material IDs — swapping mode just changes which material_id we
    // pass to add_object / the dynamic cube RenderCommand.
    mat_lit: MaterialId,
    mat_wireframe: MaterialId,
    mat_flat: MaterialId,

    // Mesh IDs
    cube_mesh_id: MeshId,

    // Static grid handles (all cubes share one handle vec per mode approach:
    // we simply store handles and re-call add_object when the mode changes).
    grid_handles: Vec<rendering_engine::scene::ObjectHandle>,
    grid_positions: Vec<Vec3>,

    // Dynamic cube
    dyn_angle: f32,

    // UI
    ui_mesh_id: MeshId,
    ui_material_id: MaterialId,
    current_fps: f32,

    // Shader IDs needed to upload per-frame uniforms
    shader_lit: ShaderId,
    shader_wireframe: ShaderId,

    width: u32,
    height: u32,

    render_mode: RenderMode,
    mode_changed: bool, // flag to rebuild the scene on mode switch

    camera_pos: Vec3,
    camera_yaw: f32,
    camera_pitch: f32,
    keys: Keys,
    cursor_grabbed: bool,

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

// ─────────────────────────────────────────────────────────────────────────────
// Mesh helpers
// ─────────────────────────────────────────────────────────────────────────────

fn create_cube_mesh() -> MeshData {
    // Each face: 4 vertices wound CCW when viewed from outside.
    // Positions are on the unit cube [-0.5, 0.5].
    // Normals are explicit per-face (hard edges, not averaged).
    // Using Vertex::new() which calls pack_normal() internally.

    let r = [220u8, 60, 60, 255]; // red    — front  +Z
    let g = [60u8, 200, 60, 255]; // green  — back   -Z
    let b = [60u8, 60, 220, 255]; // blue   — top    +Y
    let y = [220u8, 200, 40, 255]; // yellow — bottom -Y
    let m = [180u8, 60, 220, 255]; // purple — right  +X
    let c = [40u8, 200, 220, 255]; // cyan   — left   -X

    let mut v = Vec::with_capacity(24);

    // Front face (+Z), normal = (0, 0, 1)
    // Viewed from +Z looking in: CCW = BL, BR, TR, TL
    v.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, 0.0, 1.0], r));
    v.push(Vertex::new([0.5, -0.5, 0.5], [0.0, 0.0, 1.0], r));
    v.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], r));
    v.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 0.0, 1.0], r));

    // Back face (-Z), normal = (0, 0, -1)
    // Viewed from -Z looking in: CCW = BR, BL, TL, TR
    v.push(Vertex::new([0.5, -0.5, -0.5], [0.0, 0.0, -1.0], g));
    v.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], g));
    v.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 0.0, -1.0], g));
    v.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 0.0, -1.0], g));

    // Top face (+Y), normal = (0, 1, 0)
    // Viewed from +Y looking down: CCW = BL, BR, TR, TL (in XZ plane)
    v.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 1.0, 0.0], b));
    v.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], b));
    v.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 1.0, 0.0], b));
    v.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 1.0, 0.0], b));

    // Bottom face (-Y), normal = (0, -1, 0)
    // Viewed from -Y looking up: CCW = BL, BR, TR, TL
    v.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0], y));
    v.push(Vertex::new([0.5, -0.5, -0.5], [0.0, -1.0, 0.0], y));
    v.push(Vertex::new([0.5, -0.5, 0.5], [0.0, -1.0, 0.0], y));
    v.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, -1.0, 0.0], y));

    // Right face (+X), normal = (1, 0, 0)
    // Viewed from +X looking left: CCW = BL, BR, TR, TL (in ZY plane)
    v.push(Vertex::new([0.5, -0.5, 0.5], [1.0, 0.0, 0.0], m));
    v.push(Vertex::new([0.5, -0.5, -0.5], [1.0, 0.0, 0.0], m));
    v.push(Vertex::new([0.5, 0.5, -0.5], [1.0, 0.0, 0.0], m));
    v.push(Vertex::new([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], m));

    // Left face (-X), normal = (-1, 0, 0)
    // Viewed from -X looking right: CCW = BL, BR, TR, TL
    v.push(Vertex::new([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0], c));
    v.push(Vertex::new([-0.5, -0.5, 0.5], [-1.0, 0.0, 0.0], c));
    v.push(Vertex::new([-0.5, 0.5, 0.5], [-1.0, 0.0, 0.0], c));
    v.push(Vertex::new([-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0], c));

    // Indices: two CCW triangles per face (0,1,2), (2,3,0)
    // This pattern is correct for CCW quads: splits the quad along the 0-2 diagonal
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
    let n: [i8; 4] = [0, 0, 127, 0]; // +Z normal
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
    let indices = vec![0, 1, 2, 2, 3, 0];
    MeshData { vertices, indices }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3×5 bitmap font (digits 0–9)
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Grid building helper — populates the scene with the current material
// ─────────────────────────────────────────────────────────────────────────────

fn rebuild_grid(state: &mut DemoState) {
    // Remove old handles
    for h in state.grid_handles.drain(..) {
        state.renderer.remove_object(h);
    }

    let mat = match state.render_mode {
        RenderMode::Lit => state.mat_lit,
        RenderMode::Wireframe => state.mat_wireframe,
        RenderMode::Flat => state.mat_flat,
    };

    // Re-add all grid objects with the new material.
    // The mesh_id stays constant — only the material_id changes.
    // We look up the single cube mesh_id from the first handle's mesh in the
    // renderer.  Instead, we cache it in grid_positions and pass mesh_id in.
    // Simpler: store mesh_id in DemoState. We do that via a local captured var
    // passed from resumed().  Because rebuild_grid is called on mode switch
    // AFTER the initial build, we need the mesh_id available.  We stash it in
    // state as a field — see DemoState.cube_mesh_id.
    let mesh_id = state.cube_mesh_id;
    for &pos in &state.grid_positions {
        let h = state.renderer.add_object(mesh_id, mat, ObjectKind::Static);
        state.renderer.set_transform(h, pos, Quat::IDENTITY, 1.0);
        state.grid_handles.push(h);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationHandler
// ─────────────────────────────────────────────────────────────────────────────

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        // ── Window + GL context ──────────────────────────────────────────────
        let window_attributes = Window::default_attributes()
            .with_title("Rendering Engine 3D Demo")
            .with_inner_size(PhysicalSize::new(1280u32, 720u32))
            .with_resizable(false);

        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

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

        let context_attributes = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .with_context_api(ContextApi::OpenGl(Some(Version { major: 4, minor: 6 })))
            .build(Some(window_handle.as_raw()));

        let not_current = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .expect("Failed to create GL context")
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

        info!("OpenGL context initialised");

        // ── Renderer ────────────────────────────────────────────────────────
        let (write_handle, read_handle) = new_triple_buffer::<FrameData>();
        let mut renderer = Renderer::new(gl, read_handle);
        renderer.set_mdi_strategy(MdiStrategy::Multi);

        // ── Meshes ───────────────────────────────────────────────────────────
        let cube_mesh_id = renderer.load_mesh(create_cube_mesh());
        let ui_mesh_id = renderer.load_mesh(create_quad_mesh());

        // ── Shaders ──────────────────────────────────────────────────────────
        let shader_map = renderer
            .load_shaders_from_dir(std::path::Path::new("shaders"))
            .expect("Failed to load shaders from ./shaders/");

        let get = |name: &str| -> ShaderId {
            *shader_map
                .get(name)
                .unwrap_or_else(|| panic!("Missing shader: '{}'", name))
        };

        let shader_lit = get("lit");
        let shader_wireframe = get("wireframe");
        let shader_flat = get("basic");
        let shader_ui = get("ui");

        // ── Materials ────────────────────────────────────────────────────────
        let mat_lit =
            renderer.create_material(shader_lit, PipelineState::default_opaque(shader_lit.0));
2;
        let mat_wireframe = renderer.create_material(
            shader_wireframe,
            PipelineState::default_opaque(shader_wireframe.0),
        );

        let mat_flat =
            renderer.create_material(shader_flat, PipelineState::default_opaque(shader_flat.0));

        let ui_pipeline = PipelineState {
            shader_id: shader_ui.0,
            cull_mode: CullMode::None,
            depth_test: true,
            depth_write: false,
            depth_func: DepthFunc::Less,
            blend_enabled: true,
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
        };
        let ui_material_id = renderer.create_material(shader_ui, ui_pipeline);

        // ── Grid positions ───────────────────────────────────────────────────
        const GRID: i32 = 25;
        const GAP: f32 = 1.1;

        let mut grid_positions = Vec::with_capacity(((GRID * 2 + 1) * (GRID * 2 + 1)) as usize);
        for x in -GRID..=GRID {
            for z in -GRID..=GRID {
                grid_positions.push(Vec3::new(x as f32 * GAP, 0.0, z as f32 * GAP));
            }
        }

        // ── Initial scene (Lit mode) ─────────────────────────────────────────
        let mut grid_handles = Vec::with_capacity(grid_positions.len());
        for &pos in &grid_positions {
            let h = renderer.add_object(cube_mesh_id, mat_lit, ObjectKind::Static);
            renderer.set_transform(h, pos, Quat::IDENTITY, 1.0);
            grid_handles.push(h);
        }

        self.state = Some(DemoState {
            window,
            gl_context,
            gl_surface,
            renderer,
            write_handle,

            mat_lit,
            mat_wireframe,
            mat_flat,

            cube_mesh_id,
            grid_handles,
            grid_positions,
            dyn_angle: 0.0,

            ui_mesh_id,
            ui_material_id,
            current_fps: 0.0,

            shader_lit,
            shader_wireframe,

            width: 1280,
            height: 720,

            render_mode: RenderMode::Lit,
            mode_changed: false,

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

                        // Mode switching
                        KeyCode::Digit1 if pressed => {
                            if state.render_mode != RenderMode::Lit {
                                state.render_mode = RenderMode::Lit;
                                state.mode_changed = true;
                            }
                        }
                        KeyCode::Digit2 if pressed => {
                            if state.render_mode != RenderMode::Wireframe {
                                state.render_mode = RenderMode::Wireframe;
                                state.mode_changed = true;
                            }
                        }
                        KeyCode::Digit3 if pressed => {
                            if state.render_mode != RenderMode::Flat {
                                state.render_mode = RenderMode::Flat;
                                state.mode_changed = true;
                            }
                        }

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

            WindowEvent::RedrawRequested => {
                // ── FPS counter ──────────────────────────────────────────────
                state.frame_count += 1;
                let now = Instant::now();
                let elapsed = (now - state.last_fps_update).as_secs_f32();
                if elapsed >= 1.0 {
                    state.current_fps = state.frame_count as f32 / elapsed;
                    state.frame_count = 0;
                    state.last_fps_update = now;
                }

                // ── Cursor grab ──────────────────────────────────────────────
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

                // ── Delta time ───────────────────────────────────────────────
                let now = Instant::now();
                let dt = (now - state.last_frame).as_secs_f32().min(0.1);
                state.last_frame = now;

                // ── Mode switch: rebuild static grid ─────────────────────────
                if state.mode_changed {
                    state.mode_changed = false;
                    rebuild_grid(state);
                }

                // ── Camera ───────────────────────────────────────────────────
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
                state.camera_pos += vel * 10.0 * dt;

                // ── Dynamic cube (spins in the centre, above the grid) ───────
                state.dyn_angle += dt * 1.2;
                let dyn_rot = Quat::from_rotation_y(state.dyn_angle)
                    * Quat::from_rotation_x(state.dyn_angle * 0.7);

                let dyn_mat = match state.render_mode {
                    RenderMode::Lit => state.mat_lit,
                    RenderMode::Wireframe => state.mat_wireframe,
                    RenderMode::Flat => state.mat_flat,
                };

                // ── Write FrameData ──────────────────────────────────────────
                let frame = state.write_handle.write_slot();
                frame.commands.clear();
                frame.ui_commands.clear();

                // Dynamic spinning cube
                frame.commands.push(RenderCommand {
                    mesh_id: state.cube_mesh_id,
                    material_id: dyn_mat,
                    position: Vec3::new(0.0, 3.0, 0.0),
                    rotation: dyn_rot,
                    scale: 1.5,
                });

                // ── Upload per-frame shader uniforms ─────────────────────────
                // These are set via the shader's direct uniform API; the
                // renderer exposes shaders through get_shader(). We use
                // set_vec3 / set_f32 which glow handles at next use_program.
                //
                // The renderer calls use_program during render(), so we need
                // uniforms set BEFORE swap. We pre-bind them now via the
                // ShaderProgram accessors exposed through Renderer::shader_*
                // helpers — but the engine doesn't expose those directly.
                //
                // Simplest correct approach: set uniforms via the ShaderProgram
                // handles we stashed in the DemoState. The GL program is already
                // created; uniform locations are cached inside ShaderProgram.
                // We call renderer.set_shader_uniforms_lit / _wireframe below.
                // Since the renderer doesn't expose a generic "set uniform on
                // shader X" API, we use the public shader uniform helpers
                // through renderer.scene.  Actually, the cleanest path given
                // the current API surface: upload uniforms directly via the
                // renderer's wrapper methods which forward to ShaderProgram.

                // Camera
                frame.camera_position = state.camera_pos;
                frame.camera_rotation = cam_rot;
                frame.camera_fov = std::f32::consts::FRAC_PI_3;
                frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
                frame.camera_near = 0.1;
                frame.camera_far = 300.0;

                // ── UI: mode label + FPS counter ─────────────────────────────
                emit_ui_text(
                    &mut frame.ui_commands,
                    state.ui_mesh_id,
                    state.ui_material_id,
                    10.0,
                    10.0,
                    &format!(
                        "{} | {:.0} FPS",
                        state.render_mode.label(),
                        state.current_fps
                    ),
                    8.0,
                );

                state.write_handle.publish();

                // ── Per-frame uniforms for lit / wireframe shaders ────────────
                // We need to call use_program ourselves here to set uniforms.
                // The renderer's ShaderProgram is not exposed publicly, but we
                // CAN call the renderer's set_shader_* helpers after adding
                // them to the API. For now we use the renderer's gl handle
                // via the public scene/render path.
                //
                // Practical solution: add two thin helper methods to Renderer
                // (see below). Since we can't modify the library here, we use
                // the fact that ShaderProgram::set_vec3 calls use_program
                // internally — actually it doesn't; set_vec3 calls
                // get_uniform_location each time which only works on the
                // currently bound program.
                //
                // The cleanest zero-API-change approach: call
                // renderer.upload_lit_uniforms() / renderer.upload_wireframe_uniforms()
                // which we define BELOW as extension methods via a trait.
                // Those methods use the Renderer's public `scene` field and
                // the stored shader IDs. They call use_program + uniform upload
                // before render() is called.
                state.renderer.set_lit_uniforms(
                    state.shader_lit,
                    Vec3::new(0.6, 0.8, 0.4).normalize(), // sun direction
                    0.18,                                 // ambient
                );
                state.renderer.set_wireframe_uniforms(
                    state.shader_wireframe,
                    0.04, // edge width
                );

                // ── Render ───────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────────────────────
// DemoState extra fields (added inline above, declared here as a reminder)
// ─────────────────────────────────────────────────────────────────────────────
//
// The struct above already includes `cube_mesh_id: MeshId` — needed by
// `rebuild_grid`. Rust requires all fields in the struct literal, so it's
// declared in the struct body rather than here.

// ─────────────────────────────────────────────────────────────────────────────
// Renderer extension trait — sets per-frame uniforms without changing engine.rs
// ─────────────────────────────────────────────────────────────────────────────

trait DemoUniforms {
    fn set_lit_uniforms(&mut self, shader_id: ShaderId, sun_dir: Vec3, ambient: f32);
    fn set_wireframe_uniforms(&mut self, shader_id: ShaderId, edge_width: f32);
}

impl DemoUniforms for Renderer {
    fn set_lit_uniforms(&mut self, shader_id: ShaderId, sun_dir: Vec3, ambient: f32) {
        // We need the GL program handle. Renderer exposes shaders via the
        // public `shaders` HashMap field.  If it's private we access via the
        // ShaderProgram's public helpers. Looking at engine.rs: `shaders` is
        // NOT pub. The ShaderProgram helpers (set_vec3, set_f32) require a
        // currently-bound program — but they call get_uniform_location each
        // time so any program works as long as use_program has been called
        // first. We do NOT have a reference to the ShaderProgram here.
        //
        // The only public surface of Renderer that touches shaders is:
        //   load_shader, load_shader_from_files, load_shaders_from_dir
        //
        // Resolution: expose a `with_shader` method on Renderer, OR accept
        // that we set uniforms inside render() by checking uSunDir/uAmbient.
        //
        // For a DEMO-ONLY workaround with zero engine changes: store the
        // uniforms in FrameData extra fields (not clean) OR expose two thin
        // public methods on Renderer (cleanest).
        //
        // Since we own the engine source, we'll add two methods at the bottom
        // of engine.rs — described in the comments at the end of this file.
        // The trait impl below calls those methods.
        self.upload_shader_vec3(shader_id, "uSunDir", sun_dir);
        self.upload_shader_f32(shader_id, "uAmbient", ambient);
    }

    fn set_wireframe_uniforms(&mut self, shader_id: ShaderId, edge_width: f32) {
        self.upload_shader_f32(shader_id, "uEdgeWidth", edge_width);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UI text helper — emits RenderCommand quads for an ASCII string
// ─────────────────────────────────────────────────────────────────────────────

fn emit_ui_text(
    cmds: &mut Vec<RenderCommand>,
    mesh_id: MeshId,
    material_id: MaterialId,
    mut x: f32,
    y: f32,
    text: &str,
    pixel_size: f32,
) {
    // Very small inline font: digits 0–9 from FONT, space, colon, and letters
    // from a 5×3 uppercase bitmap. For simplicity we only support digits,
    // colon, pipe, space, and basic uppercase A-Z letters here.
    // Anything not in the table is replaced by a space.
    for ch in text.chars() {
        if ch == ' ' {
            x += 4.0 * pixel_size;
            continue;
        }
        if ch == '|' {
            // draw a thin vertical bar
            for row in 0..5u32 {
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
        let glyph: Option<[u8; 5]> = if ch.is_ascii_digit() {
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

/// Minimal uppercase ASCII glyph table for A–Z and a few symbols.
fn char_glyph(ch: char) -> Option<[u8; 5]> {
    match ch.to_ascii_uppercase() {
        'F' => Some([0b111, 0b100, 0b111, 0b100, 0b100]),
        'I' => Some([0b111, 0b010, 0b010, 0b010, 0b111]),
        'L' => Some([0b100, 0b100, 0b100, 0b100, 0b111]),
        'P' => Some([0b111, 0b101, 0b111, 0b100, 0b100]),
        'S' => Some([0b111, 0b100, 0b111, 0b001, 0b111]),
        'T' => Some([0b111, 0b010, 0b010, 0b010, 0b010]),
        'W' => Some([0b101, 0b101, 0b101, 0b111, 0b010]),
        'R' => Some([0b111, 0b101, 0b111, 0b110, 0b101]),
        'E' => Some([0b111, 0b100, 0b110, 0b100, 0b111]),
        'A' => Some([0b010, 0b101, 0b111, 0b101, 0b101]),
        'H' => Some([0b101, 0b101, 0b111, 0b101, 0b101]),
        'N' => Some([0b101, 0b111, 0b111, 0b101, 0b101]),
        'O' => Some([0b111, 0b101, 0b101, 0b101, 0b111]),
        'G' => Some([0b111, 0b100, 0b101, 0b101, 0b111]),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_depth_size(24)
        .with_transparency(true);

    let mut app = DemoApp::new(template);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
