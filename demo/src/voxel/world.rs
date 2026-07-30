// demo/src/voxel/world.rs
use std::collections::{HashMap, HashSet};
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
    // Limit how many new chunks we start per frame to avoid CPU spikes.
    max_load_per_frame: usize,
}

impl World {
    pub fn new(render_distance: i32, render_tx: SyncSender<RenderCommand>) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<ChunkPos>();
        let (ready_tx, ready_rx) = mpsc::channel::<MeshReady>();
        let task_rx = Arc::new(Mutex::new(task_rx));

        let worker_count = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(2, 8);

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
            max_load_per_frame: 4, // tune this value
        }
    }

    /// Update the set of loaded chunks.
    /// - Keeps all chunks within `render_distance`.
    /// - Removes only those outside the distance.
    /// - Prioritises loading chunks inside the view frustum, but limits new tasks per frame.
    pub fn update(&mut self, player_pos: Vec3, frustum_planes: &[glam::Vec4; 6]) {
        let px = (player_pos.x / CHUNK_SIZE_X as f32).floor() as i32;
        let pz = (player_pos.z / CHUNK_SIZE_Z as f32).floor() as i32;
        let player_chunk = ChunkPos { x: px, z: pz };

        // Collect finished meshes.
        while let Ok(MeshReady { pos }) = self.ready_rx.try_recv() {
            self.chunks.insert(pos, ChunkState::Loaded);
        }

        // Only recompute when the player moves to a new chunk.
        if player_chunk == self.player_chunk {
            return;
        }
        self.player_chunk = player_chunk;

        let rd = self.render_distance;
        let mut needed = HashSet::with_capacity(((2 * rd + 1) * (2 * rd + 1)) as usize);
        // Visible set is a subset of needed; we use it for prioritisation.
        let mut visible = HashSet::new();

        for dx in -rd..=rd {
            for dz in -rd..=rd {
                let pos = ChunkPos {
                    x: px + dx,
                    z: pz + dz,
                };
                needed.insert(pos);
                if Self::is_chunk_visible(pos, frustum_planes) {
                    visible.insert(pos);
                }
            }
        }

        // Remove chunks that are now out of render distance.
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

        // Count how many chunks are currently loading (to respect the per-frame limit).
        let loading_count = self
            .chunks
            .values()
            .filter(|s| **s == ChunkState::Loading)
            .count();
        let mut can_start = self.max_load_per_frame.saturating_sub(loading_count);

        if can_start == 0 {
            return;
        }

        // First, try to start visible chunks that are not yet loaded.
        for &pos in &visible {
            if can_start == 0 {
                break;
            }
            if self.chunks.contains_key(&pos) {
                continue;
            }
            self.chunks.insert(pos, ChunkState::Loading);
            let _ = self.task_tx.send(pos);
            can_start -= 1;
        }

        // If we still have capacity, start any other needed chunks (outside frustum).
        if can_start > 0 {
            for &pos in &needed {
                if can_start == 0 {
                    break;
                }
                if visible.contains(&pos) {
                    continue;
                } // already handled
                if self.chunks.contains_key(&pos) {
                    continue;
                }
                self.chunks.insert(pos, ChunkState::Loading);
                let _ = self.task_tx.send(pos);
                can_start -= 1;
            }
        }
    }

    /// AABB‑frustum test for a chunk (16×256×16 box).
    fn is_chunk_visible(pos: ChunkPos, planes: &[glam::Vec4; 6]) -> bool {
        let min = Vec3::new(
            (pos.x * CHUNK_SIZE_X as i32) as f32,
            0.0,
            (pos.z * CHUNK_SIZE_Z as i32) as f32,
        );
        let max = min + Vec3::new(CHUNK_SIZE_X as f32, 256.0, CHUNK_SIZE_Z as f32);

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
