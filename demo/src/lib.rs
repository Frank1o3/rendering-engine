// demo/src/lib.rs
pub mod app;
mod font;
mod game;
mod input;
mod meshes;
mod render_thread;
mod renderer_setup;
mod shaders;
mod state;
mod touch;

#[cfg(target_os = "android")]
mod android {
    use winit::event_loop::EventLoop;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    use crate::app::App;

    #[unsafe(no_mangle)]
    pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
        let event_loop = EventLoop::builder().with_android_app(app).build().unwrap();

        let mut app = App::new();
        event_loop.run_app(&mut app).unwrap();
    }
}
