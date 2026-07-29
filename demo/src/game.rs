// src/game.rs

use glam::{EulerRot, Quat, Vec3};

use rand::RngExt;
use rendering_engine::frame_data::RenderCommand;

use crate::{
    font::emit_ui_text,
    state::{Collectible, DemoState, GameState},
    touch::button_rects,
};

/// Initialise game state: spawn collectibles.
pub fn init(state: &mut DemoState) {
    state.game = GameState::new();
    spawn_collectibles(state);
}

/// Spawn `count` collectibles at random positions within `spawn_radius`.
fn spawn_collectibles(state: &mut DemoState) {
    let count = 15;
    let radius = state.game.spawn_radius;
    let mut rng = rand::rng(); // <-- use rng() instead of thread_rng()

    state.game.collectibles.clear();
    for _ in 0..count {
        let pos = loop {
            let p = Vec3::new(
                rng.random_range(-radius..radius),
                rng.random_range(-radius..radius),
                rng.random_range(-radius..radius),
            );
            // Avoid spawning right on top of the player's start position.
            if p.distance(Vec3::new(0.0, 1.0, 5.0)) > 2.0 {
                break p;
            }
        };
        state.game.collectibles.push(Collectible {
            position: pos,
            scale: 0.3,
        });
    }
}

/// Update game logic: move player, check collisions, respawn collectibles.
pub fn update(state: &mut DemoState, dt: f32) {
    // ── Camera movement (same as before) ──
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

    state.camera_pos += velocity * 5.0 * dt;

    // ── Collectible collisions ──
    let player_pos = state.camera_pos;
    let collect_dist = state.game.collect_distance;
    let mut rng = rand::rng();

    for collectible in &mut state.game.collectibles {
        let distance = player_pos.distance(collectible.position);
        if distance < collect_dist {
            // Collect it!
            state.game.score += 1;

            // Respawn at a new random position.
            let radius = state.game.spawn_radius;
            collectible.position = loop {
                let p = Vec3::new(
                    rng.random_range(-radius..radius),
                    rng.random_range(-radius..radius),
                    rng.random_range(-radius..radius),
                );
                if p.distance(player_pos) > 2.0 {
                    break p;
                }
            };
        }
    }

    // ── Animation (central cube) ──
    state.dyn_angle += dt * 1.2;
}

/// Build the frame: camera, world objects, UI.
pub fn build_frame(state: &mut DemoState) {
    let cam_rot = Quat::from_euler(EulerRot::YXZ, state.camera_yaw, state.camera_pitch, 0.0);

    let cube_rotation =
        Quat::from_rotation_y(state.dyn_angle) * Quat::from_rotation_x(state.dyn_angle * 0.7);

    let frame = state.write_handle.write_slot();

    frame.commands.clear();
    frame.ui_commands.clear();

    // ── World ──

    // Central spinning cube (the original demo object)
    frame.commands.push(RenderCommand {
        mesh_id: state.assets.cube_mesh,
        material_id: state.assets.lit_material,
        position: Vec3::ZERO,
        rotation: cube_rotation,
        scale: 1.0,
    });

    // Collectible cubes (golden)
    for collectible in &state.game.collectibles {
        frame.commands.push(RenderCommand {
            mesh_id: state.assets.collectible_mesh,
            material_id: state.assets.lit_material,
            position: collectible.position,
            rotation: Quat::IDENTITY,
            scale: collectible.scale,
        });
    }

    // ── Camera ──
    frame.camera_position = state.camera_pos;
    frame.camera_rotation = cam_rot;
    frame.camera_fov = std::f32::consts::FRAC_PI_3;
    frame.camera_aspect_ratio = state.width as f32 / state.height as f32;
    frame.camera_near = 0.1;
    frame.camera_far = 100.0;

    // ── UI ──
    let root_x = 20.0;
    let root_y = 30.0;

    // FPS counter
    emit_ui_text(
        &mut frame.ui_commands,
        state.assets.quad_mesh,
        state.assets.ui_material,
        root_x,
        root_y,
        &format!("{:.0} FPS", state.current_fps),
        8.0,
    );
    // Score
    emit_ui_text(
        &mut frame.ui_commands,
        state.assets.quad_mesh,
        state.assets.ui_material,
        root_x,
        root_y + 50.0,
        &format!("SCORE: {}", state.game.score),
        8.0,
    );

    // Android touch buttons
    if cfg!(target_os = "android") {
        let r = crate::touch::vsync_button_rect(state.width as f32, state.height as f32);
        frame.ui_commands.push(RenderCommand {
            mesh_id: state.assets.vsync_button_mesh,
            material_id: state.assets.ui_material,
            position: Vec3::new(r.x, r.y, 0.0),
            rotation: Quat::IDENTITY,
            scale: r.w,
        });
        for (_, rect) in button_rects(state.width as f32, state.height as f32) {
            frame.ui_commands.push(RenderCommand {
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
