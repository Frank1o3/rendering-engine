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

fn height_at_raw(
    continent: &FastNoiseLite,
    ridge: &FastNoiseLite,
    hills: &FastNoiseLite,
    detail: &FastNoiseLite,
    world_x: i32,
    world_z: i32,
) -> i32 {
    let x = world_x as f32;
    let z = world_z as f32;

    let continents = continent.get_noise_2d(x * 0.015, z * 0.015);
    let ridge_raw = ridge.get_noise_2d(x * 0.06, z * 0.06);
    let ridge_shape = (1.0 - ridge_raw.abs()).powf(2.0);
    let hill_noise = hills.get_noise_2d(x * 0.02, z * 0.02);
    let detail_noise = detail.get_noise_2d(x * 0.08, z * 0.08);

    let mountain_mask = (continents * 1.6).clamp(0.0, 1.0);
    let mountains = ridge_shape * mountain_mask * 55.0;

    let height =
        BASE_HEIGHT + continents * 18.0 + hill_noise * 14.0 + mountains + detail_noise * 3.0;

    height.round() as i32
}

/// Owns the noise generators so a worker thread can build them once and
/// reuse them both for full chunk generation *and* single-block boundary
/// probes when meshing (see `block_at`). Building `FastNoiseLite` per call
/// (the old per-chunk `create_noise` calls) was wasteful; this amortizes
/// that cost across a worker's whole lifetime.
pub struct NoiseGenerators {
    continent: FastNoiseLite,
    ridge: FastNoiseLite,
    hills: FastNoiseLite,
    detail: FastNoiseLite,
}

impl NoiseGenerators {
    pub fn new() -> Self {
        Self {
            continent: create_noise(1337),
            ridge: create_noise(4242),
            hills: create_noise(9001),
            detail: create_noise(777),
        }
    }

    pub fn height_at(&self, world_x: i32, world_z: i32) -> i32 {
        height_at_raw(
            &self.continent,
            &self.ridge,
            &self.hills,
            &self.detail,
            world_x,
            world_z,
        )
        .clamp(1, 250)
    }

    /// Single-block lookup used for chunk-boundary face culling. Mirrors
    /// the column logic in `generate_chunk_with` exactly, so a chunk's
    /// own blocks and a neighbor-probed block never disagree at the seam.
    pub fn block_at(&self, world_x: i32, world_y: i32, world_z: i32) -> BlockId {
        if world_y < 0 {
            return BlockId::AIR; // nothing below the generated floor
        }

        let surface = self.height_at(world_x, world_z);

        if world_y >= surface {
            BlockId::AIR
        } else if world_y == surface - 1 {
            if surface <= SEA_LEVEL {
                BlockId::DIRT
            } else {
                BlockId::GRASS
            }
        } else if world_y >= surface - 4 {
            BlockId::DIRT
        } else {
            BlockId::STONE
        }
    }
}

impl Default for NoiseGenerators {
    fn default() -> Self {
        Self::new()
    }
}

pub fn generate_chunk_with(pos: ChunkPos, gens: &NoiseGenerators) -> Chunk {
    let mut chunk = Chunk::new(pos);
    let (origin_x, _, origin_z) = chunk.world_origin();

    for lx in 0..CHUNK_SIZE_X {
        for lz in 0..CHUNK_SIZE_Z {
            let world_x = origin_x + lx as i32;
            let world_z = origin_z + lz as i32;

            let surface = gens.height_at(world_x, world_z);

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
