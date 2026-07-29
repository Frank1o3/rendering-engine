// src/voxel/mesher.rs
//
// Naive per-block face culling: for every solid block, emit a quad for
// each of its 6 faces bordering a non-solid neighbor. No merging of
// coplanar faces yet (greedy meshing is phase 3) — this exists to validate
// the chunk data model, the free-list-backed GeometryPool, and the render
// path end-to-end with real geometry before adding that complexity.

use super::block::BlockId;
use super::chunk::{CHUNK_HEIGHT, CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk};
use rendering_engine::mesh::{MeshData, Vertex};

/// One cube face: 4 corner offsets in the block's local unit cube [0,1]³,
/// CCW winding viewed from outside along `normal` — matches the winding
/// convention used everywhere else in the engine (CullMode::Back expects
/// CCW front faces), and matches meshes.rs's existing per-face vertex order
/// exactly (just shifted from centered ±0.5 to a 0..1 grid cell).
struct FaceDef {
    corners: [[f32; 3]; 4],
    normal: [f32; 3],
    delta: (i32, i32, i32),
}

const FACES: [FaceDef; 6] = [
    FaceDef {
        corners: [
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ],
        normal: [1.0, 0.0, 0.0],
        delta: (1, 0, 0),
    },
    FaceDef {
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ],
        normal: [-1.0, 0.0, 0.0],
        delta: (-1, 0, 0),
    },
    FaceDef {
        corners: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        normal: [0.0, 1.0, 0.0],
        delta: (0, 1, 0),
    },
    FaceDef {
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        normal: [0.0, -1.0, 0.0],
        delta: (0, -1, 0),
    },
    FaceDef {
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        normal: [0.0, 0.0, 1.0],
        delta: (0, 0, 1),
    },
    FaceDef {
        corners: [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
        normal: [0.0, 0.0, -1.0],
        delta: (0, 0, -1),
    },
];

/// Meshes a single chunk. `neighbor_solid` is called only for lookups that
/// fall outside this chunk's own bounds (its edges) — pass a closure that
/// checks loaded adjacent chunks. Return `false` if the neighbor chunk
/// isn't loaded yet: this errs toward an extra border face rather than a
/// hole in the world, and the border face gets naturally hidden the moment
/// the neighbor chunk loads and re-meshes this one.
pub fn mesh_chunk(chunk: &Chunk, neighbor_solid: impl Fn(i32, i32, i32) -> bool) -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let is_solid = |x: i32, y: i32, z: i32| -> bool {
        match chunk.get_checked(x, y, z) {
            Some(b) => b.is_solid(),
            None => neighbor_solid(x, y, z),
        }
    };

    for y in 0..CHUNK_HEIGHT {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let block = chunk.get(x, y, z);
                if !block.is_solid() {
                    continue;
                }

                let color = block.color();
                let (bx, by, bz) = (x as f32, y as f32, z as f32);

                for face in &FACES {
                    let (dx, dy, dz) = face.delta;
                    let (nx, ny, nz) = (x as i32 + dx, y as i32 + dy, z as i32 + dz);
                    if is_solid(nx, ny, nz) {
                        continue;
                    }

                    let base = vertices.len() as u32;
                    for corner in &face.corners {
                        vertices.push(Vertex::new(
                            [bx + corner[0], by + corner[1], bz + corner[2]],
                            face.normal,
                            color,
                        ));
                    }
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base + 2,
                        base + 3,
                        base,
                    ]);
                }
            }
        }
    }

    MeshData { vertices, indices }
}
