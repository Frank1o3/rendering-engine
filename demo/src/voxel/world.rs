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
}

impl World {
    pub fn new(render_distance: i32, render_tx: SyncSender<RenderCommand>) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<ChunkPos>();
        let (ready_tx, ready_rx) = mpsc::channel::<MeshReady>();
        let task_rx = Arc::new(Mutex::new(task_rx));

        // Each worker fully owns a chunk end-to-end (generate + greedy-mesh)
        // and never touches another chunk's data, so this scales cleanly
        // with core count — no locking needed except the brief task-queue pop.
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
                            Err(_) => break, // World dropped
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
        }
    }

    pub fn update(&mut self, player_pos: Vec3) {
        let px = (player_pos.x / CHUNK_SIZE_X as f32).floor() as i32;
        let pz = (player_pos.z / CHUNK_SIZE_Z as f32).floor() as i32;
        let player_chunk = ChunkPos { x: px, z: pz };

        while let Ok(MeshReady { pos }) = self.ready_rx.try_recv() {
            self.chunks.insert(pos, ChunkState::Loaded);
        }

        if player_chunk == self.player_chunk {
            return;
        }
        self.player_chunk = player_chunk;

        let rd = self.render_distance;
        let mut needed = HashSet::with_capacity(((2 * rd + 1) * (2 * rd + 1)) as usize);
        for dx in -rd..=rd {
            for dz in -rd..=rd {
                needed.insert(ChunkPos {
                    x: px + dx,
                    z: pz + dz,
                });
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

        for &pos in &needed {
            if self.chunks.contains_key(&pos) {
                continue;
            }
            self.chunks.insert(pos, ChunkState::Loading);
            let _ = self.task_tx.send(pos);
        }
    }
}
