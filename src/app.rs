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
use std::{ffi::CString, num::NonZeroU32, sync::Arc};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    window::{Window, WindowId},
};

// 1. Import your renderer types
use glam::{Quat, Vec3};

use crate::renderer::{
    engine::{MaterialId, MeshId, Renderer},
    frame_data::{FrameData, RenderCommand},
    math::{Transform, transform_to_model_matrix},
    mesh::{MeshData, Vertex},
    // --- NEW PHASE 1 IMPORTS ---
    triple_buffer::{WriteHandle, new_triple_buffer},
};

struct GlState {
    window: Arc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,

    // The renderer now OWNS the GL context, so we don't need `gl: Context` here anymore!
    renderer: Renderer,

    // The game engine keeps the write handle to push frame data to the lock-free buffer
    write_handle: WriteHandle<FrameData>,

    // Store the IDs so we don't recreate the mesh/shader every single frame
    mesh_id: MeshId,
    material_id: MaterialId,

    // Track window size to calculate the correct aspect ratio
    width: u32,
    height: u32,
}

pub struct App {
    pub template: ConfigTemplateBuilder,
    state: Option<GlState>,
}

impl App {
    pub fn new(template: ConfigTemplateBuilder) -> Self {
        Self {
            state: None,
            template,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("OpenGL Rectangle")
            .with_transparent(true)
            .with_inner_size(PhysicalSize::new(800, 600));

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

        // ==========================================
        // 2. INITIALIZE RENDERER & LOAD ASSETS
        // ==========================================

        // Create the lock-free Triple Buffer
        let (write_handle, read_handle) = new_triple_buffer::<FrameData>();

        // The renderer takes ownership of the GL context AND the read_handle
        let mut renderer = Renderer::new(gl, read_handle);

        // Create a rectangle mesh (composed of 2 triangles)
        let vertices = vec![
            Vertex {
                position: [-0.5, -0.5, 0.0],
                color: [255, 0, 0, 255],
            }, // Bottom-left (Red)
            Vertex {
                position: [0.5, -0.5, 0.0],
                color: [0, 255, 0, 255],
            }, // Bottom-right (Green)
            Vertex {
                position: [0.5, 0.5, 0.0],
                color: [0, 0, 255, 255],
            }, // Top-right (Blue)
            Vertex {
                position: [-0.5, 0.5, 0.0],
                color: [255, 255, 0, 255],
            }, // Top-left (Yellow)
        ];
        let indices = vec![
            0, 1, 2, // First triangle
            2, 3, 0, // Second triangle
        ];
        let mesh_id = renderer.load_mesh(MeshData { vertices, indices });

        // Load all shaders from the `shaders/` directory at the project root
        let shader_map = renderer
            .load_shaders_from_dir(std::path::Path::new("shaders"))
            .expect("Failed to load shaders directory");

        // Grab the 'basic' shader we just loaded by its filename
        let shader_id = *shader_map
            .get("basic")
            .expect("Missing 'basic.vert' and 'basic.frag' in shaders/ directory");

        let material_id = renderer.create_material(shader_id);

        // Save state for the render loop
        self.state = Some(GlState {
            window,
            gl_context,
            gl_surface,
            renderer,
            write_handle, // <--- Store the write handle in the game state
            mesh_id,
            material_id,
            width: 800,
            height: 600,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                info!("Close requested, exiting.");
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

                    // Tell the renderer to update the OpenGL viewport
                    state.renderer.resize(size.width as i32, size.height as i32);
                }
            }

            WindowEvent::RedrawRequested => {
                // ==========================================
                // 3. BUILD FRAME DATA (GAME ENGINE LOGIC)
                // ==========================================

                // 1. Get the write slot and clear it (keeps the pre-allocated capacity!)
                let frame = state.write_handle.write_slot();
                frame.commands.clear();

                // 2. Game engine pre-computes the model matrix (Dumb Renderer rule!)
                let transform = Transform {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                };
                let model_matrix = transform_to_model_matrix(&transform);

                // 3. Push the command
                frame.commands.push(RenderCommand {
                    mesh_id: state.mesh_id,
                    material_id: state.material_id,
                    model_matrix,
                    _padding: [0; 2], // Explicit padding to satisfy bytemuck alignment
                });

                // 4. Set camera data
                frame.camera_position = Vec3::new(0.0, 0.0, 2.0); // Pull camera back slightly
                frame.camera_rotation = Quat::IDENTITY;
                frame.camera_fov = std::f32::consts::FRAC_PI_4; // 45 degrees
                frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
                frame.camera_near = 0.1;
                frame.camera_far = 100.0;

                // 5. Publish to the renderer via the lock-free buffer
                state.write_handle.publish();

                // ==========================================
                // 4. RENDER!
                // ==========================================

                // The renderer internally consumes the data from its ReadHandle.
                // Notice it takes ZERO arguments!
                state.renderer.render();

                // 5. Swap buffers
                state.gl_surface.swap_buffers(&state.gl_context).unwrap();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}
