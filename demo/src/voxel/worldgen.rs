// demo/src/voxel/worldgen.rs
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};

use super::block::BlockId;
use super::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk, ChunkPos};

const SEA_LEVEL: i32 = 42;
const BASE_HEIGHT: f32 = 64.0;

fn create_noise(seed: i32) -> FastNoiseLite {
    let mut noise = FastNoiseLite::with_seed(seed);
    noise.set_noise_type(Some(NoiseType::OpenSimplex2));
    noise.set_fractal_type(Some(FractalType::FBm));
    noise.set_fractal_octaves(Some(5));
    noise.set_fractal_lacunarity(Some(2.0));
    noise.set_fractal_gain(Some(0.5));
    noise
}

fn height_at(
    continent: &FastNoiseLite,
    ridge: &FastNoiseLite,
    hills: &FastNoiseLite,
    detail: &FastNoiseLite,
    world_x: i32,
    world_z: i32,
) -> i32 {
    let x = world_x as f32;
    let z = world_z as f32;

    // Large-scale landmass shape (ocean vs. mountainous regions). Kept
    // low-amplitude on purpose — it should decide WHERE mountains go, not
    // BE the visible terrain shape (that was the old bug).
    let continents = continent.get_noise_2d(x * 0.015, z * 0.015);

    // Ridged noise: 1 - |noise|, squared to sharpen peaks — gives actual
    // mountain ridgelines instead of smooth rolling bumps.
    let ridge_raw = ridge.get_noise_2d(x * 0.06, z * 0.06);
    let ridge_shape = (1.0 - ridge_raw.abs()).powf(2.0);

    // Mid-frequency hills — completes a cycle roughly every ~50 blocks,
    // which is the scale that actually reads as "terrain" within normal
    // render distance.
    let hill_noise = hills.get_noise_2d(x * 0.02, z * 0.02);

    // Small surface roughness.
    let detail_noise = detail.get_noise_2d(x * 0.08, z * 0.08);

    // Mountains only rise where continents is already high, so oceans and
    // plains stay clear of ridge noise bleeding in.
    let mountain_mask = (continents * 1.6).clamp(0.0, 1.0);
    let mountains = ridge_shape * mountain_mask * 55.0;

    let height =
        BASE_HEIGHT + continents * 18.0 + hill_noise * 14.0 + mountains + detail_noise * 3.0;

    height.round() as i32
}

pub fn generate_chunk(pos: ChunkPos) -> Chunk {
    let continent = create_noise(1337);
    let ridge = create_noise(4242);
    let hills = create_noise(9001);
    let detail = create_noise(777);

    let mut chunk = Chunk::new(pos);
    let (origin_x, _, origin_z) = chunk.world_origin();

    for lx in 0..CHUNK_SIZE_X {
        for lz in 0..CHUNK_SIZE_Z {
            let world_x = origin_x + lx as i32;
            let world_z = origin_z + lz as i32;

            let surface =
                height_at(&continent, &ridge, &hills, &detail, world_x, world_z).clamp(1, 250);

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
