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
    engine::{Camera, MaterialId, MeshId, RenderInput, RenderObject, Renderer, Transform},
    mesh::{MeshData, Vertex},
};

struct GlState {
    window: Arc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,

    // The renderer now OWNS the GL context, so we don't need `gl: Context` here anymore!
    renderer: Renderer,

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

        // The renderer takes ownership of the GL context
        let mut renderer = Renderer::new(gl);

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

        // Define GLSL Shaders
        let vs_src = r#"
            #version 460 core
            layout (location = 0) in vec3 aPos;
            layout (location = 1) in vec4 aColor;
            out vec4 ourColor;
            uniform mat4 uMVP;
            void main() {
                gl_Position = uMVP * vec4(aPos, 1.0);
                ourColor = aColor;
            }
        "#;

        let fs_src = r#"
            #version 460 core
            in vec4 ourColor;
            out vec4 FragColor;
            void main() {
                FragColor = ourColor;
            }
        "#;

        let shader_id = renderer.load_shader(vs_src, fs_src);
        let material_id = renderer.create_material(shader_id);

        // Save state for the render loop
        self.state = Some(GlState {
            window,
            gl_context,
            gl_surface,
            renderer,
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
                // 1. Build the Camera
                let camera = Camera {
                    position: Vec3::new(0.0, 0.0, 2.0), // Pull camera back slightly
                    rotation: Quat::IDENTITY,
                    fov: std::f32::consts::FRAC_PI_4, // 45 degrees
                    aspect_ratio: state.width as f32 / state.height as f32,
                    near: 0.1,
                    far: 100.0,
                };

                // 2. Build the Object Transform
                let transform = Transform {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                };

                // 3. Package into RenderInput
                let render_input = RenderInput {
                    camera,
                    objects: vec![RenderObject {
                        transform,
                        mesh_id: state.mesh_id,
                        material_id: state.material_id,
                    }],
                };

                // 4. RENDER!
                // The renderer handles clearing the screen, computing matrices, and drawing.
                state.renderer.render(&render_input);

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
