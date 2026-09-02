/// Render pass abstraction for organizing rendering stages.
///
/// This module provides a lightweight pass system that separates concerns
/// like state management, viewport configuration, clear behavior, and
/// resource bindings for different rendering stages (Opaque, Transparent, UI, etc).
use glam::Vec4;
use std::cmp::Ordering;

/// Defines the type of rendering pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PassType {
    /// Opaque geometry pass (depth-tested, back-face culled).
    Opaque = 0,
    /// Transparent geometry pass (sorted by depth, blended).
    Transparent = 1,
    /// Skybox pass (depth-tested but no write).
    Skybox = 2,
    /// UI pass (viewport-local, no depth test).
    UI = 3,
    /// Shadow map pass (depth-only).
    Shadow = 4,
    /// Post-process pass (full-screen, no depth).
    PostProcess = 5,
}

/// Clear behavior for a pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearFlags {
    pub color: bool,
    pub depth: bool,
    pub stencil: bool,
}

impl ClearFlags {
    pub fn none() -> Self {
        Self {
            color: false,
            depth: false,
            stencil: false,
        }
    }

    pub fn color_and_depth() -> Self {
        Self {
            color: true,
            depth: true,
            stencil: false,
        }
    }

    pub fn depth_only() -> Self {
        Self {
            color: false,
            depth: true,
            stencil: false,
        }
    }

    pub fn all() -> Self {
        Self {
            color: true,
            depth: true,
            stencil: true,
        }
    }

    pub fn to_gl_bits(&self) -> u32 {
        let mut bits = 0u32;
        if self.color {
            bits |= 0x00004000; // glow::COLOR_BUFFER_BIT
        }
        if self.depth {
            bits |= 0x00000100; // glow::DEPTH_BUFFER_BIT
        }
        if self.stencil {
            bits |= 0x00000400; // glow::STENCIL_BUFFER_BIT
        }
        bits
    }
}

/// Configuration for a rendering pass.
#[derive(Clone, Debug)]
pub struct PassConfig {
    /// Type of pass.
    pub pass_type: PassType,

    /// What to clear at the start of this pass.
    pub clear_flags: ClearFlags,

    /// Clear color (only used if clear_flags.color is true).
    pub clear_color: Option<Vec4>,

    /// Viewport (x, y, width, height). None means full framebuffer.
    pub viewport: Option<(i32, i32, i32, i32)>,

    /// Whether to write to depth buffer.
    pub depth_write: bool,

    /// Whether to read from depth buffer (test).
    pub depth_test: bool,
}

impl PassConfig {
    pub fn new(pass_type: PassType) -> Self {
        match pass_type {
            PassType::Opaque => Self {
                pass_type,
                clear_flags: ClearFlags::none(),
                clear_color: None,
                viewport: None,
                depth_write: true,
                depth_test: true,
            },
            PassType::Transparent => Self {
                pass_type,
                clear_flags: ClearFlags::none(),
                clear_color: None,
                viewport: None,
                depth_write: false,
                depth_test: true,
            },
            PassType::Skybox => Self {
                pass_type,
                clear_flags: ClearFlags::color_and_depth(),
                clear_color: Some(Vec4::new(0.1, 0.1, 0.1, 1.0)),
                viewport: None,
                depth_write: false,
                depth_test: true,
            },
            PassType::UI => Self {
                pass_type,
                clear_flags: ClearFlags::none(),
                clear_color: None,
                viewport: None,
                depth_write: false,
                depth_test: false,
            },
            PassType::Shadow => Self {
                pass_type,
                clear_flags: ClearFlags::depth_only(),
                clear_color: None,
                viewport: None,
                depth_write: true,
                depth_test: true,
            },
            PassType::PostProcess => Self {
                pass_type,
                clear_flags: ClearFlags::none(),
                clear_color: None,
                viewport: None,
                depth_write: false,
                depth_test: false,
            },
        }
    }

    pub fn with_clear_color(mut self, color: Vec4) -> Self {
        self.clear_color = Some(color);
        self.clear_flags.color = true;
        self
    }

    pub fn with_viewport(mut self, x: i32, y: i32, w: i32, h: i32) -> Self {
        self.viewport = Some((x, y, w, h));
        self
    }

    pub fn with_clear_flags(mut self, flags: ClearFlags) -> Self {
        self.clear_flags = flags;
        self
    }
}

/// A pass group for batching draws of the same type.
///
/// Passes are automatically ordered to minimize state changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassGroup {
    pub pass_type: PassType,
    pub index: u32,
}

impl PartialOrd for PassGroup {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PassGroup {
    fn cmp(&self, other: &Self) -> Ordering {
        self.pass_type.cmp(&other.pass_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pass_group_order_is_stable() {
        let mut passes = vec![
            PassGroup {
                pass_type: PassType::UI,
                index: 2,
            },
            PassGroup {
                pass_type: PassType::Opaque,
                index: 1,
            },
            PassGroup {
                pass_type: PassType::Skybox,
                index: 0,
            },
        ];
        passes.sort();

        assert_eq!(passes[0].pass_type, PassType::Opaque);
        assert_eq!(passes[1].pass_type, PassType::Skybox);
        assert_eq!(passes[2].pass_type, PassType::UI);
    }

    #[test]
    fn pass_config_default_clear_flags_match_expected_stage() {
        let skybox = PassConfig::new(PassType::Skybox);
        assert!(skybox.clear_flags.color);
        assert!(skybox.clear_flags.depth);

        let opaque = PassConfig::new(PassType::Opaque);
        assert!(!opaque.clear_flags.color);
        assert!(!opaque.clear_flags.depth);
    }
}
