use glow::{Context, HasContext};
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

struct GlState {
    window: Arc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gl: Context,
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
            .with_title("Test Window")
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

        info!("OpenGL context successfully initialized using Glow!");

        self.state = Some(GlState {
            window,
            gl_context,
            gl_surface,
            gl,
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
                    state.gl_surface.resize(
                        &state.gl_context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );
                    unsafe {
                        state
                            .gl
                            .viewport(0, 0, size.width as i32, size.height as i32);
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                unsafe {
                    // Clear screen using pure glow commands
                    state.gl.clear_color(0.1, 0.2, 0.3, 1.0);
                    state.gl.clear(glow::COLOR_BUFFER_BIT);
                }

                // Display your work by swapping front and back buffers
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
