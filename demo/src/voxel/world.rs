use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

use glam::Vec3;

use super::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z, ChunkPos};
use super::mesher::mesh_chunk;
use super::worldgen::generate_chunk;
use crate::render_thread::RenderCommand;

#[derive(PartialEq, Eq)]
enum ChunkState {
    Loading,
    Loaded,
}

struct MeshReady {
    pos: ChunkPos,
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
    last_forward: Vec3, // NEW — baseline to detect "turned enough to matter"
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
                loop {
                    let pos = {
                        let rx = task_rx.lock().unwrap();
                        match rx.recv() {
                            Ok(pos) => pos,
                            Err(_) => break,
                        }
                    };

                    let chunk = generate_chunk(pos);
                    let mesh = mesh_chunk(&chunk, |_, _, _| false);

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
        }
    }

    /// `camera_forward` — normalized view direction. game.rs already
    /// computes this for movement; just pass the same value through.
    pub fn update(
        &mut self,
        player_pos: Vec3,
        camera_forward: Vec3,
        frustum_planes: &[glam::Vec4; 6],
    ) {
        let px = (player_pos.x / CHUNK_SIZE_X as f32).floor() as i32;
        let pz = (player_pos.z / CHUNK_SIZE_Z as f32).floor() as i32;
        let player_chunk = ChunkPos { x: px, z: pz };

        while let Ok(MeshReady { pos }) = self.ready_rx.try_recv() {
            self.chunks.insert(pos, ChunkState::Loaded);
        }

        if player_chunk != self.player_chunk {
            // Moved to a new chunk — full rescan: what's needed, what's now
            // out of range, fresh queue ordering.
            self.player_chunk = player_chunk;
            self.rebuild_needed_and_queue(player_chunk, frustum_planes);
            self.last_forward = camera_forward;
        } else {
            // Cumulative-angle gate: cos(18°) ≈ 0.95. Only reprioritize once
            // you've turned roughly 18°+ since the last check — a slow pan
            // won't resort every single frame, but a real turn responds
            // immediately.
            if self.last_forward.dot(camera_forward) < 0.95 {
                self.reprioritize_queue(player_chunk, frustum_planes);
                self.last_forward = camera_forward;
            }
        }

        // Always dispatch from the queue, every call — independent of
        // whether this tick moved or rotated.
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

    /// Expensive O(render_distance²) pass: recompute the needed set, drop
    /// chunks that fell out of range, rebuild the queue from scratch.
    /// Only called on a chunk-boundary crossing.
    fn rebuild_needed_and_queue(&mut self, center: ChunkPos, frustum_planes: &[glam::Vec4; 6]) {
        let rd = self.render_distance;
        let mut needed = HashSet::with_capacity(((2 * rd + 1) * (2 * rd + 1)) as usize);
        let mut visible = Vec::new();
        let mut rest = Vec::new();

        for dx in -rd..=rd {
            for dz in -rd..=rd {
                let pos = ChunkPos {
                    x: center.x + dx,
                    z: center.z + dz,
                };
                needed.insert(pos);

                if self.chunks.contains_key(&pos) {
                    continue; // already loaded or in flight
                }

                if pos.is_visible(frustum_planes) {
                    visible.push(pos);
                } else {
                    rest.push(pos);
                }
            }
        }

        let to_remove: Vec<ChunkPos> = self
            .chunks
            .iter()
            .filter(|(pos, state)| **state == ChunkState::Loaded && !needed.contains(pos))
            .map(|(pos, _)| *pos)
            .collect();
        for pos in to_remove {
            let _ = self.render_tx.send(RenderCommand::RemoveChunk { pos });
            self.chunks.remove(&pos);
        }

        let dist2 = |p: &ChunkPos| (p.x - center.x).pow(2) + (p.z - center.z).pow(2);
        visible.sort_by_key(dist2);
        rest.sort_by_key(dist2);

        self.load_queue.clear();
        self.load_queue.extend(visible);
        self.load_queue.extend(rest);
    }

    /// Cheap re-sort of the *existing* queue by current visibility/distance.
    /// Doesn't touch the `chunks` map or the needed set — those only change
    /// when you actually move. This is what makes rotation responsive: any
    /// still-unloaded chunk that just entered the frustum jumps to the
    /// front, without redoing the full render-distance scan.
    fn reprioritize_queue(&mut self, center: ChunkPos, frustum_planes: &[glam::Vec4; 6]) {
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
