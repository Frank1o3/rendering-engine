/// Lighting system for the renderer.
///
/// Supports directional, point, and spot lights with support for
/// shadow mapping in future extensions.
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::f32::consts::PI;

/// Maximum number of lights that can be active at once.
/// This is limited by the uniform buffer size and performance considerations.
pub const MAX_LIGHTS: usize = 16;

/// Unique identifier for a light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LightId(pub u32);

/// Type of light source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    /// Directional light (like sunlight) - infinite distance, parallel rays.
    Directional = 0,
    /// Point light - emits light in all directions from a point.
    Point = 1,
    /// Spot light - emits light in a cone from a point.
    Spot = 2,
}

/// Light data as it appears in the uniform buffer.
/// Packed to 64 bytes (16 floats) for efficient GPU transfer.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct LightData {
    /// Light position (for point/spot), or negative direction (for directional).
    /// For directional lights, the direction is normalized.
    pub position_or_dir: Vec3,
    /// Light type (0=Directional, 1=Point, 2=Spot).
    pub light_type: u32,

    /// Light color and intensity (intensity in w).
    pub color: Vec3,
    /// Spot angle (cos of half-angle), or range for point lights.
    pub range_or_angle: f32,

    /// Spot direction (normalized), unused for other types.
    pub spot_direction: Vec3,
    /// Reserved for future use.
    pub _reserved1: u32,

    /// Ambient influence (0.0 to 1.0).
    pub ambient_influence: f32,
    /// Whether this light casts shadows.
    pub casts_shadow: u32,
    /// Specular intensity.
    pub specular_intensity: f32,
    /// Reserved for future use.
    pub _reserved2: u32,
}

impl LightData {
    /// Create light data for a directional light.
    pub fn directional(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            position_or_dir: -direction.normalize(),
            light_type: LightType::Directional as u32,
            color,
            range_or_angle: intensity,
            spot_direction: Vec3::ZERO,
            _reserved1: 0,
            ambient_influence: 0.0,
            casts_shadow: 0,
            specular_intensity: 1.0,
            _reserved2: 0,
        }
    }

    /// Create light data for a point light.
    pub fn point(position: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            position_or_dir: position,
            light_type: LightType::Point as u32,
            color: color * intensity,
            range_or_angle: range,
            spot_direction: Vec3::ZERO,
            _reserved1: 0,
            ambient_influence: 0.0,
            casts_shadow: 0,
            specular_intensity: 1.0,
            _reserved2: 0,
        }
    }

    /// Create light data for a spot light.
    pub fn spot(
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        _range: f32,
        angle: f32,
    ) -> Self {
        let half_angle = angle * 0.5;
        Self {
            position_or_dir: position,
            light_type: LightType::Spot as u32,
            color: color * intensity,
            range_or_angle: half_angle.cos(),
            spot_direction: direction.normalize(),
            _reserved1: 0,
            ambient_influence: 0.0,
            casts_shadow: 0,
            specular_intensity: 1.0,
            _reserved2: 0,
        }
    }
}

/// A light in the scene.
pub struct Light {
    id: LightId,
    position: Vec3,
    direction: Vec3,
    color: Vec3,
    intensity: f32,
    light_type: LightType,
    range: f32,
    angle: f32, // For spot lights, the total angle
    enabled: bool,
}

impl Light {
    /// Create a directional light.
    pub fn directional(id: LightId, direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            id,
            position: Vec3::ZERO,
            direction: direction.normalize(),
            color,
            intensity,
            light_type: LightType::Directional,
            range: f32::INFINITY,
            angle: 2.0 * PI,
            enabled: true,
        }
    }

    /// Create a point light.
    pub fn point(id: LightId, position: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            id,
            position,
            direction: Vec3::Z,
            color,
            intensity,
            light_type: LightType::Point,
            range,
            angle: 2.0 * PI,
            enabled: true,
        }
    }

    /// Create a spot light.
    pub fn spot(
        id: LightId,
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        angle: f32,
    ) -> Self {
        Self {
            id,
            position,
            direction: direction.normalize(),
            color,
            intensity,
            light_type: LightType::Spot,
            range,
            angle,
            enabled: true,
        }
    }

    /// Get this light as GPU-ready data.
    pub fn to_gpu_data(&self) -> LightData {
        match self.light_type {
            LightType::Directional => {
                LightData::directional(self.direction, self.color, self.intensity)
            }
            LightType::Point => {
                LightData::point(self.position, self.color, self.intensity, self.range)
            }
            LightType::Spot => LightData::spot(
                self.position,
                self.direction,
                self.color,
                self.intensity,
                self.range,
                self.angle,
            ),
        }
    }

    pub fn id(&self) -> LightId {
        self.id
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, pos: Vec3) {
        self.position = pos;
    }

    pub fn direction(&self) -> Vec3 {
        self.direction
    }

    pub fn set_direction(&mut self, dir: Vec3) {
        self.direction = dir.normalize();
    }

    pub fn color(&self) -> Vec3 {
        self.color
    }

    pub fn set_color(&mut self, color: Vec3) {
        self.color = color;
    }

    pub fn intensity(&self) -> f32 {
        self.intensity
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn light_type(&self) -> LightType {
        self.light_type
    }
}

/// Manages all lights in the scene.
pub struct LightManager {
    lights: std::collections::HashMap<LightId, Light>,
    next_id: u32,
    active_lights: Vec<LightId>,
}

impl LightManager {
    pub fn new() -> Self {
        Self {
            lights: std::collections::HashMap::new(),
            next_id: 0,
            active_lights: Vec::new(),
        }
    }

    /// Create a new directional light and add it to the scene.
    pub fn create_directional(&mut self, direction: Vec3, color: Vec3, intensity: f32) -> LightId {
        let id = LightId(self.next_id);
        self.next_id += 1;
        let light = Light::directional(id, direction, color, intensity);
        self.active_lights.push(id);
        self.lights.insert(id, light);
        id
    }

    /// Create a new point light and add it to the scene.
    pub fn create_point(
        &mut self,
        position: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
    ) -> LightId {
        let id = LightId(self.next_id);
        self.next_id += 1;
        let light = Light::point(id, position, color, intensity, range);
        self.active_lights.push(id);
        self.lights.insert(id, light);
        id
    }

    /// Create a new spot light and add it to the scene.
    pub fn create_spot(
        &mut self,
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        angle: f32,
    ) -> LightId {
        let id = LightId(self.next_id);
        self.next_id += 1;
        let light = Light::spot(id, position, direction, color, intensity, range, angle);
        self.active_lights.push(id);
        self.lights.insert(id, light);
        id
    }

    /// Remove a light from the scene.
    pub fn remove(&mut self, id: LightId) {
        self.lights.remove(&id);
        self.active_lights.retain(|&lid| lid != id);
    }

    /// Get a mutable reference to a light.
    pub fn get_mut(&mut self, id: LightId) -> Option<&mut Light> {
        self.lights.get_mut(&id)
    }

    /// Get an immutable reference to a light.
    pub fn get(&self, id: LightId) -> Option<&Light> {
        self.lights.get(&id)
    }

    /// Get the GPU data for all active lights, up to MAX_LIGHTS.
    pub fn get_active_light_data(&self) -> Vec<LightData> {
        self.active_lights
            .iter()
            .take(MAX_LIGHTS)
            .filter_map(|&id| self.lights.get(&id).map(|light| light.to_gpu_data()))
            .collect()
    }

    /// Get the number of active lights.
    pub fn active_light_count(&self) -> usize {
        self.active_lights.len().min(MAX_LIGHTS)
    }

    /// Get all active light IDs.
    pub fn active_lights(&self) -> &[LightId] {
        &self.active_lights
    }

    /// Disable a light without removing it.
    pub fn set_enabled(&mut self, id: LightId, enabled: bool) {
        if let Some(light) = self.lights.get_mut(&id) {
            light.set_enabled(enabled);
        }
    }
}

impl Default for LightManager {
    fn default() -> Self {
        Self::new()
    }
}
