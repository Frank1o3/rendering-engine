/// Lighting system for the renderer.
///
/// Supports directional, point, and spot lights with support for
/// shadow mapping in future extensions.
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::f32::consts::PI;

fn safe_direction(direction: Vec3) -> Vec3 {
    direction.try_normalize().unwrap_or(Vec3::NEG_Y)
}

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

/// Light data as it appears in the engine light uniform block.
/// Each group occupies one std140 `vec4` slot (64 bytes total).
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct LightData {
    /// Light position (for point/spot), or negative direction (for directional).
    /// For directional lights, the direction is normalized.
    pub position_or_dir: Vec3,
    /// Light type (0=Directional, 1=Point, 2=Spot).
    pub light_type: u32,

    /// Light color.
    pub color: Vec3,
    /// Light intensity.
    pub intensity: f32,

    /// Spot direction (normalized), unused for other types.
    pub spot_direction: Vec3,
    /// Point/spot attenuation range.
    pub range: f32,

    /// Cosine of the spot light half-angle; ignored for other light types.
    pub spot_angle_cos: f32,
    /// Ambient influence (0.0 to 1.0).
    pub ambient_influence: f32,
    /// Whether this light casts shadows.
    pub casts_shadow: u32,
    /// Specular intensity.
    pub specular_intensity: f32,
}

/// Fixed-size std140 uniform block uploaded by `Renderer` once per frame.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct LightUniformBlock {
    pub light_count: u32,
    pub _padding: [u32; 3],
    pub lights: [LightData; MAX_LIGHTS],
}

impl Default for LightUniformBlock {
    fn default() -> Self {
        Self::zeroed()
    }
}

/// GLSL declarations matching [`LightUniformBlock`]. Include this text in a
/// shader and call `Renderer::bind_light_block` for its `EngineLights` block.
pub const LIGHTS_BLOCK_GLSL: &str = r#"
struct EngineLight {
    vec3 position_or_dir; uint light_type;
    vec3 color; float intensity;
    vec3 spot_direction; float range;
    float spot_angle_cos; float ambient_influence;
    uint casts_shadow; float specular_intensity;
};
layout(std140) uniform EngineLights {
    uint uLightCount;
    uint _lightPadding0;
    uint _lightPadding1;
    uint _lightPadding2;
    EngineLight uLights[16];
};
"#;

const _: () = assert!(std::mem::size_of::<LightData>() == 64);
const _: () = assert!(std::mem::size_of::<LightUniformBlock>() == 16 + 64 * MAX_LIGHTS);

impl LightData {
    /// Create light data for a directional light.
    pub fn directional(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            position_or_dir: -safe_direction(direction),
            light_type: LightType::Directional as u32,
            color,
            intensity,
            spot_direction: Vec3::ZERO,
            range: f32::INFINITY,
            spot_angle_cos: -1.0,
            ambient_influence: 0.0,
            casts_shadow: 0,
            specular_intensity: 1.0,
        }
    }

    /// Create light data for a point light.
    pub fn point(position: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            position_or_dir: position,
            light_type: LightType::Point as u32,
            color,
            intensity,
            spot_direction: Vec3::ZERO,
            range,
            spot_angle_cos: -1.0,
            ambient_influence: 0.0,
            casts_shadow: 0,
            specular_intensity: 1.0,
        }
    }

    /// Create light data for a spot light.
    pub fn spot(
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        angle: f32,
    ) -> Self {
        let half_angle = angle * 0.5;
        Self {
            position_or_dir: position,
            light_type: LightType::Spot as u32,
            color,
            intensity,
            spot_direction: safe_direction(direction),
            range,
            spot_angle_cos: half_angle.cos(),
            ambient_influence: 0.0,
            casts_shadow: 0,
            specular_intensity: 1.0,
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
            direction: safe_direction(direction),
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
            direction: safe_direction(direction),
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
        self.direction = safe_direction(dir);
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
            .filter_map(|&id| {
                self.lights
                    .get(&id)
                    .filter(|light| light.enabled())
                    .map(|light| light.to_gpu_data())
            })
            .take(MAX_LIGHTS)
            .collect()
    }

    /// Fill fixed-size GPU data without allocating a temporary vector.
    pub fn write_uniform_block(&self, block: &mut LightUniformBlock) {
        *block = LightUniformBlock::default();
        for &id in &self.active_lights {
            if block.light_count as usize >= MAX_LIGHTS {
                break;
            }
            if let Some(light) = self.lights.get(&id).filter(|light| light.enabled()) {
                block.lights[block.light_count as usize] = light.to_gpu_data();
                block.light_count += 1;
            }
        }
    }

    /// Get the number of active lights.
    pub fn active_light_count(&self) -> usize {
        self.active_lights
            .iter()
            .filter(|id| self.lights.get(id).is_some_and(Light::enabled))
            .take(MAX_LIGHTS)
            .count()
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
