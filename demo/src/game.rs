// demo/src/game.rs
use glam::{EulerRot, Quat, Vec3};
use rendering_engine::{
    frame_data::RenderCommand as EngineRenderCommand,
    math::{camera_to_projection_matrix, camera_to_view_matrix, extract_frustum_planes},
};

use crate::{font::emit_ui_text, state::DemoState};

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
        velocity = velocity.normalize() * state.config.fly_speed;
    }
    state.camera_pos += velocity * dt;

    // Compute frustum planes using the engine’s helpers.
    let view = camera_to_view_matrix(state.camera_pos, cam_rot);
    let proj = camera_to_projection_matrix(
        state.config.fov_degrees.to_radians(),
        state.width as f32 / state.height as f32,
        state.config.near_plane,
        state.config.far_plane,
    );
    let vp = proj * view;
    let frustum = extract_frustum_planes(vp);

    state.world.update(state.camera_pos, &frustum);
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
    frame.camera_fov = state.config.fov_degrees.to_radians();
    frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
    frame.camera_near = state.config.near_plane;
    frame.camera_far = state.config.far_plane;

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
