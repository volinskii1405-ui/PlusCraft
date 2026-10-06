//! Чанк 16×16×256 и его данные.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub const CHUNK_W: usize = 16;
pub const CHUNK_H: usize = 256;
pub const CHUNK_AREA: usize = CHUNK_W * CHUNK_W;
pub const CHUNK_VOL: usize = CHUNK_AREA * CHUNK_H;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
    pub fn of_block(bx: i32, bz: i32) -> Self {
        Self { x: bx.div_euclid(CHUNK_W as i32), z: bz.div_euclid(CHUNK_W as i32) }
    }
    pub fn offset(self, dx: i32, dz: i32) -> Self {
        Self { x: self.x + dx, z: self.z + dz }
    }
}

/// Индекс ячейки: y — старшие биты, затем z, затем x.
#[inline(always)]
pub fn idx(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
}

/// Блоки чанка + карта биомов. Хранится за `Arc`, чтобы фоновые потоки
/// мешинга получали дешёвый снимок; запись — через `Arc::make_mut` (копия
/// при записи, только если снимок ещё у кого-то в руках).
#[derive(Clone, Serialize, Deserialize)]
pub struct ChunkData {
    pub blocks: Vec<u16>,
    pub biomes: Vec<u8>,
}

impl ChunkData {
    pub fn empty() -> Self {
        Self { blocks: vec![0; CHUNK_VOL], biomes: vec![0; CHUNK_AREA] }
    }
    #[inline(always)]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u16 {
        self.blocks[idx(x, y, z)]
    }
    #[inline(always)]
    pub fn set(&mut self, x: usize, y: usize, z: usize, v: u16) {
        self.blocks[idx(x, y, z)] = v;
    }
    /// Максимальная высота непустого блока (+1) — для ограничения мешинга.
    pub fn top_y(&self) -> usize {
        for y in (0..CHUNK_H).rev() {
            let base = y << 8;
            if self.blocks[base..base + CHUNK_AREA].iter().any(|&b| b != 0) {
                return y + 1;
            }
        }
        0
    }
}

/// Освещённость чанка: старшая тетрада — небо, младшая — блоки.
pub type LightData = Vec<u8>;

pub struct Chunk {
    pub pos: ChunkPos,
    pub data: Arc<ChunkData>,
    /// Свет, посчитанный при последнем мешинге (для игровой логики).
    pub light: Option<Arc<LightData>>,
    /// Изменён игроком/миром — нужно сохранить.
    pub modified: bool,
    /// Версия данных: растёт при каждом изменении (для отбрасывания устаревших мешей).
    pub version: u64,
    pub mesh_dirty: bool,
    pub mesh_pending: bool,
}

impl Chunk {
    pub fn new(pos: ChunkPos, data: ChunkData) -> Self {
        Self {
            pos,
            data: Arc::new(data),
            light: None,
            modified: false,
            version: 0,
            mesh_dirty: true,
            mesh_pending: false,
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u16 {
        self.data.blocks[idx(x, y, z)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, v: u16) {
        Arc::make_mut(&mut self.data).blocks[idx(x, y, z)] = v;
        self.version += 1;
        self.mesh_dirty = true;
    }

    /// Уровни света (небо, блоки) в ячейке. Без посчитанного света — «день».
    pub fn light_at(&self, x: usize, y: usize, z: usize) -> (u8, u8) {
        match &self.light {
            Some(l) => {
                let v = l[idx(x, y, z)];
                (v >> 4, v & 15)
            }
            None => (15, 0),
        }
    }
}
