// src/renderer/math.rs
use glam::camera::rh::proj::opengl::perspective;
use glam::{Mat4, Quat, Vec3};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A wrapper type around floats to allow hashing them via their raw bits.
#[derive(Copy, Clone, Debug)]
pub struct HashableF32(pub f32);

impl PartialEq for HashableF32 {
    fn eq(&self, other: &Self) -> bool {
        // Fallback to bit representation to avoid NaN != NaN issues when hashing
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for HashableF32 {}

impl Hash for HashableF32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

/// Helper function to compute a u64 hash for a 3D float vector.
pub fn hash_vec3(v: Vec3) -> u64 {
    let mut hasher = DefaultHasher::new();
    v.x.to_bits().hash(&mut hasher);
    v.y.to_bits().hash(&mut hasher);
    v.z.to_bits().hash(&mut hasher);
    hasher.finish()
}

/// Helper function to compute a u64 hash for a quaternion.
pub fn hash_quat(q: Quat) -> u64 {
    let mut hasher = DefaultHasher::new();
    q.x.to_bits().hash(&mut hasher);
    q.y.to_bits().hash(&mut hasher);
    q.z.to_bits().hash(&mut hasher);
    q.w.to_bits().hash(&mut hasher);
    hasher.finish()
}

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
