use std::time::Instant;

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

use crate::{game, input, state::DemoState};

pub struct App {
    pub state: Option<DemoState>,
    pub pending_grab: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            pending_grab: true,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut state = DemoState::new(event_loop);
        game::init(&mut state); // initialise game
        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        match &event {
            WindowEvent::CloseRequested => {
                std::process::exit(0);
            }

            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
            }

            WindowEvent::RedrawRequested => {}

            _ => {}
        }

        input::window_event(state, &mut self.pending_grab, &event);
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(state) = self.state.as_mut() {
            input::device_event(state, &event);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        //
        // Cursor grab (deferred)
        //
        if self.pending_grab {
            use winit::window::CursorGrabMode;

            if state.window.set_cursor_grab(CursorGrabMode::Locked).is_ok() {
                state.window.set_cursor_visible(false);
                state.cursor_grabbed = true;
                self.pending_grab = false;
            }
        }

        //
        // TIME STEP
        //
        let now = Instant::now();
        let dt = (now - state.last_frame).as_secs_f32();
        state.last_frame = now;

        //
        // FPS COUNTER (smoothed)
        //
        state.frame_count += 1;
        let elapsed = now.duration_since(state.last_fps_update).as_secs_f32();

        if elapsed >= 0.5 {
            state.current_fps = state.frame_count as f32 / elapsed;
            state.frame_count = 0;
            state.last_fps_update = now;
        }

        //
        // GAME UPDATE & FRAME BUILD
        //
        game::update(state, dt);
        game::build_frame(state);

        // Upload lighting uniforms (must be done before render)
        use glam::Vec3;
        state.renderer.upload_shader_vec3(
            state.assets.lit_shader,
            "uSunDir",
            Vec3::new(0.6, 0.8, 0.4).normalize(),
        );
        state
            .renderer
            .upload_shader_f32(state.assets.lit_shader, "uAmbient", 0.18);

        //
        // RENDER
        //
        state.renderer.render();
        state.render();

        //
        // Request next frame
        //
        state.window.request_redraw();
    }
}
