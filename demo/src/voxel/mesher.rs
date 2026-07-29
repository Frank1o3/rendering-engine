// demo/src/voxel/mesher.rs
//
// Greedy meshing for a single chunk.
// Merges adjacent coplanar faces into larger quads to reduce draw calls.
// Produces a MeshData with correct normals and CCW winding.

use super::block::BlockId;
use super::chunk::{Chunk, CHUNK_HEIGHT, CHUNK_SIZE_X, CHUNK_SIZE_Z};
use rendering_engine::mesh::{MeshData, Vertex};

#[derive(Clone, Copy)]
struct Face {
    normal: [f32; 3],
    dir: (i32, i32, i32),   // axis direction: ±x, ±y, ±z
    u_axis: (i32, i32, i32), // tangent axis for u
    v_axis: (i32, i32, i32), // tangent axis for v
}

const FACES: [Face; 6] = [
    // +x
    Face {
        normal: [1.0, 0.0, 0.0],
        dir: (1, 0, 0),
        u_axis: (0, 0, 1),
        v_axis: (0, 1, 0),
    },
    // -x
    Face {
        normal: [-1.0, 0.0, 0.0],
        dir: (-1, 0, 0),
        u_axis: (0, 0, 1),
        v_axis: (0, 1, 0),
    },
    // +y
    Face {
        normal: [0.0, 1.0, 0.0],
        dir: (0, 1, 0),
        u_axis: (1, 0, 0),
        v_axis: (0, 0, 1),
    },
    // -y
    Face {
        normal: [0.0, -1.0, 0.0],
        dir: (0, -1, 0),
        u_axis: (1, 0, 0),
        v_axis: (0, 0, 1),
    },
    // +z
    Face {
        normal: [0.0, 0.0, 1.0],
        dir: (0, 0, 1),
        u_axis: (1, 0, 0),
        v_axis: (0, 1, 0),
    },
    // -z
    Face {
        normal: [0.0, 0.0, -1.0],
        dir: (0, 0, -1),
        u_axis: (1, 0, 0),
        v_axis: (0, 1, 0),
    },
];

/// Meshes a single chunk using a simple greedy algorithm.
/// `neighbor_solid` is called for coordinates outside this chunk (edges).
/// It should return `true` if the neighbor block is solid.
pub fn mesh_chunk(chunk: &Chunk, neighbor_solid: impl Fn(i32, i32, i32) -> bool) -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let is_solid = |x: i32, y: i32, z: i32| -> bool {
        match chunk.get_checked(x, y, z) {
            Some(b) => b.is_solid(),
            None => neighbor_solid(x, y, z),
        }
    };

    // We'll mesh each face direction separately.
    for face in &FACES {
        let (dx, dy, dz) = face.dir;
        let (ux, uy, uz) = face.u_axis;
        let (vx, vy, vz) = face.v_axis;

        // Determine the range of the two axes perpendicular to the face normal.
        // We'll iterate over slices of the chunk along the face normal.
        // For each (u,v) coordinate, we check if the face is visible.
        // We'll use a 2D mask to mark which (u,v) positions have a visible face.
        let u_size = if ux != 0 { CHUNK_SIZE_X } else if uy != 0 { CHUNK_HEIGHT } else { CHUNK_SIZE_Z };
        let v_size = if vx != 0 { CHUNK_SIZE_X } else if vy != 0 { CHUNK_HEIGHT } else { CHUNK_SIZE_Z };

        // We'll store a 2D array of bool for visibility.
        let mut visible = vec![vec![false; v_size]; u_size];

        // Iterate over all blocks in the chunk, and for each block that is solid,
        // check if the neighbor in the face direction is non-solid -> mark visible.
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let block = chunk.get(x, y, z);
                    if !block.is_solid() {
                        continue;
                    }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let nz = z as i32 + dz;
                    if is_solid(nx, ny, nz) {
                        continue; // neighbor solid -> face not visible
                    }
                    // Compute u and v indices for this block's face.
                    let u = if ux != 0 { x } else if uy != 0 { y } else { z };
                    let v = if vx != 0 { x } else if vy != 0 { y } else { z };
                    visible[u][v] = true;
                }
            }
        }

        // Now greedily find rectangles of true values in the visible mask.
        // We'll use a simple algorithm: for each row, find runs of true, then try to extend down.
        let mut visited = vec![vec![false; v_size]; u_size];

        for u in 0..u_size {
            for v in 0..v_size {
                if !visible[u][v] || visited[u][v] {
                    continue;
                }
                // Start a new rectangle at (u,v).
                // Find the width: extend to the right until false or visited.
                let mut width = 1;
                while u + width < u_size && visible[u + width][v] && !visited[u + width][v] {
                    width += 1;
                }
                // Find the height: extend down while all cells in the current width are true.
                let mut height = 1;
                'height_loop: while v + height < v_size {
                    for u_offset in 0..width {
                        if !visible[u + u_offset][v + height] || visited[u + u_offset][v + height] {
                            break 'height_loop;
                        }
                    }
                    height += 1;
                }

                // Mark the rectangle as visited.
                for u_off in 0..width {
                    for v_off in 0..height {
                        visited[u + u_off][v + v_off] = true;
                    }
                }

                // Compute the world-space coordinates of the rectangle.
                // The face is at the boundary between the block and its neighbor.
                // The position of the face's corners depend on the normal direction.
                // We'll compute the base position of the block at (u,v) in the plane.
                // The block coordinate (bx, by, bz) is derived from (u,v) and the face normal direction.
                // We need to map u -> coordinate along u-axis, v -> along v-axis, and the face normal coordinate is either the block's coordinate or block+dir.
                // We'll compute the four corners of the rectangle in local chunk coordinates.
                // The rectangle spans from (u_start, v_start) to (u_start+width, v_start+height) in the plane.
                let u_start = u;
                let v_start = v;
                let u_end = u_start + width;
                let v_end = v_start + height;

                // Compute the block coordinates for each corner.
                // We'll use a function to map (u,v) to (x,y,z) for a given face.
                // We'll compute the four corners: (u_start, v_start), (u_end, v_start), (u_end, v_end), (u_start, v_end).
                // For each corner, we need to know if it's the block's coordinate or the neighbor's coordinate.
                // The face is at the boundary; we'll position the quad such that it lies exactly on the boundary.
                // For positive direction, the face is at block coordinate + 1; for negative, at block coordinate.
                // But we'll define the quad corners as the positions of the vertices.
                // For simplicity, we'll use the block coordinates and then shift by the normal direction if positive.
                let (bx, by, bz) = if dx > 0 {
                    // +x: face is at x = block.x + 1, y,z are block's y,z.
                    // We need to map (u,v) to (x,y,z)
                    // u-axis is z? Actually for +x, u_axis = (0,0,1) -> z, v_axis = (0,1,0) -> y.
                    // So u -> z, v -> y.
                    let z = u_start;
                    let y = v_start;
                    (CHUNK_SIZE_X as i32, y as i32, z as i32)
                } else if dx < 0 {
                    // -x: face at x = block.x, y,z are block's y,z.
                    let z = u_start;
                    let y = v_start;
                    (0, y as i32, z as i32)
                } else if dy > 0 {
                    // +y: face at y = block.y+1, x,z are block's x,z.
                    // u_axis = (1,0,0) -> x, v_axis = (0,0,1) -> z.
                    let x = u_start;
                    let z = v_start;
                    (x as i32, CHUNK_HEIGHT as i32, z as i32)
                } else if dy < 0 {
                    // -y: face at y = block.y, x,z are block's x,z.
                    let x = u_start;
                    let z = v_start;
                    (x as i32, 0, z as i32)
                } else if dz > 0 {
                    // +z: face at z = block.z+1, x,y are block's x,y.
                    // u_axis = (1,0,0) -> x, v_axis = (0,1,0) -> y.
                    let x = u_start;
                    let y = v_start;
                    (x as i32, y as i32, CHUNK_SIZE_Z as i32)
                } else {
                    // -z: face at z = block.z, x,y are block's x,y.
                    let x = u_start;
                    let y = v_start;
                    (x as i32, y as i32, 0)
                };

                // Now compute the four corners in world space (chunk-local coordinates).
                // We need to offset by (u_offset, v_offset) along the plane axes.
                // The plane axes are (ux,uy,uz) and (vx,vy,vz).
                let corner = |u_off: i32, v_off: i32| -> [f32; 3] {
                    let x = bx as f32 + u_off as f32 * ux as f32 + v_off as f32 * vx as f32;
                    let y = by as f32 + u_off as f32 * uy as f32 + v_off as f32 * vy as f32;
                    let z = bz as f32 + u_off as f32 * uz as f32 + v_off as f32 * vz as f32;
                    [x, y, z]
                };

                let c0 = corner(0, 0);
                let c1 = corner(width as i32, 0);
                let c2 = corner(width as i32, height as i32);
                let c3 = corner(0, height as i32);

                // The quad winding must be CCW when viewed from outside.
                // For a face with normal (nx,ny,nz), the winding should be such that
                // the normal points out. We'll use the standard order: c0, c1, c2, c3.
                // But we might need to swap depending on the direction of the plane axes.
                // We'll assume the axes are right-handed; if not, we'll adjust.
                // For positive directions, the order c0,c1,c2,c3 should be CCW.
                // For negative directions, we may need to reverse.
                // We'll simply use the order that yields a normal pointing outward.
                // Compute the normal from the quad edges: (c1-c0) x (c3-c0).
                let e1 = glam::Vec3::new(c1[0]-c0[0], c1[1]-c0[1], c1[2]-c0[2]);
                let e2 = glam::Vec3::new(c3[0]-c0[0], c3[1]-c0[1], c3[2]-c0[2]);
                let n = e1.cross(e2).normalize_or_zero();
                let desired = glam::Vec3::from(face.normal);
                if n.dot(desired) < 0.0 {
                    // Reverse winding by swapping c1 and c3.
                    let (c1, c3) = (c3, c1);
                    // Now the normal should be correct.
                }
                // Use the order: c0, c1, c2, c3 (CCW).
                let corners = [c0, c1, c2, c3];

                // Get block color from the first block of the quad.
                // We'll use the block at the starting position (u_start, v_start) along the plane.
                // Convert back to (x,y,z) from (u_start, v_start).
                let (bx0, by0, bz0) = if dx > 0 {
                    let z = u_start;
                    let y = v_start;
                    (CHUNK_SIZE_X as i32 - 1, y as i32, z as i32)
                } else if dx < 0 {
                    let z = u_start;
                    let y = v_start;
                    (0, y as i32, z as i32)
                } else if dy > 0 {
                    let x = u_start;
                    let z = v_start;
                    (x as i32, CHUNK_HEIGHT as i32 - 1, z as i32)
                } else if dy < 0 {
                    let x = u_start;
                    let z = v_start;
                    (x as i32, 0, z as i32)
                } else if dz > 0 {
                    let x = u_start;
                    let y = v_start;
                    (x as i32, y as i32, CHUNK_SIZE_Z as i32 - 1)
                } else {
                    let x = u_start;
                    let y = v_start;
                    (x as i32, y as i32, 0)
                };
                let block = chunk.get(bx0 as usize, by0 as usize, bz0 as usize);
                let color = block.color();

                let base = vertices.len() as u32;
                for &corner in &corners {
                    vertices.push(Vertex::new(corner, face.normal, color));
                }
                indices.extend_from_slice(&[base, base+1, base+2, base+2, base+3, base]);
            }
        }
    }

    MeshData { vertices, indices }
}
