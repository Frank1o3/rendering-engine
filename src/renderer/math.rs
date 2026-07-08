use crate::renderer::engine::{Camera, Transform};
use glam::Mat4;
use glam::camera::rh::proj::opengl::perspective;

pub fn transform_to_model_matrix(transform: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(transform.scale, transform.rotation, transform.position)
}

pub fn camera_to_view_matrix(camera: &Camera) -> Mat4 {
    // View matrix is the inverse of the camera's world transform
    let translation = Mat4::from_translation(-camera.position);
    let rotation = Mat4::from_quat(camera.rotation.conjugate());
    rotation * translation
}

pub fn camera_to_projection_matrix(camera: &Camera, aspect_ratio: f32) -> Mat4 {
    perspective(camera.fov, aspect_ratio, camera.near, camera.far)
}
