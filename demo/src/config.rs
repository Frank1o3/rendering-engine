// demo/src/config.rs
//
// All previously-hardcoded demo tuning values live here. Loaded once at
// startup from config.json; if the file doesn't exist (first run) or fails
// to parse, falls back to defaults and writes them out so the file is
// always there to edit afterward — e.g. bump render_distance and relaunch,
// no rebuild needed.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub render_distance: i32,
    pub fly_speed: f32,
    pub mouse_sensitivity: f32,
    pub touch_look_sensitivity: f32,
    pub fov_degrees: f32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub window_width: u32,
    pub window_height: u32,
    pub vsync_default: bool,
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            render_distance: 8,
            fly_speed: 12.0,
            mouse_sensitivity: 0.002,
            touch_look_sensitivity: 0.004,
            fov_degrees: 60.0,
            near_plane: 0.1,
            far_plane: 500.0,
            window_width: 1280,
            window_height: 720,
            vsync_default: false,
            sun_azimuth_deg: 45.0,
            sun_elevation_deg: 35.0,
        }
    }
}

impl Config {
    pub fn load_or_default() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::warn!("config.json failed to parse ({e}), using defaults");
                    let cfg = Self::default();
                    cfg.save();
                    cfg
                }
            },
            Err(_) => {
                let cfg = Self::default();
                cfg.save(); // first run — write out defaults so the file exists to edit
                cfg
            }
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(&path, text) {
                    log::warn!("Failed to write config.json: {e}");
                }
            }
            Err(e) => log::warn!("Failed to serialize config: {e}"),
        }
    }

    /// Sun direction as a normalized world-space vector, derived from the
    /// azimuth/elevation config values.
    pub fn sun_direction(&self) -> glam::Vec3 {
        let az = self.sun_azimuth_deg.to_radians();
        let el = self.sun_elevation_deg.to_radians();
        glam::Vec3::new(
            el.cos() * az.cos(),
            el.sin(),
            el.cos() * az.sin(),
        )
        .normalize()
    }

    fn path() -> PathBuf {
        Self::dir().join("config.json")
    }

    #[cfg(not(target_os = "android"))]
    fn dir() -> PathBuf {
        // Next to the executable — matches "raise render distance without
        // an app rebuild" for desktop.
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[cfg(target_os = "android")]
    fn dir() -> PathBuf {
        // NativeActivity has no "next to the binary" — the only writable
        // path comes from AndroidApp, captured once in android_main below.
        crate::android::DATA_DIR
            .get()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("/data/local/tmp"))
    }
}
