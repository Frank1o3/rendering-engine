use super::chunk::{CHUNK_HEIGHT, CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk};
use rendering_engine::resources::mesh::{MeshData, Vertex};

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

fn chunk_y_bounds(chunk: &Chunk) -> Option<(i32, i32)> {
    let mut min_y = None;
    'find_min: for y in 0..CHUNK_HEIGHT {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                if chunk.get(x, y, z).is_solid() {
                    min_y = Some(y as i32);
                    break 'find_min;
                }
            }
        }
    }
    let min_y = min_y?;

    let mut max_y = min_y;
    'find_max: for y in (min_y as usize..CHUNK_HEIGHT).rev() {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                if chunk.get(x, y, z).is_solid() {
                    max_y = y as i32;
                    break 'find_max;
                }
            }
        }
    }
    Some((min_y, max_y))
}

fn axis_extent(axis: (i32, i32, i32), min_y: i32, max_y: i32) -> (usize, i32) {
    if axis.0 != 0 {
        (CHUNK_SIZE_X, 0)
    } else if axis.1 != 0 {
        (((max_y - min_y + 1).max(1)) as usize, min_y)
    } else {
        (CHUNK_SIZE_Z, 0)
    }
}

pub fn mesh_chunk(chunk: &Chunk, neighbor_solid: impl Fn(i32, i32, i32) -> bool) -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let Some((min_y, max_y)) = chunk_y_bounds(chunk) else {
        return MeshData { vertices, indices };
    };

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

        let (depth_size, depth_axis_off) = axis_extent(face.dir, min_y, max_y);
        let (u_size, _u_axis_off) = axis_extent(face.u_axis, min_y, max_y);
        let (v_size, v_axis_off) = axis_extent(face.v_axis, min_y, max_y);

        let to_xyz = |d: i32, u: i32, v: i32| -> (i32, i32, i32) {
            if dx != 0 {
                (d, v + v_axis_off, u)
            } else if dy != 0 {
                (u, d, v)
            } else {
                (u, v + v_axis_off, d)
            }
        };

        let mut visible = vec![false; u_size * v_size];
        let mut visited = vec![false; u_size * v_size];
        let idx = |u: usize, v: usize| u * v_size + v;

        for d_idx in 0..depth_size as i32 {
            let d = d_idx + depth_axis_off;

            visible.iter_mut().for_each(|b| *b = false);
            visited.iter_mut().for_each(|b| *b = false);

            for u in 0..u_size as i32 {
                for v in 0..v_size as i32 {
                    let (x, y, z) = to_xyz(d, u, v);
                    if !is_solid(x, y, z) {
                        continue;
                    }
                    if is_solid(x + dx, y + dy, z + dz) {
                        continue;
                    }
                    visible[idx(u as usize, v as usize)] = true;
                }
            }

            for u in 0..u_size {
                for v in 0..v_size {
                    if !visible[idx(u, v)] || visited[idx(u, v)] {
                        continue;
                    }

                    let mut width = 1;
                    while u + width < u_size
                        && visible[idx(u + width, v)]
                        && !visited[idx(u + width, v)]
                    {
                        width += 1;
                    }

                    let mut height = 1;
                    'grow: while v + height < v_size {
                        for u_off in 0..width {
                            if !visible[idx(u + u_off, v + height)]
                                || visited[idx(u + u_off, v + height)]
                            {
                                break 'grow;
                            }
                        }
                        height += 1;
                    }

                    for u_off in 0..width {
                        for v_off in 0..height {
                            visited[idx(u + u_off, v + v_off)] = true;
                        }
                    }

                    let plane_coord = if dx > 0 || dy > 0 || dz > 0 { d + 1 } else { d };

                    let corner = |u_off: i32, v_off: i32| -> [f32; 3] {
                        let uu = (u as i32 + u_off) as f32;
                        let vv = (v as i32 + v_off + v_axis_off) as f32;
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

                    let corners = if n.dot(desired) < 0.0 {
                        [c0, c3, c2, c1]
                    } else {
                        [c0, c1, c2, c3]
                    };

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
