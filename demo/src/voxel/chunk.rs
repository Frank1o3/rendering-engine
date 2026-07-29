// src/voxel/chunk.rs

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
