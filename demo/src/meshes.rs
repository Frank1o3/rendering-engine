use rendering_engine::resources::mesh::{MeshData, Vertex};

/// Unit quad used for UI text rendering.
pub fn create_quad_mesh() -> MeshData {
    let normal: [i8; 4] = [0, 0, 127, 0];

    MeshData {
        vertices: vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
                uv: [0, 0],
            },
            Vertex {
                position: [1.0, 0.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
                uv: [10, 0],
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
                uv: [1, 1],
            },
            Vertex {
                position: [0.0, 1.0, 0.0],
                normal,
                color: [255, 255, 255, 255],
                uv: [0, 1],
            },
        ],
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}

/// Translucent quad used for Android touch buttons.
pub fn create_button_quad_mesh() -> MeshData {
    let normal: [i8; 4] = [0, 0, 127, 0];
    let color: [u8; 4] = [70u8, 70, 80, 140];

    MeshData {
        vertices: vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal,
                color,
                uv: [0, 0],
            },
            Vertex {
                position: [1.0, 0.0, 0.0],
                normal,
                color,
                uv: [1, 0],
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal,
                color,
                uv: [1, 1],
            },
            Vertex {
                position: [0.0, 1.0, 0.0],
                normal,
                color,
                uv: [0, 1],
            },
        ],
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}

/// Solid red quad for the vsync toggle button.
pub fn create_vsync_button_mesh() -> MeshData {
    let normal: [i8; 4] = [0, 0, 127, 0];
    let color: [u8; 4] = [220u8, 35, 35, 220];

    MeshData {
        vertices: vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal,
                color,
                uv: [0, 0],
            },
            Vertex {
                position: [1.0, 0.0, 0.0],
                normal,
                color,
                uv: [1, 0],
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal,
                color,
                uv: [1, 1],
            },
            Vertex {
                position: [0.0, 1.0, 0.0],
                normal,
                color,
                uv: [0, 1],
            },
        ],
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}

/// Solid red quad for the vsync toggle button.
pub fn create_wireframe_button_mesh() -> MeshData {
    let normal: [i8; 4] = [0, 0, 127, 0];
    let color: [u8; 4] = [35, 35, 220u8, 220];

    MeshData {
        vertices: vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal,
                color,
                uv: [0, 0],
            },
            Vertex {
                position: [1.0, 0.0, 0.0],
                normal,
                color,
                uv: [1, 0],
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal,
                color,
                uv: [1, 1],
            },
            Vertex {
                position: [0.0, 1.0, 0.0],
                normal,
                color,
                uv: [0, 1],
            },
        ],
        indices: vec![0, 1, 2, 2, 3, 0],
    }
}
