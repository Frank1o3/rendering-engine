// src/renderer/frame_data.rs
use crate::engine::{MaterialId, MeshId};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct RenderCommand {
    pub model_matrix: Mat4,      // 64 bytes
    pub mesh_id: MeshId,         // 4 bytes
    pub material_id: MaterialId, // 4 bytes
    pub _padding: [u32; 2],      // 8 bytes explicit padding
}

pub struct FrameData {
    pub commands: Vec<RenderCommand>,
    pub ui_commands: Vec<RenderCommand>, // For 2D overlay
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
            commands: Vec::with_capacity(1024), // Pre-allocated!
            ui_commands: Vec::with_capacity(256), // Pre-allocate for UI
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
