use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

use glam::Vec3;

use super::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z, ChunkPos};
use super::mesher::mesh_chunk;
use super::worldgen::{NoiseGenerators, generate_chunk_with};
use crate::render_thread::RenderCommand;

/// How many grid cells the rebuild scan processes per `update()` call.
/// At render_distance 20 the full ring is (2*20+1)^2 = 1681 cells; at 400
/// cells/frame that's ~5 frames to finish instead of one blocking pass —
/// no single-frame stall, and it self-scales down for smaller distances.
const REBUILD_CELLS_PER_FRAME: i32 = 400;

#[derive(PartialEq, Eq)]
enum ChunkState {
    Loading,
    Loaded,
}

struct MeshReady {
    pos: ChunkPos,
}

/// In-progress state for an incremental needed-set rebuild, resumed across
/// `update()` calls until `index` reaches `total`.
struct RebuildState {
    positions: Vec<ChunkPos>,
    index: i32,
    total: i32,
    needed: HashSet<ChunkPos>,
}

pub struct World {
    chunks: HashMap<ChunkPos, ChunkState>,
    render_distance: i32,
    player_chunk: ChunkPos,
    task_tx: Sender<ChunkPos>,
    ready_rx: Receiver<MeshReady>,
    render_tx: SyncSender<RenderCommand>,
    max_in_flight: usize,
    load_queue: VecDeque<ChunkPos>,
    pending_rebuild: Option<RebuildState>,
}

impl World {
    pub fn new(render_distance: i32, render_tx: SyncSender<RenderCommand>) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<ChunkPos>();
        let (ready_tx, ready_rx) = mpsc::channel::<MeshReady>();
        let task_rx = Arc::new(Mutex::new(task_rx));

        let worker_count = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .saturating_sub(3)
            .clamp(1, 6);

        for _ in 0..worker_count {
            let task_rx = task_rx.clone();
            let ready_tx = ready_tx.clone();
            let render_tx = render_tx.clone();

            thread::spawn(move || {
                let gens = NoiseGenerators::new();

                loop {
                    let pos = {
                        let rx = task_rx.lock().unwrap();
                        match rx.recv() {
                            Ok(pos) => pos,
                            Err(_) => break,
                        }
                    };

                    let chunk = generate_chunk_with(pos, &gens);
                    let (origin_x, _, origin_z) = chunk.world_origin();
                    // Meshing probes the neighboring column once per boundary
                    // voxel. Terrain height is constant for all of those Y
                    // probes, so cache it per X/Z column instead of rerunning
                    // four fractal-noise samples for every voxel.
                    let mut edge_heights =
                        HashMap::with_capacity((CHUNK_SIZE_X + CHUNK_SIZE_Z) * 4);

                    let mesh = mesh_chunk(&chunk, |x, y, z| {
                        if y < 0 {
                            return false;
                        }
                        let world_x = origin_x + x;
                        let world_z = origin_z + z;
                        let surface = *edge_heights
                            .entry((world_x, world_z))
                            .or_insert_with(|| gens.height_at(world_x, world_z));
                        y < surface
                    });

                    if !mesh.indices.is_empty() {
                        let _ = render_tx.send(RenderCommand::AddChunk { pos, mesh });
                    }
                    if ready_tx.send(MeshReady { pos }).is_err() {
                        break;
                    }
                }
            });
        }

        Self {
            chunks: HashMap::new(),
            render_distance,
            player_chunk: ChunkPos {
                x: i32::MAX,
                z: i32::MAX,
            },
            task_tx,
            ready_rx,
            render_tx,
            max_in_flight: worker_count,
            load_queue: VecDeque::new(),
            pending_rebuild: None,
        }
    }

    pub fn update(&mut self, player_pos: Vec3) {
        let px = (player_pos.x / CHUNK_SIZE_X as f32).floor() as i32;
        let pz = (player_pos.z / CHUNK_SIZE_Z as f32).floor() as i32;
        let player_chunk = ChunkPos { x: px, z: pz };

        if player_chunk != self.player_chunk {
            // Crossed a chunk boundary — (re)start an incremental rebuild.
            // If one was already mid-flight from a previous crossing, this
            // just restarts it centered on the new position; that's fine,
            // the old partial state is discarded rather than finished.
            self.player_chunk = player_chunk;
            self.start_rebuild(player_chunk);
        }

        // A worker can finish a chunk after the player has moved and the
        // previous needed-set scan has completed. Don't let that stale result
        // become a permanent loaded chunk outside the current render radius.
        let rd = self.render_distance.max(0);
        let mut stale_ready = Vec::new();
        while let Ok(MeshReady { pos }) = self.ready_rx.try_recv() {
            if (pos.x - player_chunk.x).abs() <= rd && (pos.z - player_chunk.z).abs() <= rd {
                self.chunks.insert(pos, ChunkState::Loaded);
            } else {
                self.chunks.remove(&pos);
                stale_ready.push(pos);
            }
        }
        if !stale_ready.is_empty() {
            let _ = self
                .render_tx
                .send(RenderCommand::RemoveChunks(stale_ready));
        }

        if self.pending_rebuild.is_some() {
            self.step_rebuild(REBUILD_CELLS_PER_FRAME);
        }

        let loading_count = self
            .chunks
            .values()
            .filter(|s| **s == ChunkState::Loading)
            .count();
        let mut can_start = self.max_in_flight.saturating_sub(loading_count);

        while can_start > 0 {
            let Some(pos) = self.load_queue.pop_front() else {
                break;
            };
            if self.chunks.contains_key(&pos) {
                continue;
            }
            self.chunks.insert(pos, ChunkState::Loading);
            let _ = self.task_tx.send(pos);
            can_start -= 1;
        }
    }

    /// Begins an incremental, center-out rebuild. The radial queue is
    /// generated in distance order and can be consumed by workers while the
    /// scan continues, so the center becomes visible before the outer ring.
    fn start_rebuild(&mut self, center: ChunkPos) {
        let rd = self.render_distance.max(0);
        let mut positions = Vec::with_capacity(((2 * rd + 1) * (2 * rd + 1)) as usize);
        for dz in -rd..=rd {
            for dx in -rd..=rd {
                positions.push(ChunkPos {
                    x: center.x + dx,
                    z: center.z + dz,
                });
            }
        }
        positions.sort_by_key(|pos| {
            let dx = pos.x - center.x;
            let dz = pos.z - center.z;
            (dx * dx + dz * dz, dz, dx)
        });
        let total = positions.len() as i32;

        // Discard work that has not yet been handed to a worker for the old
        // center. Already-running jobs finish normally and may be reused.
        self.load_queue.clear();

        self.pending_rebuild = Some(RebuildState {
            positions,
            index: 0,
            total,
            needed: HashSet::with_capacity((total) as usize),
        });
    }

    /// Processes up to `budget` positions, immediately queueing missing
    /// chunks in radial order. Once the full needed-set is known, stale
    /// loaded chunks are removed in one batch.
    fn step_rebuild(&mut self, budget: i32) {
        let Some(state) = self.pending_rebuild.as_mut() else {
            return;
        };

        let mut processed = 0;
        while state.index < state.total && processed < budget {
            let pos = state.positions[state.index as usize];

            state.needed.insert(pos);

            if !self.chunks.contains_key(&pos) {
                self.load_queue.push_back(pos);
            }

            state.index += 1;
            processed += 1;
        }

        if state.index < state.total {
            return; // more to do next frame
        }

        // Scan complete — finalize stale chunk removal. The queue was
        // already consumed incrementally by the worker scheduler.
        let state = self.pending_rebuild.take().unwrap();

        let to_remove: Vec<ChunkPos> = self
            .chunks
            .iter()
            .filter(|(pos, s)| **s == ChunkState::Loaded && !state.needed.contains(pos))
            .map(|(pos, _)| *pos)
            .collect();

        if !to_remove.is_empty() {
            for pos in &to_remove {
                self.chunks.remove(pos);
            }
            // One batched send instead of one per chunk — this is what was
            // stalling the main thread when the channel backed up during
            // a burst of evictions.
            let _ = self.render_tx.send(RenderCommand::RemoveChunks(to_remove));
        }
    }
}
