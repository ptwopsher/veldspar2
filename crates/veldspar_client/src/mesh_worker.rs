use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::renderer::mesh::{build_chunk_mesh, ChunkMeshes, ChunkNeighbors};
use veldspar_shared::block::BlockRegistry;
use veldspar_shared::chunk::ChunkData;
use veldspar_shared::coords::{ChunkPos, CHUNK_SIZE};
use veldspar_shared::lighting::{
    compute_chunk_lighting_with_neighbors, propagate_light_with_neighbors, LightMap,
};

const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;

pub struct MeshRequest {
    pub chunk_pos: ChunkPos,
    pub chunk: ChunkData,
    // Neighbor order: +X, -X, +Y, -Y, +Z, -Z
    pub neighbors: [Option<ChunkData>; 6],
    pub registry: Arc<BlockRegistry>,
    pub world_seed: u64,
    pub version: u64,
    pub lod_level: u8,
}

pub struct MeshWorker {
    pool: ThreadPool,
    completed_rx: Receiver<(ChunkPos, ChunkMeshes, u64)>,
    completed_tx: Sender<(ChunkPos, ChunkMeshes, u64)>,
}

impl MeshWorker {
    pub fn new() -> Self {
        let available = std::thread::available_parallelism()
            .map(|parallelism| parallelism.get())
            .unwrap_or(4);
        let worker_threads = available.saturating_sub(1).max(2).min(8);
        let pool = ThreadPoolBuilder::new()
            .num_threads(worker_threads)
            .thread_name(|index| format!("mesh-worker-{index}"))
            .build()
            .expect("failed to create mesh worker thread pool");
        let (completed_tx, completed_rx) = mpsc::channel();

        Self {
            pool,
            completed_rx,
            completed_tx,
        }
    }

    pub fn submit(&self, request: MeshRequest) {
        let completed_tx = self.completed_tx.clone();
        self.pool.spawn(move || {
            let neighbors = neighbors_from_array(&request.neighbors);
            let neighbor_refs = [
                request.neighbors[0].as_ref(),
                request.neighbors[1].as_ref(),
                request.neighbors[2].as_ref(),
                request.neighbors[3].as_ref(),
                request.neighbors[4].as_ref(),
                request.neighbors[5].as_ref(),
            ];

            let (light_map, emissive_light_map) = if request.lod_level == 0 {
                let mut center_light = compute_chunk_lighting_with_neighbors(
                    &request.chunk,
                    &request.registry,
                    neighbor_refs,
                );
                let neighbor_lights =
                    compute_neighbor_light_maps(&request.chunk, &request.neighbors, &request.registry);
                populate_center_extended_levels(&mut center_light, &neighbor_lights);

                let mut center_emissive = LightMap::new();
                propagate_light_with_neighbors(
                    &mut center_emissive,
                    &request.chunk,
                    &request.registry,
                    neighbor_refs,
                );

                (Some(center_light), Some(center_emissive))
            } else {
                (None, None)
            };
            let meshes = build_chunk_mesh(
                &request.chunk,
                &request.registry,
                &neighbors,
                request.chunk_pos,
                request.world_seed,
                light_map.as_ref(),
                emissive_light_map.as_ref(),
                request.lod_level,
            );
            let _ = completed_tx.send((request.chunk_pos, meshes, request.version));
        });
    }

    pub fn poll(&self) -> Vec<(ChunkPos, ChunkMeshes, u64)> {
        let mut completed = Vec::new();
        while let Ok(result) = self.completed_rx.try_recv() {
            completed.push(result);
        }
        completed
    }
}

fn compute_neighbor_light_maps(
    center_chunk: &ChunkData,
    neighbors: &[Option<ChunkData>; 6],
    registry: &BlockRegistry,
) -> [Option<LightMap>; 6] {
    [
        neighbors[0].as_ref().map(|chunk| {
            compute_chunk_lighting_with_neighbors(
                chunk,
                registry,
                [None, Some(center_chunk), None, None, None, None],
            )
        }),
        neighbors[1].as_ref().map(|chunk| {
            compute_chunk_lighting_with_neighbors(
                chunk,
                registry,
                [Some(center_chunk), None, None, None, None, None],
            )
        }),
        neighbors[2].as_ref().map(|chunk| {
            compute_chunk_lighting_with_neighbors(
                chunk,
                registry,
                [None, None, None, Some(center_chunk), None, None],
            )
        }),
        neighbors[3].as_ref().map(|chunk| {
            compute_chunk_lighting_with_neighbors(
                chunk,
                registry,
                [None, None, Some(center_chunk), None, None, None],
            )
        }),
        neighbors[4].as_ref().map(|chunk| {
            compute_chunk_lighting_with_neighbors(
                chunk,
                registry,
                [None, None, None, None, None, Some(center_chunk)],
            )
        }),
        neighbors[5].as_ref().map(|chunk| {
            compute_chunk_lighting_with_neighbors(
                chunk,
                registry,
                [None, None, None, None, Some(center_chunk), None],
            )
        }),
    ]
}

fn populate_center_extended_levels(center_light: &mut LightMap, neighbors: &[Option<LightMap>; 6]) {
    let edge_range = -1..=CHUNK_SIZE_I32;

    if let Some(neg_x) = neighbors[1].as_ref() {
        for y in edge_range.clone() {
            for z in edge_range.clone() {
                center_light.set_extended(-1, y, z, neg_x.get_i32(CHUNK_SIZE_I32 - 1, y, z));
            }
        }
    }
    if let Some(pos_x) = neighbors[0].as_ref() {
        for y in edge_range.clone() {
            for z in edge_range.clone() {
                center_light.set_extended(CHUNK_SIZE_I32, y, z, pos_x.get_i32(0, y, z));
            }
        }
    }

    if let Some(neg_y) = neighbors[3].as_ref() {
        for x in edge_range.clone() {
            for z in edge_range.clone() {
                center_light.set_extended(x, -1, z, neg_y.get_i32(x, CHUNK_SIZE_I32 - 1, z));
            }
        }
    }
    if let Some(pos_y) = neighbors[2].as_ref() {
        for x in edge_range.clone() {
            for z in edge_range.clone() {
                center_light.set_extended(x, CHUNK_SIZE_I32, z, pos_y.get_i32(x, 0, z));
            }
        }
    }

    if let Some(neg_z) = neighbors[5].as_ref() {
        for x in edge_range.clone() {
            for y in edge_range.clone() {
                center_light.set_extended(x, y, -1, neg_z.get_i32(x, y, CHUNK_SIZE_I32 - 1));
            }
        }
    }
    if let Some(pos_z) = neighbors[4].as_ref() {
        for x in edge_range.clone() {
            for y in edge_range.clone() {
                center_light.set_extended(x, y, CHUNK_SIZE_I32, pos_z.get_i32(x, y, 0));
            }
        }
    }
}

pub fn neighbors_from_array(arr: &[Option<ChunkData>; 6]) -> ChunkNeighbors<'_> {
    ChunkNeighbors {
        pos_x: arr[0].as_ref(),
        neg_x: arr[1].as_ref(),
        pos_y: arr[2].as_ref(),
        neg_y: arr[3].as_ref(),
        pos_z: arr[4].as_ref(),
        neg_z: arr[5].as_ref(),
    }
}
