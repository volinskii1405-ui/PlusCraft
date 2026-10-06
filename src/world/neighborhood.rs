//! Снимок 3×3 чанков для фоновых задач (свет, меширование).

use std::sync::Arc;

use super::block::{id, make};
use super::chunk::{idx, ChunkData, ChunkPos, CHUNK_H, CHUNK_W};

pub struct Neighborhood {
    pub center: ChunkPos,
    /// [dz+1][dx+1]
    pub chunks: [[Option<Arc<ChunkData>>; 3]; 3],
}

impl Neighborhood {
    /// x, z — в координатах центрального чанка (от -16 до 31).
    #[inline(always)]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        if y < 0 {
            return make(id::BEDROCK, 0);
        }
        if y >= CHUNK_H as i32 {
            return 0;
        }
        let cx = (x + CHUNK_W as i32) >> 4; // 0..2
        let cz = (z + CHUNK_W as i32) >> 4;
        if !(0..3).contains(&cx) || !(0..3).contains(&cz) {
            return 0;
        }
        match &self.chunks[cz as usize][cx as usize] {
            Some(c) => c.blocks[idx((x & 15) as usize, y as usize, (z & 15) as usize)],
            None => 0,
        }
    }

    pub fn center_data(&self) -> Option<&Arc<ChunkData>> {
        self.chunks[1][1].as_ref()
    }

    pub fn biome(&self, x: usize, z: usize) -> u8 {
        self.center_data().map(|c| c.biomes[z * CHUNK_W + x]).unwrap_or(0)
    }
}
