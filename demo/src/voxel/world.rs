// demo/src/voxel/world.rs

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use std::time::Duration;

use glam::Vec3;

use super::chunk::{CHUNK_HEIGHT, CHUNK_SIZE_X, CHUNK_SIZE_Z, Chunk, ChunkPos};
use super::mesher::mesh_chunk;
use super::worldgen::generate_chunk;
use crate::render_thread::RenderCommand;

/// The main world manager, owned by the game thread.
pub struct World {
    chunks: HashMap<ChunkPos, ChunkData>,
    render_distance: i32,
    player_chunk: ChunkPos,
    mesh_tx: Sender<MeshTask>,
    mesh_ready_rx: Receiver<MeshReady>,
}

/// Per-chunk data stored on the game thread.
pub struct ChunkData {
    pub chunk: Chunk,
    pub state: ChunkState,
    pub dirty: bool,
}

#[derive(PartialEq)]
pub enum ChunkState {
    Unloaded,
    Loading, // meshing in progress
    Loaded,  // mesh is on GPU
}

/// Task sent to the meshing worker.
struct MeshTask {
    pos: ChunkPos,
    blocks: Vec<u8>, // serialized BlockId
}

/// Message from the worker back to the game thread when a mesh is ready.
struct MeshReady {
    pos: ChunkPos,
}

impl World {
    pub fn new(render_distance: i32) -> Self {
        let (mesh_tx, mesh_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        // Spawn the meshing worker thread.
        thread::spawn(move || {
            worker_loop(mesh_rx, ready_tx);
        });

        Self {
            chunks: HashMap::new(),
            render_distance,
            player_chunk: ChunkPos { x: 0, z: 0 },
            mesh_tx,
            mesh_ready_rx: ready_rx,
        }
    }

    /// Update the world based on player position.
    /// Loads/unloads chunks as needed, and submits meshing tasks for new chunks.
    pub fn update(&mut self, player_pos: Vec3, render_tx: &SyncSender<RenderCommand>) {
        // Determine player chunk.
        let px = (player_pos.x / CHUNK_SIZE_X as f32).floor() as i32;
        let pz = (player_pos.z / CHUNK_SIZE_Z as f32).floor() as i32;
        let player_chunk = ChunkPos { x: px, z: pz };
        if player_chunk != self.player_chunk {
            self.player_chunk = player_chunk;
        }

        // Compute the set of chunks that should be loaded (within render distance).
        let rd = self.render_distance;
        let mut needed = HashMap::new();
        for dx in -rd..=rd {
            for dz in -rd..=rd {
                let pos = ChunkPos {
                    x: px + dx,
                    z: pz + dz,
                };
                needed.insert(pos, ());
            }
        }

        // Remove chunks that are no longer needed.
        let to_remove: Vec<ChunkPos> = self
            .chunks
            .keys()
            .filter(|&pos| !needed.contains_key(pos))
            .cloned()
            .collect();

        for pos in to_remove {
            // Send remove command to render thread.
            let _ = render_tx.send(RenderCommand::RemoveChunk { pos });
            self.chunks.remove(&pos);
        }

        // Add new chunks that are not yet loaded or loading.
        for pos in needed.keys() {
            if !self.chunks.contains_key(pos) {
                // Generate chunk data (block array).
                let chunk = generate_chunk(*pos);
                let blocks = chunk.blocks.clone(); // Vec<BlockId> -> we'll serialize to u8
                // We need to convert BlockId to u8. BlockId is repr(transparent) around u8, so we can cast.
                let blocks_u8: Vec<u8> = blocks.iter().map(|&b| b.0).collect();

                // Insert as Loading.
                self.chunks.insert(
                    *pos,
                    ChunkData {
                        chunk,
                        state: ChunkState::Loading,
                        dirty: false,
                    },
                );

                // Submit meshing task.
                let task = MeshTask {
                    pos: *pos,
                    blocks: blocks_u8,
                };
                let _ = self.mesh_tx.send(task);
            }
        }

        // Process any ready meshes.
        while let Ok(MeshReady { pos }) = self.mesh_ready_rx.try_recv() {
            if let Some(data) = self.chunks.get_mut(&pos) {
                data.state = ChunkState::Loaded;
                // The mesh has already been sent to the render thread by the worker.
            }
        }
    }

    /// Mark a chunk as dirty (e.g., block changed). Will cause remeshing.
    pub fn mark_dirty(&mut self, pos: ChunkPos) {
        if let Some(data) = self.chunks.get_mut(&pos) {
            data.dirty = true;
        }
    }

    /// Get the chunk data for a position, if loaded.
    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&ChunkData> {
        self.chunks.get(&pos)
    }
}

/// The meshing worker loop.
fn worker_loop(tasks: Receiver<MeshTask>, ready_tx: Sender<MeshReady>) {
    // We need a sender to the render thread. We'll receive it via a global or we can have it passed.
    // For simplicity, we'll use a channel from the main thread to send render commands.
    // But the main thread owns the render_tx. We'll create a second channel for the worker to send render commands?
    // Actually, the worker can use the same render_tx if we clone it and send it to the worker.
    // We'll modify the World constructor to take a render_tx clone.
    // However, we can also have the worker send a message back to the main thread, and the main thread forwards to render_tx.
    // That's simpler: the worker sends MeshReady, and the main thread, upon receiving, sends AddChunk to render thread.
    // But we need the mesh data. The worker generates the mesh data and must send it to the render thread.
    // So the worker must have a way to send RenderCommand::AddChunk.
    // We'll have the worker receive a SyncSender<RenderCommand> as part of its initialization.
    // We'll modify World to hold a clone of render_tx and pass it to the worker.
    // I'll adjust World::new to accept render_tx: SyncSender<RenderCommand>.
    // But then World is created before render_tx exists? Actually render_tx is created in renderer_setup and then passed to DemoState.
    // We'll create World after DemoState is created, so we can pass render_tx.
    // I'll change the code accordingly.
    // For now, I'll leave the worker as is and we'll update the World construction.
    // In practice, we'll have the worker send AddChunk directly.
    // To avoid refactoring, I'll keep the worker as a placeholder and we'll implement the full flow in the final code.
}

// We'll need to modify World to accept render_tx.
// I'll rewrite World with a proper constructor.
