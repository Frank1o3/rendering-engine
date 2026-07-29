// demo/src/voxel/mesher.rs
//
// Greedy meshing for a single chunk, done independently per depth-slice
// along each face's normal axis. Merges adjacent coplanar exposed faces
// within a slice into larger quads.
//
// A slice-per-depth pass is required: collapsing all depths into a single
// (u, v) mask (as an earlier version did) silently overwrites faces from
// different depths and mispositions everything at a fixed chunk edge —
// that produced the spike/wall artifacts and stray magenta ("unknown
// block") faces seen in testing.

use super::chunk::{CHUNK_HEIGHT, CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk};
use rendering_engine::mesh::{MeshData, Vertex};

#[derive(Clone, Copy)]
struct Face {
    normal: [f32; 3],
    dir: (i32, i32, i32),
    u_axis: (i32, i32, i32),
    v_axis: (i32, i32, i32),
}

const FACES: [Face; 6] = [
    Face {
        normal: [1.0, 0.0, 0.0],
        dir: (1, 0, 0),
        u_axis: (0, 0, 1),
        v_axis: (0, 1, 0),
    },
    Face {
        normal: [-1.0, 0.0, 0.0],
        dir: (-1, 0, 0),
        u_axis: (0, 0, 1),
        v_axis: (0, 1, 0),
    },
    Face {
        normal: [0.0, 1.0, 0.0],
        dir: (0, 1, 0),
        u_axis: (1, 0, 0),
        v_axis: (0, 0, 1),
    },
    Face {
        normal: [0.0, -1.0, 0.0],
        dir: (0, -1, 0),
        u_axis: (1, 0, 0),
        v_axis: (0, 0, 1),
    },
    Face {
        normal: [0.0, 0.0, 1.0],
        dir: (0, 0, 1),
        u_axis: (1, 0, 0),
        v_axis: (0, 1, 0),
    },
    Face {
        normal: [0.0, 0.0, -1.0],
        dir: (0, 0, -1),
        u_axis: (1, 0, 0),
        v_axis: (0, 1, 0),
    },
];

pub fn mesh_chunk(chunk: &Chunk, neighbor_solid: impl Fn(i32, i32, i32) -> bool) -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let is_solid = |x: i32, y: i32, z: i32| -> bool {
        match chunk.get_checked(x, y, z) {
            Some(b) => b.is_solid(),
            None => neighbor_solid(x, y, z),
        }
    };

    for face in &FACES {
        let (dx, dy, dz) = face.dir;
        let (ux, uy, uz) = face.u_axis;
        let (vx, vy, vz) = face.v_axis;
        let axis_unit = (dx.abs(), dy.abs(), dz.abs());

        // depth = the axis the face normal points along; u/v = the other two.
        let depth_size = if dx != 0 {
            CHUNK_SIZE_X
        } else if dy != 0 {
            CHUNK_HEIGHT
        } else {
            CHUNK_SIZE_Z
        };
        let u_size = if ux != 0 {
            CHUNK_SIZE_X
        } else if uy != 0 {
            CHUNK_HEIGHT
        } else {
            CHUNK_SIZE_Z
        };
        let v_size = if vx != 0 {
            CHUNK_SIZE_X
        } else if vy != 0 {
            CHUNK_HEIGHT
        } else {
            CHUNK_SIZE_Z
        };

        // Maps this face's local (depth, u, v) back to real chunk-local (x, y, z).
        let to_xyz = |d: i32, u: i32, v: i32| -> (i32, i32, i32) {
            if dx != 0 {
                (d, v, u)
            } else if dy != 0 {
                (u, d, v)
            } else {
                (u, v, d)
            }
        };

        // One independent 2D mask per depth slice — the actual fix: faces
        // at different depths along the normal axis can no longer merge
        // with or overwrite each other.
        for d in 0..depth_size as i32 {
            let mut visible = vec![vec![false; v_size]; u_size];

            for u in 0..u_size as i32 {
                for v in 0..v_size as i32 {
                    let (x, y, z) = to_xyz(d, u, v);
                    if !is_solid(x, y, z) {
                        continue;
                    }
                    if is_solid(x + dx, y + dy, z + dz) {
                        continue;
                    }
                    visible[u as usize][v as usize] = true;
                }
            }

            let mut visited = vec![vec![false; v_size]; u_size];

            for u in 0..u_size {
                for v in 0..v_size {
                    if !visible[u][v] || visited[u][v] {
                        continue;
                    }

                    let mut width = 1;
                    while u + width < u_size && visible[u + width][v] && !visited[u + width][v] {
                        width += 1;
                    }

                    let mut height = 1;
                    'grow: while v + height < v_size {
                        for u_off in 0..width {
                            if !visible[u + u_off][v + height] || visited[u + u_off][v + height] {
                                break 'grow;
                            }
                        }
                        height += 1;
                    }

                    for u_off in 0..width {
                        for v_off in 0..height {
                            visited[u + u_off][v + v_off] = true;
                        }
                    }

                    // Face plane is the boundary between the solid block at
                    // depth `d` and its non-solid neighbor: d+1 on the
                    // positive side, d itself on the negative side.
                    let plane_coord = if dx > 0 || dy > 0 || dz > 0 { d + 1 } else { d };

                    let corner = |u_off: i32, v_off: i32| -> [f32; 3] {
                        let uu = (u as i32 + u_off) as f32;
                        let vv = (v as i32 + v_off) as f32;
                        let pp = plane_coord as f32;
                        [
                            pp * axis_unit.0 as f32 + uu * ux as f32 + vv * vx as f32,
                            pp * axis_unit.1 as f32 + uu * uy as f32 + vv * vy as f32,
                            pp * axis_unit.2 as f32 + uu * uz as f32 + vv * vz as f32,
                        ]
                    };

                    let c0 = corner(0, 0);
                    let c1 = corner(width as i32, 0);
                    let c2 = corner(width as i32, height as i32);
                    let c3 = corner(0, height as i32);

                    let e1 = glam::Vec3::from(c1) - glam::Vec3::from(c0);
                    let e2 = glam::Vec3::from(c3) - glam::Vec3::from(c0);
                    let n = e1.cross(e2).normalize_or_zero();
                    let desired = glam::Vec3::from(face.normal);

                    // Correct winding fix — this branch actually takes
                    // effect now (the previous version's swap was scoped to
                    // a shadowed `let` inside the `if` and never reached
                    // the `corners` array, so back-facing quads got
                    // silently backface-culled instead of fixed).
                    let corners = if n.dot(desired) < 0.0 {
                        [c0, c3, c2, c1]
                    } else {
                        [c0, c1, c2, c3]
                    };

                    // Color from the block that actually owns this face —
                    // at depth `d` on the solid side, not the face plane.
                    let (bx, by, bz) = to_xyz(d, u as i32, v as i32);
                    let block = chunk.get(bx as usize, by as usize, bz as usize);
                    let color = block.color();

                    let base = vertices.len() as u32;
                    for &c in &corners {
                        vertices.push(Vertex::new(c, face.normal, color));
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
