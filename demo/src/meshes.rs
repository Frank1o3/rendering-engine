// src/meshes.rs

use rendering_engine::mesh::{MeshData, Vertex};

/// Helper: create a cube mesh with the same vertex colour for all faces.
fn create_solid_cube_mesh(color: [u8; 4]) -> MeshData {
    let mut vertices = Vec::with_capacity(24);

    // Front
    vertices.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, 0.0, 1.0], color));
    vertices.push(Vertex::new([0.5, -0.5, 0.5], [0.0, 0.0, 1.0], color));
    vertices.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], color));
    vertices.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 0.0, 1.0], color));

    // Back
    vertices.push(Vertex::new([0.5, -0.5, -0.5], [0.0, 0.0, -1.0], color));
    vertices.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], color));
    vertices.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 0.0, -1.0], color));
    vertices.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 0.0, -1.0], color));

    // Top
    vertices.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 1.0, 0.0], color));
    vertices.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], color));
    vertices.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 1.0, 0.0], color));
    vertices.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 1.0, 0.0], color));

    // Bottom
    vertices.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0], color));
    vertices.push(Vertex::new([0.5, -0.5, -0.5], [0.0, -1.0, 0.0], color));
    vertices.push(Vertex::new([0.5, -0.5, 0.5], [0.0, -1.0, 0.0], color));
    vertices.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, -1.0, 0.0], color));

    // Right
    vertices.push(Vertex::new([0.5, -0.5, 0.5], [1.0, 0.0, 0.0], color));
    vertices.push(Vertex::new([0.5, -0.5, -0.5], [1.0, 0.0, 0.0], color));
    vertices.push(Vertex::new([0.5, 0.5, -0.5], [1.0, 0.0, 0.0], color));
    vertices.push(Vertex::new([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], color));

    // Left
    vertices.push(Vertex::new([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0], color));
    vertices.push(Vertex::new([-0.5, -0.5, 0.5], [-1.0, 0.0, 0.0], color));
    vertices.push(Vertex::new([-0.5, 0.5, 0.5], [-1.0, 0.0, 0.0], color));
    vertices.push(Vertex::new([-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0], color));

    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    MeshData { vertices, indices }
}

/// Creates the demo cube mesh with per‑face colours.
pub fn create_cube_mesh() -> MeshData {
    let r = [220u8, 60, 60, 255];
    let g = [60u8, 200, 60, 255];
    let b = [60u8, 60, 220, 255];
    let y = [220u8, 200, 40, 255];
    let m = [180u8, 60, 220, 255];
    let c = [40u8, 200, 220, 255];

    let mut vertices = Vec::with_capacity(24);

    // Front
    vertices.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, 0.0, 1.0], r));
    vertices.push(Vertex::new([0.5, -0.5, 0.5], [0.0, 0.0, 1.0], r));
    vertices.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], r));
    vertices.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 0.0, 1.0], r));

    // Back
    vertices.push(Vertex::new([0.5, -0.5, -0.5], [0.0, 0.0, -1.0], g));
    vertices.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], g));
    vertices.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 0.0, -1.0], g));
    vertices.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 0.0, -1.0], g));

    // Top
    vertices.push(Vertex::new([-0.5, 0.5, 0.5], [0.0, 1.0, 0.0], b));
    vertices.push(Vertex::new([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], b));
    vertices.push(Vertex::new([0.5, 0.5, -0.5], [0.0, 1.0, 0.0], b));
    vertices.push(Vertex::new([-0.5, 0.5, -0.5], [0.0, 1.0, 0.0], b));

    // Bottom
    vertices.push(Vertex::new([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0], y));
    vertices.push(Vertex::new([0.5, -0.5, -0.5], [0.0, -1.0, 0.0], y));
    vertices.push(Vertex::new([0.5, -0.5, 0.5], [0.0, -1.0, 0.0], y));
    vertices.push(Vertex::new([-0.5, -0.5, 0.5], [0.0, -1.0, 0.0], y));

    // Right
    vertices.push(Vertex::new([0.5, -0.5, 0.5], [1.0, 0.0, 0.0], m));
    vertices.push(Vertex::new([0.5, -0.5, -0.5], [1.0, 0.0, 0.0], m));
    vertices.push(Vertex::new([0.5, 0.5, -0.5], [1.0, 0.0, 0.0], m));
    vertices.push(Vertex::new([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], m));

    // Left
    vertices.push(Vertex::new([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0], c));
    vertices.push(Vertex::new([-0.5, -0.5, 0.5], [-1.0, 0.0, 0.0], c));
    vertices.push(Vertex::new([-0.5, 0.5, 0.5], [-1.0, 0.0, 0.0], c));
    vertices.push(Vertex::new([-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0], c));

    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    MeshData { vertices, indices }
}

/// Golden cube used as a collectible.
pub fn create_collectible_mesh() -> MeshData {
    create_solid_cube_mesh([255, 215, 0, 255]) // gold
}

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
