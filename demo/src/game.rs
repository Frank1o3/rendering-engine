use glam::{EulerRot, Quat, Vec3};
use rendering_engine::frame_data::RenderCommand as EngineRenderCommand;

use crate::{font::emit_ui_text, state::DemoState};

const FLY_SPEED: f32 = 12.0;

pub fn init(_state: &mut DemoState) {
    // Nothing to seed — World::update on the first frame does the rest.
}

/// Fly-camera movement — no gravity, no collision. Exploration only.
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
        velocity = velocity.normalize();
    }
    state.camera_pos += velocity * FLY_SPEED * dt;

    state.world.update(state.camera_pos);
}

/// Camera + a minimal HUD. Terrain never passes through here — it lives on
/// the render thread via AddChunk/RemoveChunk.
pub fn build_frame(state: &mut DemoState) {
    let cam_rot = Quat::from_euler(EulerRot::YXZ, state.camera_yaw, state.camera_pitch, 0.0);

    let frame = state.write_handle.write_slot();
    frame.commands.clear();
    frame.ui_commands.clear();

    frame.camera_position = state.camera_pos;
    frame.camera_rotation = cam_rot;
    frame.camera_fov = std::f32::consts::FRAC_PI_3;
    frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
    frame.camera_near = 0.1;
    frame.camera_far = 500.0; // terrain draws much further than the old single-cube demo

    emit_ui_text(
        &mut frame.ui_commands,
        state.assets.quad_mesh,
        state.assets.ui_material,
        20.0,
        30.0,
        &format!("{:.0} FPS", state.current_fps),
        8.0,
    );

    if cfg!(target_os = "android") {
        let r = crate::touch::vsync_button_rect(state.width as f32, state.height as f32);
        frame.ui_commands.push(EngineRenderCommand {
            mesh_id: state.assets.vsync_button_mesh,
            material_id: state.assets.ui_material,
            position: Vec3::new(r.x, r.y, 0.0),
            rotation: Quat::IDENTITY,
            scale: r.w,
        });
        for (_, rect) in crate::touch::button_rects(state.width as f32, state.height as f32) {
            frame.ui_commands.push(EngineRenderCommand {
                mesh_id: state.assets.button_quad_mesh,
                material_id: state.assets.ui_material,
                position: Vec3::new(rect.x, rect.y, 0.0),
                rotation: Quat::IDENTITY,
                scale: rect.w,
            });
        }
    }

    state.write_handle.publish();
}
