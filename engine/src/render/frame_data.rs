use crate::render::renderer::MeshId;
use crate::resources::material::MaterialId;
use bytemuck::{Pod, Zeroable};
use glam::{Quat, Vec3};

/// Compact per-instance GPU data: 32 bytes instead of 64-byte Mat4.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub position: [f32; 3], // 12 bytes
    pub scale: f32,         // 4 bytes
    pub rotation: [f32; 4], // 16 bytes (quaternion xyzw)
}

impl InstanceData {
    pub fn new(position: Vec3, rotation: Quat, scale: f32) -> Self {
        Self {
            position: position.to_array(),
            scale,
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
        }
    }

    pub const IDENTITY: Self = Self {
        position: [0.0, 0.0, 0.0],
        scale: 1.0,
        rotation: [0.0, 0.0, 0.0, 1.0],
    };
}

impl Default for InstanceData {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Dynamic per-frame render command.
#[derive(Clone, Copy, Debug)]
pub struct RenderCommand {
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: f32,
}

impl RenderCommand {
    pub fn to_instance_data(&self) -> InstanceData {
        InstanceData::new(self.position, self.rotation, self.scale)
    }
}

/// Per-frame payload passed via triple buffer.
pub struct FrameData {
    pub commands: Vec<RenderCommand>,
    pub ui_commands: Vec<RenderCommand>,

    pub camera_position: Vec3,
    pub camera_rotation: Quat,

    pub camera_fov: f32,
    pub camera_aspect_ratio: f32,
    pub camera_near: f32,
    pub camera_far: f32,
}

impl Default for FrameData {
    fn default() -> Self {
        Self {
            commands: Vec::with_capacity(1024),
            ui_commands: Vec::with_capacity(256),
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
