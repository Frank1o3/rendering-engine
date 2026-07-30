// src/input.rs

use std::sync::atomic::Ordering;

use winit::{
    event::{DeviceEvent, ElementState, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::CursorGrabMode,
};

use crate::{
    state::DemoState,
    touch::{TouchKind, button_rects, set_button_key},
};

pub fn window_event(state: &mut DemoState, pending_grab: &mut bool, event: &WindowEvent) {
    match event {
        WindowEvent::KeyboardInput { event, .. } => {
            let pressed = event.state == ElementState::Pressed;

            if let PhysicalKey::Code(code) = event.physical_key {
                match code {
                    KeyCode::KeyW => state.keys.w = pressed,
                    KeyCode::KeyA => state.keys.a = pressed,
                    KeyCode::KeyS => state.keys.s = pressed,
                    KeyCode::KeyD => state.keys.d = pressed,

                    KeyCode::Space => state.keys.space = pressed,
                    KeyCode::ControlLeft => state.keys.lctrl = pressed,
                    KeyCode::KeyE if pressed => {
                        state.vsync_enabled.fetch_xor(true, Ordering::Relaxed);
                    }

                    KeyCode::Escape if pressed => {
                        if state.cursor_grabbed {
                            let _ = state.window.set_cursor_grab(CursorGrabMode::None);

                            state.window.set_cursor_visible(true);
                            state.cursor_grabbed = false;
                        } else {
                            *pending_grab = true;
                        }
                    }

                    _ => {}
                }
            }
        }

        WindowEvent::Touch(touch) => {
            let x = touch.location.x as f32;
            let y = touch.location.y as f32;

            match touch.phase {
                winit::event::TouchPhase::Started => {
                    let hit = button_rects(state.width as f32, state.height as f32)
                        .into_iter()
                        .find(|(_, rect)| rect.contains(x, y));

                    if let Some((button, _)) = hit {
                        set_button_key(state, button, true);

                        state.touches.insert(touch.id, TouchKind::Button(button));
                    } else {
                        state
                            .touches
                            .insert(touch.id, TouchKind::Look { last: (x, y) });
                    }
                }

                winit::event::TouchPhase::Moved => {
                    let x = touch.location.x as f32;
                    let y = touch.location.y as f32;

                    if touch.phase == winit::event::TouchPhase::Started
                        && crate::touch::vsync_button_rect(state.width as f32, state.height as f32)
                            .contains(x, y)
                    {
                        state.vsync_enabled.fetch_xor(true, Ordering::Relaxed);
                        state.touches.insert(touch.id, TouchKind::VsyncToggle);
                        return; // consumed — don't fall through to movement-button hit test
                    }

                    if let Some(TouchKind::Look { last }) = state.touches.get_mut(&touch.id) {
                        let dx = x - last.0;
                        let dy = y - last.1;

                        state.camera_yaw -= dx * state.config.touch_look_sensitivity;

                        state.camera_pitch -= dy * state.config.touch_look_sensitivity;

                        state.camera_pitch = state.camera_pitch.clamp(-1.5, 1.5);

                        *last = (x, y);
                    }
                }

                winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                    if let Some(TouchKind::Button(button)) = state.touches.remove(&touch.id) {
                        set_button_key(state, button, false);
                    }
                }
            }
        }

        _ => {}
    }
}

pub fn device_event(state: &mut DemoState, event: &DeviceEvent) {
    if let DeviceEvent::MouseMotion { delta } = event {
        if state.cursor_grabbed {
            state.camera_yaw -= delta.0 as f32 * state.config.mouse_sensitivity;
            state.camera_pitch -= delta.1 as f32 * state.config.mouse_sensitivity;

            state.camera_pitch = state.camera_pitch.clamp(-1.5, 1.5);
        }
    }
}
