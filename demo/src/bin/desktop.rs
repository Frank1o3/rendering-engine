use winit::event_loop::EventLoop;

use demo::app::App;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();

    event_loop.run_app(&mut app).unwrap();
}
