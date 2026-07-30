// src/renderer/math.rs
use glam::camera::rh::{proj::opengl::perspective, view::look_to_mat4};
use glam::{Mat4, Quat, Vec3, Vec4};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A wrapper type around floats to allow hashing them via their raw bits.
#[derive(Copy, Clone, Debug)]
pub struct HashableF32(pub f32);

impl PartialEq for HashableF32 {
    fn eq(&self, o: &Self) -> bool {
        self.0.to_bits() == o.0.to_bits()
    }
}
impl Eq for HashableF32 {}
impl Hash for HashableF32 {
    fn hash<H: Hasher>(&self, s: &mut H) {
        self.0.to_bits().hash(s);
    }
}

/// A helper function to compute a u64 hash for a 3D float vector.
pub fn hash_vec3(v: Vec3) -> u64 {
    let mut h = DefaultHasher::new();
    v.x.to_bits().hash(&mut h);
    v.y.to_bits().hash(&mut h);
    v.z.to_bits().hash(&mut h);
    h.finish()
}

/// A helper function to compute a u64 hash for a quaternion.
pub fn hash_quat(q: Quat) -> u64 {
    let mut h = DefaultHasher::new();
    q.x.to_bits().hash(&mut h);
    q.y.to_bits().hash(&mut h);
    q.z.to_bits().hash(&mut h);
    q.w.to_bits().hash(&mut h);
    h.finish()
}

/// A lightweight helper for the Game Engine to build transforms.
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

pub fn transform_to_model_matrix(t: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}

/// Builds a right-handed view matrix from a camera position and orientation quaternion.
pub fn camera_to_view_matrix(position: Vec3, rotation: Quat) -> Mat4 {
    let forward = rotation * Vec3::NEG_Z;
    let up = rotation * Vec3::Y;
    look_to_mat4(position, forward, up)
}

/// Builds a right-handed OpenGL perspective projection matrix (depth → [-1, 1]).
///
/// Uses `glam::camera::rh::proj::opengl::perspective`.
/// Do NOT use `Mat4::perspective_rh` — that maps depth to [0, 1] (Vulkan NDC).
pub fn camera_to_projection_matrix(fov: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    perspective(fov, aspect, near, far)
}

// ── Frustum culling ──────────────────────────────────────────────────────────

/// The six planes of the view frustum extracted from a combined VP matrix,
/// using the Gribb / Hartmann method (2001).
///
/// Each `Vec4` stores (A, B, C, D) such that the plane equation is
///   A·x + B·y + C·z + D ≥ 0
/// for points that are on the *inside* of that plane.
///
/// The planes are not normalised here — normalise before doing distance tests
/// (see `sphere_inside_frustum`).
///
/// Plane order: [left, right, bottom, top, near, far].
pub fn extract_frustum_planes(vp: Mat4) -> [Vec4; 6] {
    let m = vp.transpose();
    let col = |i: usize| m.col(i);

    let row0 = col(0);
    let row1 = col(1);
    let row2 = col(2);
    let row3 = col(3);

    let planes = [
        row3 + row0, // left
        row3 - row0, // right
        row3 + row1, // bottom
        row3 - row1, // top
        row3 + row2, // near
        row3 - row2, // far
    ];

    // Normalise each plane so sphere_inside_frustum gets exact signed distances.
    planes.map(|p| {
        let len = Vec3::new(p.x, p.y, p.z).length();
        p / len
    })
}

/// Returns `true` if a sphere is fully or partially inside (or intersects)
/// all six frustum planes, i.e. it is not culled.
///
/// `planes` should come from `extract_frustum_planes`.
/// The test normalises each plane on-the-fly which is acceptable since we call
/// this O(N) times per frame and the normalisation cost is dominated by the
/// actual rendering.
#[inline]
pub fn sphere_inside_frustum(center: Vec3, radius: f32, planes: &[Vec4; 6]) -> bool {
    for plane in planes {
        // Planes are pre-normalised so this is an exact signed distance.
        let dist = plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w;
        if dist < -radius {
            return false;
        }
    }
    true
}
