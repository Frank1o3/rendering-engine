use crate::core::free_list::Allocation;
use bytemuck::{Pod, Zeroable};

/// 28-byte vertex: position (12) + normal (4, packed i8) + color (4) + uv (8).
///
/// Normal packing: three signed bytes in [-127, 127] map to [-1.0, 1.0] when
/// the attribute is declared `GL_BYTE, normalized=true`. The fourth byte is
/// always 0 (padding for alignment).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3], // 12 bytes  offset  0
    pub normal: [i8; 4],    //  4 bytes  offset 12  (xyz packed, w=0 padding)
    pub color: [u8; 4],     //  4 bytes  offset 16
    pub uv: [u16; 2],       //  8 bytes  offset 20
} // 28 bytes total

impl Vertex {
    /// Convenience constructor with default zero UVs.
    pub fn new(position: [f32; 3], normal: [f32; 3], color: [u8; 4]) -> Self {
        Self {
            position,
            normal: pack_normal(normal),
            color,
            uv: [0, 0],
        }
    }

    /// Full constructor including UV coordinates.
    pub fn new_with_uv(position: [f32; 3], normal: [f32; 3], color: [u8; 4], uv: [u16; 2]) -> Self {
        Self {
            position,
            normal: pack_normal(normal),
            color,
            uv,
        }
    }
}

/// Packs a unit normal into four signed bytes.
/// Each component is clamped and scaled to [-127, 127]; byte 3 is 0 (padding).
pub fn pack_normal(n: [f32; 3]) -> [i8; 4] {
    let clamp = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8;
    [clamp(n[0]), clamp(n[1]), clamp(n[2]), 0]
}

#[derive(Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Compute per-vertex normals by averaging the face normals of every
    /// triangle that shares each vertex.
    pub fn compute_normals(&mut self) {
        let mut accum: Vec<glam::Vec3> = vec![glam::Vec3::ZERO; self.vertices.len()];

        for tri in self.indices.chunks_exact(3) {
            let (ia, ib, ic) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let a = glam::Vec3::from(self.vertices[ia].position);
            let b = glam::Vec3::from(self.vertices[ib].position);
            let c = glam::Vec3::from(self.vertices[ic].position);
            let face_normal = (b - a).cross(c - a);
            accum[ia] += face_normal;
            accum[ib] += face_normal;
            accum[ic] += face_normal;
        }

        for (v, n) in self.vertices.iter_mut().zip(accum.iter()) {
            let unit = n.normalize_or_zero();
            v.normal = pack_normal(unit.to_array());
        }
    }

    /// Forces every triangle to satisfy CCW winding when viewed from outside.
    pub fn fix_winding(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        let centroid: glam::Vec3 = self
            .vertices
            .iter()
            .map(|v| glam::Vec3::from(v.position))
            .sum::<glam::Vec3>()
            / self.vertices.len() as f32;

        let mut flipped = 0;
        for tri in self.indices.chunks_exact_mut(3) {
            let a = glam::Vec3::from(self.vertices[tri[0] as usize].position);
            let b = glam::Vec3::from(self.vertices[tri[1] as usize].position);
            let c = glam::Vec3::from(self.vertices[tri[2] as usize].position);

            let face_normal = (b - a).cross(c - a);
            let to_face = a - centroid;

            if face_normal.dot(to_face) < 0.0 {
                tri.swap(1, 2);
                flipped += 1;
            }
        }

        if flipped > 0 {
            log::warn!("fix_winding: corrected {} backwards triangle(s)", flipped);
        }
    }

    /// Conservative bounding sphere radius: distance from the origin of local space
    /// to its furthest vertex (+ 5% padding).
    pub fn bounding_radius(&self) -> f32 {
        self.vertices
            .iter()
            .map(|v| {
                let p = glam::Vec3::from(v.position);
                p.length()
            })
            .fold(0.0_f32, f32::max)
            * 1.05
    }
}

/// A mesh is a view into the shared `GeometryPool`.
#[derive(Clone, Copy, Debug)]
pub struct Mesh {
    pub base_vertex: i32,
    pub first_index: u32,
    pub index_count: i32,
    pub bounding_radius: f32,
    pub(crate) vertex_alloc: Allocation,
    pub(crate) index_alloc: Allocation,
}
