// demo/src/lib.rs
pub mod app;
pub mod config;
mod font;
mod game;
mod input;
mod meshes;
mod render_thread;
mod renderer_setup;
mod shaders;
mod state;
mod touch;
mod voxel;

// =====================================================================
// Android Configuration Platform Hook
// =====================================================================
#[cfg(target_os = "android")]
pub mod android {
    use crate::app::App;
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use winit::event_loop::EventLoop;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    pub static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

    #[unsafe(no_mangle)]
    pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
        if let Some(dir) = app.internal_data_path() {
            let _ = DATA_DIR.set(dir);
        }

        let event_loop = EventLoop::builder().with_android_app(app).build().unwrap();
        let mut app = App::new();
        event_loop.run_app(&mut app).unwrap();
    }
}

// =====================================================================
// iOS Configuration Platform Hook (Added for Winit 0.30+)
// =====================================================================
#[cfg(target_os = "ios")]
pub mod ios {
    use crate::app::App;
    use winit::event_loop::EventLoop;

    // Registers the runner block with UIKit/UIApplication life-cycle loops.
    // Winit translates this into a standard C entry symbol context.
    winit::main_app!(ios_main);

    fn ios_main() {
        let event_loop = EventLoop::new().unwrap();
        let mut app = App::new();
        event_loop.run_app(&mut app).unwrap();
    }
}
