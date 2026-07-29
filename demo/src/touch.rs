// src/touch.rs

use crate::state::DemoState;

/// Virtual buttons shown on touch devices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonId {
    Forward,
    Back,
    Left,
    Right,
    Up,
    Down,
}

/// Simple screen-space rectangle (origin: top-left).
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[inline]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// Active touch state.
///
/// A touch is either:
/// - Holding one of the virtual buttons.
/// - Dragging to rotate the camera.
pub enum TouchKind {
    Button(ButtonId),
    VsyncToggle,
    Look { last: (f32, f32) },
}

/// Camera sensitivity for touch look.
pub const TOUCH_LOOK_SENSITIVITY: f32 = 0.004;

/// Vsync toggle button — smaller than the movement buttons, top-right corner,
/// clear of both the look-drag area and the D-pad/up-down cluster.
pub const VSYNC_BTN: f32 = 96.0;
pub const VSYNC_MARGIN: f32 = 32.0;

/// Button layout constants.
pub const BTN: f32 = 172.0;
pub const GAP: f32 = 20.0;
pub const MARGIN: f32 = 48.0;

/// Returns the six virtual button rectangles.
///
/// Layout:
///
/// ```text
///        [W]
///    [A][S][D]
///
///                [UP]
///              [DOWN]
/// ```
pub fn button_rects(width: f32, height: f32) -> [(ButtonId, Rect); 6] {
    let row_y = height - MARGIN - BTN;

    let a_x = MARGIN;
    let s_x = MARGIN + BTN + GAP;
    let d_x = MARGIN + 2.0 * (BTN + GAP);

    let w_y = row_y - BTN - GAP;

    let down_x = width - MARGIN - BTN;
    let down_y = height - MARGIN - BTN;
    let up_y = down_y - BTN - GAP;

    [
        (
            ButtonId::Forward,
            Rect {
                x: s_x,
                y: w_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Left,
            Rect {
                x: a_x,
                y: row_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Back,
            Rect {
                x: s_x,
                y: row_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Right,
            Rect {
                x: d_x,
                y: row_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Up,
            Rect {
                x: down_x,
                y: up_y,
                w: BTN,
                h: BTN,
            },
        ),
        (
            ButtonId::Down,
            Rect {
                x: down_x,
                y: down_y,
                w: BTN,
                h: BTN,
            },
        ),
    ]
}

pub fn vsync_button_rect(width: f32, _height: f32) -> Rect {
    Rect {
        x: width - VSYNC_MARGIN - VSYNC_BTN,
        y: VSYNC_MARGIN,
        w: VSYNC_BTN,
        h: VSYNC_BTN,
    }
}

/// Updates the keyboard movement state from a virtual button.
pub fn set_button_key(state: &mut DemoState, button: ButtonId, pressed: bool) {
    match button {
        ButtonId::Forward => state.keys.w = pressed,
        ButtonId::Back => state.keys.s = pressed,
        ButtonId::Left => state.keys.a = pressed,
        ButtonId::Right => state.keys.d = pressed,
        ButtonId::Up => state.keys.space = pressed,
        ButtonId::Down => state.keys.lctrl = pressed,
    }
}
