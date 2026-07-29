use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;

use glam::Vec3;

use super::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk, ChunkPos};
use super::mesher::mesh_chunk;
use super::worldgen::generate_chunk;
use crate::render_thread::RenderCommand;

#[derive(PartialEq, Eq)]
enum ChunkState {
    Loading,
    Loaded,
}

struct MeshTask {
    pos: ChunkPos,
    chunk: Chunk,
}

struct MeshReady {
    pos: ChunkPos,
}

pub struct World {
    chunks: HashMap<ChunkPos, ChunkState>,
    render_distance: i32,
    player_chunk: ChunkPos,
    mesh_tx: Sender<MeshTask>,
    ready_rx: Receiver<MeshReady>,
    render_tx: SyncSender<RenderCommand>,
}

impl World {
    pub fn new(render_distance: i32, render_tx: SyncSender<RenderCommand>) -> Self {
        let (mesh_tx, mesh_rx) = mpsc::channel::<MeshTask>();
        let (ready_tx, ready_rx) = mpsc::channel::<MeshReady>();

        // Single meshing worker. Chunks are moved (not cloned) into the task,
        // meshed, and the resulting geometry is sent straight to the render
        // thread — no round trip through the game thread's FrameData needed,
        // since a chunk is a Scene object (see render_thread::AddChunk), not
        // a per-frame RenderCommand.
        let worker_render_tx = render_tx.clone();
        thread::spawn(move || {
            for task in mesh_rx {
                let mesh = mesh_chunk(&task.chunk, |_, _, _| false);
                // Treating out-of-chunk neighbors as "not solid" means a
                // chunk boundary face always renders — safe (no holes), at
                // the cost of some hidden geometry at seams until real
                // cross-chunk neighbor lookups are added.
                if !mesh.indices.is_empty() {
                    let _ = worker_render_tx.send(RenderCommand::AddChunk {
                        pos: task.pos,
                        mesh,
                    });
                }
                if ready_tx.send(MeshReady { pos: task.pos }).is_err() {
                    break;
                }
            }
        });

        Self {
            chunks: HashMap::new(),
            render_distance,
            // Sentinel forces the first update() to always run the full
            // load pass, regardless of starting position.
            player_chunk: ChunkPos {
                x: i32::MAX,
                z: i32::MAX,
            },
            mesh_tx,
            ready_rx,
            render_tx,
        }
    }

    pub fn update(&mut self, player_pos: Vec3) {
        let px = (player_pos.x / CHUNK_SIZE_X as f32).floor() as i32;
        let pz = (player_pos.z / CHUNK_SIZE_Z as f32).floor() as i32;
        let player_chunk = ChunkPos { x: px, z: pz };

        // Drain finished meshing jobs first — flips Loading -> Loaded so the
        // removal pass below never yanks a chunk out from under an in-flight
        // AddChunk that's still on its way to the render thread.
        while let Ok(MeshReady { pos }) = self.ready_rx.try_recv() {
            self.chunks.insert(pos, ChunkState::Loaded);
        }

        if player_chunk == self.player_chunk {
            return; // Same chunk as last update — nothing to load/unload.
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
            let chunk = generate_chunk(pos);
            let _ = self.mesh_tx.send(MeshTask { pos, chunk });
        }
    }
}
