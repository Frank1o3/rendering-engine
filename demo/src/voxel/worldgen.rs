// src/voxel/worldgen.rs
//
// Placeholder terrain generator — deterministic, dependency-free layered
// sine/cosine heightmap. Enough to produce non-flat terrain and verify
// meshing/rendering; swap for a proper noise crate when world-gen becomes
// its own phase (see roadmap — disk persistence phase).

use super::block::BlockId;
use super::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk, ChunkPos};

fn height_at(world_x: i32, world_z: i32) -> i32 {
    let x = world_x as f32;
    let z = world_z as f32;
    let h = 40.0 + 6.0 * (x * 0.05).sin() * (z * 0.05).cos() + 3.0 * (x * 0.13 + z * 0.09).sin();
    h.round() as i32
}

pub fn generate_chunk(pos: ChunkPos) -> Chunk {
    let mut chunk = Chunk::new(pos);
    let (origin_x, _, origin_z) = chunk.world_origin();

    for lx in 0..CHUNK_SIZE_X {
        for lz in 0..CHUNK_SIZE_Z {
            let world_x = origin_x + lx as i32;
            let world_z = origin_z + lz as i32;
            let surface = height_at(world_x, world_z).clamp(1, 250);

            for y in 0..surface {
                let block = if y == surface - 1 {
                    BlockId::GRASS
                } else if y >= surface - 4 {
                    BlockId::DIRT
                } else {
                    BlockId::STONE
                };
                chunk.set(lx, y as usize, lz, block);
            }
        }
    }

    chunk
}
