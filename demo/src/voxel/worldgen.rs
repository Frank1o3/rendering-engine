// src/voxel/worldgen.rs

use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};

use super::block::BlockId;
use super::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk, ChunkPos};

const SEA_LEVEL: i32 = 42;
const BASE_HEIGHT: f32 = 44.0;

fn create_noise() -> FastNoiseLite {
    let mut noise = FastNoiseLite::with_seed(1337);

    noise.set_noise_type(Some(NoiseType::OpenSimplex2));

    noise.set_fractal_type(Some(FractalType::FBm));
    noise.set_fractal_octaves(Some(5));
    noise.set_fractal_lacunarity(Some(2.0));
    noise.set_fractal_gain(Some(0.5));

    noise
}

fn height_at(noise: &FastNoiseLite, world_x: i32, world_z: i32) -> i32 {
    let x = world_x as f32;
    let z = world_z as f32;

    // Very large terrain features
    let continents = noise.get_noise_2d(x * 0.002, z * 0.002);

    // Rolling hills
    let hills = noise.get_noise_2d(x * 0.01, z * 0.01);

    // Small terrain detail
    let detail = noise.get_noise_2d(x * 0.05, z * 0.05);

    // Only allow rough terrain on higher elevations
    let mountain_strength = continents.max(0.0);

    let height =
        BASE_HEIGHT + continents * 30.0 + hills * (8.0 + mountain_strength * 14.0) + detail * 2.5;

    height.round() as i32
}

pub fn generate_chunk(pos: ChunkPos) -> Chunk {
    let noise = create_noise();

    let mut chunk = Chunk::new(pos);
    let (origin_x, _, origin_z) = chunk.world_origin();

    for lx in 0..CHUNK_SIZE_X {
        for lz in 0..CHUNK_SIZE_Z {
            let world_x = origin_x + lx as i32;
            let world_z = origin_z + lz as i32;

            let surface = height_at(&noise, world_x, world_z).clamp(1, 250);

            for y in 0..surface {
                let block = if y == surface - 1 {
                    if surface <= SEA_LEVEL {
                        BlockId::DIRT
                    } else {
                        BlockId::GRASS
                    }
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
