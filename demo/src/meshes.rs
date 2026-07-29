// src/meshes.rs

use rendering_engine::mesh::{MeshData, Vertex};

/// Unit quad used for UI text rendering.
pub fn create_quad_mesh() -> MeshData {
    let normal: [i8; 4] = [0, 0, 127, 0];

    MeshData {
        vertices: vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
            },
            Vertex {
                position: [1.0, 0.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
            },
            Vertex {
                position: [0.0, 1.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
            },
        ],
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}

/// Translucent quad used for Android touch buttons.
pub fn create_button_quad_mesh() -> MeshData {
    let normal: [i8; 4] = [0, 0, 127, 0];
    let color = [70u8, 70, 80, 140];

    MeshData {
        vertices: vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal,
                color,
            },
            Vertex {
                position: [1.0, 0.0, 0.0],
                normal,
                color,
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal,
                color,
            },
            Vertex {
                position: [0.0, 1.0, 0.0],
                normal,
                color,
            },
        ],
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}

/// Solid red quad for the vsync toggle button — visually distinct from the
/// grey movement/D-pad buttons so it reads as a mode switch, not a hold-key.
pub fn create_vsync_button_mesh() -> MeshData {
    let normal: [i8; 4] = [0, 0, 127, 0];
    let color = [220u8, 35, 35, 220]; // red, slightly translucent like the other buttons

    MeshData {
        vertices: vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal,
                color,
            },
            Vertex {
                position: [1.0, 0.0, 0.0],
                normal,
                color,
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal,
                color,
            },
            Vertex {
                position: [0.0, 1.0, 0.0],
                normal,
                color,
            },
        ],
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}
