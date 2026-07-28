use glow::Context;
use glutin::{
    GlSurface,
    config::ConfigTemplateBuilder,
    context::{NotCurrentContext, PossiblyCurrentContext},
    display::GlDisplay,
    surface::{Surface, WindowSurface},
};
use raw_window_handle::HasWindowHandle;
use rendering_engine::{engine::Renderer, frame_data::FrameData, triple_buffer::ReadHandle};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

pub enum RenderCommand {
    Render,
    Resize(u32, u32),
    Shutdown,
}

pub fn start_render_thread(
    not_current: NotCurrentContext,
    surface: Surface<WindowSurface>,
    read_handle: ReadHandle<FrameData>,
    window_handle: impl HasWindowHandle + Send + 'static,
) -> (Sender<RenderCommand>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        // Make the context current on this thread
        let gl_context = not_current.make_current(&surface).unwrap();
        // Get the GL function loader
        let gl_display = surface.display();
        let gl = unsafe {
            Context::from_loader_function(|sym| {
                let c = std::ffi::CString::new(sym).unwrap();
                gl_display.get_proc_address(&c)
            })
        };
        let mut renderer = Renderer::new(gl, read_handle);

        let mut running = true;
        while running {
            // Check for messages from the main thread
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    RenderCommand::Render => {
                        // renderer.render() already uses the latest frame from read_handle
                        renderer.render();
                        surface.swap_buffers(&gl_context).unwrap();
                    }
                    RenderCommand::Resize(w, h) => {
                        // Resize surface and renderer viewport
                        use std::num::NonZeroU32;
                        surface.resize(
                            &gl_context,
                            NonZeroU32::new(w).unwrap(),
                            NonZeroU32::new(h).unwrap(),
                        );
                        renderer.resize(w as i32, h as i32);
                    }
                    RenderCommand::Shutdown => {
                        running = false;
                    }
                }
            }
            // If no messages, we could idle or wait for a new frame.
            // A simple approach: loop with a small sleep.
            // Better: use a condition variable to wake when a new frame is published.
            thread::sleep(std::time::Duration::from_millis(1));
        }
        // Cleanup: context will be dropped here
    });
    (tx, handle)
}
