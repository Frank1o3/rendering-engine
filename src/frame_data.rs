// src/renderer/frame_data.rs
use crate::engine::{MaterialId, MeshId};
use bytemuck::{Pod, Zeroable};
use glam::{Quat, Vec3};

/// Compact per-instance GPU data: 32 bytes instead of 64-byte Mat4.
/// The shader reconstructs the transform using quaternion rotation.
///
/// Memory layout:
///   position : [f32; 3] = 12 bytes (offset  0)
///   scale    : f32      =  4 bytes (offset 12)
///   rotation : [f32; 4] = 16 bytes (offset 16)  — quaternion (x, y, z, w)
///   ─────────────────────────────────────────────
///   Total                 32 bytes
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub position: [f32; 3], // 12 bytes
    pub scale: f32,         // 4 bytes (uniform scale)
    pub rotation: [f32; 4], // 16 bytes (quaternion xyzw)
}

impl InstanceData {
    /// Construct from glam types.
    pub fn new(position: Vec3, rotation: Quat, scale: f32) -> Self {
        Self {
            position: position.to_array(),
            scale,
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
        }
    }

    /// Identity instance at the origin.
    pub const IDENTITY: Self = Self {
        position: [0.0, 0.0, 0.0],
        scale: 1.0,
        rotation: [0.0, 0.0, 0.0, 1.0], // Quat::IDENTITY = (0, 0, 0, 1)
    };
}

impl Default for InstanceData {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A render command submitted by the game engine each frame.
/// Used for **dynamic** objects (particles, projectiles, UI) that change every frame.
/// Static objects are registered via the Scene and don't use this struct.
#[derive(Clone, Copy, Debug)]
pub struct RenderCommand {
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: f32,
}

impl RenderCommand {
    /// Convert to GPU-ready InstanceData.
    pub fn to_instance_data(&self) -> InstanceData {
        InstanceData::new(self.position, self.rotation, self.scale)
    }
}

/// Per-frame data passed from the game engine to the renderer via the triple buffer.
/// Static scene objects are managed separately via the Scene registry.
pub struct FrameData {
    /// Dynamic 3D commands (particles, projectiles, etc.)
    pub commands: Vec<RenderCommand>,
    /// 2D UI overlay commands (rendered with dedicated UI shader)
    pub ui_commands: Vec<RenderCommand>,

    // Camera Transform
    pub camera_position: Vec3,
    pub camera_rotation: Quat,

    // Camera Projection (Needed by the renderer to build the VP matrix)
    pub camera_fov: f32,
    pub camera_aspect_ratio: f32,
    pub camera_near: f32,
    pub camera_far: f32,
}

impl Default for FrameData {
    fn default() -> Self {
        Self {
            commands: Vec::with_capacity(1024),    // Pre-allocated!
            ui_commands: Vec::with_capacity(256),   // Pre-allocate for UI
            camera_position: Vec3::ZERO,
            camera_rotation: Quat::IDENTITY,
            camera_fov: std::f32::consts::FRAC_PI_4,
            camera_aspect_ratio: 16.0 / 9.0,
            camera_near: 0.1,
            camera_far: 100.0,
        }
    }
}

impl Clone for FrameData {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            ui_commands: self.ui_commands.clone(),
            camera_position: self.camera_position,
            camera_rotation: self.camera_rotation,
            camera_fov: self.camera_fov,
            camera_aspect_ratio: self.camera_aspect_ratio,
            camera_near: self.camera_near,
            camera_far: self.camera_far,
        }
    }
}
