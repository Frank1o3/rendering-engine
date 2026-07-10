use bytemuck::{Pod, Zeroable};

// The optimized 16-byte vertex struct
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3], // 12 bytes
    pub color: [u8; 4],     // 4 bytes
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Forces every triangle to satisfy the engine's fixed convention:
    /// CCW winding when viewed from outside the mesh, i.e. the winding-derived
    /// normal (b-a) x (c-a) must point AWAY from the mesh centroid.
    ///
    /// Run this once at load time for anything procedurally generated or
    /// imported (cubes, quads, OBJ files) so `CullMode::Back` never silently
    /// eats faces that happened to be wound the wrong way.
    pub fn fix_winding(&mut self) {
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
}

/// A mesh is now just a view into the shared `GeometryPool` — it owns no GL
/// resources of its own. Created by `Renderer::load_mesh`, which uploads the
/// data into the pool and hands back this descriptor.
#[derive(Clone, Copy, Debug)]
pub struct Mesh {
    pub base_vertex: i32,
    pub first_index: u32,
    pub index_count: i32,
}
