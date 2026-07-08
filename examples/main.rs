// examples/main.rs

// Import the app module (which is sitting right next to this file in examples/)
mod app;

use crate::app::App;
use anyhow::Result;
use glutin::config::ConfigTemplateBuilder;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let env_loop = EventLoop::new()?;
    env_loop.set_control_flow(ControlFlow::Poll);

    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true);

    let mut app = App::new(template);

    // Run the winit event loop
    env_loop.run_app(&mut app)?;

    Ok(())
}
