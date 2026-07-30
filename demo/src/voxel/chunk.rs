// src/voxel/chunk.rs

use glam::{Vec3, Vec4};

use super::block::BlockId;

pub const CHUNK_SIZE_X: usize = 16;
pub const CHUNK_SIZE_Z: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;
const BLOCKS_PER_CHUNK: usize = CHUNK_SIZE_X * CHUNK_SIZE_Z * CHUNK_HEIGHT;

/// Chunk-space coordinate (chunk units, not blocks). Chunk (1, 0) covers
/// world blocks x in [16,32), z in [0,16), full Y range — tall-column
/// (Minecraft-style) chunking.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    /// World-space origin of this chunk's (0,0,0) corner.
    /// Mesh vertices from `mesh_chunk` are chunk-local, so this is what
    /// positions the chunk's whole-mesh Scene object correctly.
    pub fn world_origin(&self) -> Vec3 {
        Vec3::new(
            (self.x * CHUNK_SIZE_X as i32) as f32,
            0.0,
            (self.z * CHUNK_SIZE_Z as i32) as f32,
        )
    }
    /// World-space (min, max) corners of this chunk's full-height column.
    pub fn aabb(&self) -> (Vec3, Vec3) {
        let min = self.world_origin();
        let max = min
            + Vec3::new(
                CHUNK_SIZE_X as f32,
                CHUNK_HEIGHT as f32,
                CHUNK_SIZE_Z as f32,
            );
        (min, max)
    }

    /// AABB-vs-frustum-planes visibility test. Single source of truth so
    /// `World`'s load prioritization and the render thread's GPU
    /// promote/evict decisions never disagree about what's "in view."
    pub fn is_visible(&self, planes: &[Vec4; 6]) -> bool {
        let (min, max) = self.aabb();
        for plane in planes {
            let normal = plane.truncate();
            let d = plane.w;
            let p = Vec3::new(
                if normal.x > 0.0 { max.x } else { min.x },
                if normal.y > 0.0 { max.y } else { min.y },
                if normal.z > 0.0 { max.z } else { min.z },
            );
            if normal.dot(p) + d < 0.0 {
                return false;
            }
        }
        true
    }
}

/// A single 16×256×16 block column. Flat `Vec<BlockId>` storage for now —
/// 65,536 blocks per chunk even where mostly air. Worth revisiting
/// (paletted/RLE compression) once real memory pressure from many loaded
/// chunks shows up; not worth the complexity before that's measured.
pub struct Chunk {
    pub pos: ChunkPos,
    blocks: Vec<BlockId>,
}

impl Chunk {
    pub fn new(pos: ChunkPos) -> Self {
        Self {
            pos,
            blocks: vec![BlockId::AIR; BLOCKS_PER_CHUNK],
        }
    }

    #[inline]
    fn index(x: usize, y: usize, z: usize) -> usize {
        (y * CHUNK_SIZE_Z + z) * CHUNK_SIZE_X + x
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockId {
        self.blocks[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: BlockId) {
        self.blocks[Self::index(x, y, z)] = block;
    }

    /// Bounds-checked lookup with signed, possibly out-of-range coordinates
    /// — used by the mesher probing a block's neighbor, which can fall just
    /// outside this chunk. `None` means out-of-bounds, not "air".
    #[inline]
    pub fn get_checked(&self, x: i32, y: i32, z: i32) -> Option<BlockId> {
        if x < 0 || y < 0 || z < 0 {
            return None;
        }
        let (x, y, z) = (x as usize, y as usize, z as usize);
        if x >= CHUNK_SIZE_X || y >= CHUNK_HEIGHT || z >= CHUNK_SIZE_Z {
            return None;
        }
        Some(self.get(x, y, z))
    }

    /// World-space block coordinate of this chunk's (0,0,0) corner.
    pub fn world_origin(&self) -> (i32, i32, i32) {
        (
            self.pos.x * CHUNK_SIZE_X as i32,
            0,
            self.pos.z * CHUNK_SIZE_Z as i32,
        )
    }
}
