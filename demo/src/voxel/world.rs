use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

use glam::{Vec3, Vec4};

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
    center: ChunkPos,
    rd: i32,
    side: i32,
    index: i32,
    total: i32,
    needed: HashSet<ChunkPos>,
    visible: Vec<ChunkPos>,
    rest: Vec<ChunkPos>,
}

pub struct World {
    chunks: HashMap<ChunkPos, ChunkState>,
    render_distance: i32,
    player_chunk: ChunkPos,
    task_tx: Sender<ChunkPos>,
    ready_rx: Receiver<MeshReady>,
    render_tx: SyncSender<RenderCommand>,
    max_load_per_frame: usize,
    load_queue: VecDeque<ChunkPos>,
    last_forward: Vec3,
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

                    let mesh = mesh_chunk(&chunk, |x, y, z| {
                        gens.block_at(origin_x + x, y, origin_z + z).is_solid()
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
            max_load_per_frame: 4,
            load_queue: VecDeque::new(),
            last_forward: Vec3::NEG_Z,
            pending_rebuild: None,
        }
    }

    pub fn update(&mut self, player_pos: Vec3, camera_forward: Vec3, frustum_planes: &[Vec4; 6]) {
        let px = (player_pos.x / CHUNK_SIZE_X as f32).floor() as i32;
        let pz = (player_pos.z / CHUNK_SIZE_Z as f32).floor() as i32;
        let player_chunk = ChunkPos { x: px, z: pz };

        while let Ok(MeshReady { pos }) = self.ready_rx.try_recv() {
            self.chunks.insert(pos, ChunkState::Loaded);
        }

        if player_chunk != self.player_chunk {
            // Crossed a chunk boundary — (re)start an incremental rebuild.
            // If one was already mid-flight from a previous crossing, this
            // just restarts it centered on the new position; that's fine,
            // the old partial state is discarded rather than finished.
            self.player_chunk = player_chunk;
            self.start_rebuild(player_chunk);
            self.last_forward = camera_forward;
        }

        if self.pending_rebuild.is_some() {
            self.step_rebuild(frustum_planes, REBUILD_CELLS_PER_FRAME);
        } else if self.last_forward.dot(camera_forward) < 0.95 {
            self.reprioritize_queue(player_chunk, frustum_planes);
            self.last_forward = camera_forward;
        }

        let loading_count = self
            .chunks
            .values()
            .filter(|s| **s == ChunkState::Loading)
            .count();
        let mut can_start = self.max_load_per_frame.saturating_sub(loading_count);

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

    /// Begins a fresh incremental rebuild. Doesn't touch `load_queue` or
    /// send any removals yet — those only happen once `step_rebuild`
    /// finishes the full scan, so we know the complete needed-set before
    /// deciding what's actually stale.
    fn start_rebuild(&mut self, center: ChunkPos) {
        let rd = self.render_distance;
        let side = 2 * rd + 1;
        let total = side * side;

        self.pending_rebuild = Some(RebuildState {
            center,
            rd,
            side,
            index: 0,
            total,
            needed: HashSet::with_capacity((total) as usize),
            visible: Vec::new(),
            rest: Vec::new(),
        });
    }

    /// Processes up to `budget` grid cells of the in-progress rebuild.
    /// When it finishes the full scan, computes stale chunks, sends one
    /// batched `RemoveChunks` (instead of one send per chunk — the thing
    /// that was blocking the main thread on a bursty boundary crossing),
    /// and replaces `load_queue` with the freshly sorted result.
    fn step_rebuild(&mut self, frustum_planes: &[Vec4; 6], budget: i32) {
        let Some(state) = self.pending_rebuild.as_mut() else {
            return;
        };

        let mut processed = 0;
        while state.index < state.total && processed < budget {
            let dx = state.index % state.side - state.rd;
            let dz = state.index / state.side - state.rd;
            let pos = ChunkPos {
                x: state.center.x + dx,
                z: state.center.z + dz,
            };

            state.needed.insert(pos);

            if !self.chunks.contains_key(&pos) {
                if pos.is_visible(frustum_planes) {
                    state.visible.push(pos);
                } else {
                    state.rest.push(pos);
                }
            }

            state.index += 1;
            processed += 1;
        }

        if state.index < state.total {
            return; // more to do next frame
        }

        // Scan complete — finalize.
        let state = self.pending_rebuild.take().unwrap();
        let center = state.center;

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

        let mut visible = state.visible;
        let mut rest = state.rest;
        let dist2 = |p: &ChunkPos| (p.x - center.x).pow(2) + (p.z - center.z).pow(2);
        visible.sort_by_key(dist2);
        rest.sort_by_key(dist2);

        self.load_queue.clear();
        self.load_queue.extend(visible);
        self.load_queue.extend(rest);
    }

    /// Cheap re-sort of the *existing* queue by current visibility/distance.
    /// Unchanged from before — this one was never the problem.
    fn reprioritize_queue(&mut self, center: ChunkPos, frustum_planes: &[Vec4; 6]) {
        let mut visible = Vec::new();
        let mut rest = Vec::new();

        for pos in self.load_queue.drain(..) {
            if pos.is_visible(frustum_planes) {
                visible.push(pos);
            } else {
                rest.push(pos);
            }
        }

        let dist2 = |p: &ChunkPos| (p.x - center.x).pow(2) + (p.z - center.z).pow(2);
        visible.sort_by_key(dist2);
        rest.sort_by_key(dist2);

        self.load_queue.extend(visible);
        self.load_queue.extend(rest);
    }
}
