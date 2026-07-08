// src/renderer/math.rs
use glam::camera::rh::proj::opengl::perspective;
use glam::{Mat4, Quat, Vec3};

/// A lightweight helper for the Game Engine to build transforms.
/// The Renderer knows nothing about this struct!
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

pub fn transform_to_model_matrix(transform: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(transform.scale, transform.rotation, transform.position)
}

// The renderer calls these directly using data from FrameData
pub fn camera_to_view_matrix(position: Vec3, rotation: Quat) -> Mat4 {
    let translation = Mat4::from_translation(-position);
    let rotation = Mat4::from_quat(rotation.conjugate());
    rotation * translation
}

pub fn camera_to_projection_matrix(fov: f32, aspect_ratio: f32, near: f32, far: f32) -> Mat4 {
    perspective(fov, aspect_ratio, near, far)
}
