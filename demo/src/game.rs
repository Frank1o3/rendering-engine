use glam::{EulerRot, Quat, Vec3};

use crate::{font::emit_ui_text, state::DemoState};

pub fn init(_state: &mut DemoState) {}

pub fn update(state: &mut DemoState, dt: f32) {
    let cam_rot = Quat::from_euler(EulerRot::YXZ, state.camera_yaw, state.camera_pitch, 0.0);
    let forward = cam_rot * Vec3::NEG_Z;
    let right = cam_rot * Vec3::X;

    let mut velocity = Vec3::ZERO;
    if state.keys.w {
        velocity += forward;
    }
    if state.keys.s {
        velocity -= forward;
    }
    if state.keys.a {
        velocity -= right;
    }
    if state.keys.d {
        velocity += right;
    }
    if state.keys.space {
        velocity += Vec3::Y;
    }
    if state.keys.lctrl {
        velocity -= Vec3::Y;
    }

    if velocity.length_squared() > 0.0 {
        velocity = velocity.normalize() * state.config.fly_speed;
    }
    state.camera_pos += velocity * dt;

    state.world.update(state.camera_pos);
}

pub fn build_frame(state: &mut DemoState) {
    let cam_rot = Quat::from_euler(EulerRot::YXZ, state.camera_yaw, state.camera_pitch, 0.0);

    let frame = state.write_handle.write_slot();
    frame.commands.clear();
    frame.ui.clear();
    frame.ui_commands.clear();

    frame.camera_position = state.camera_pos;
    frame.camera_rotation = cam_rot;
    frame.camera_fov = state.config.fov_degrees.to_radians();
    frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
    frame.camera_near = state.config.near_plane;
    frame.camera_far = state.config.far_plane;

    emit_ui_text(
        &mut frame.ui,
        20.0,
        30.0,
        &format!("{:.0} FPS", state.current_fps),
        8.0,
        [255, 255, 255, 255],
    );

    if cfg!(target_os = "android") {
        let r = crate::touch::vsync_button_rect(state.width as f32, state.height as f32);
        frame.ui.rect(r.x, r.y, r.w, r.h, [220, 35, 35, 220]);
        let r = crate::touch::wireframe_button_rect(state.width as f32, state.height as f32);
        frame.ui.rect(r.x, r.y, r.w, r.h, [35, 35, 220, 220]);
        for (_, rect) in crate::touch::button_rects(state.width as f32, state.height as f32) {
            frame
                .ui
                .rect(rect.x, rect.y, rect.w, rect.h, [70, 70, 80, 140]);
        }
    }

    state.write_handle.publish();
}
