//! Процедурная генерация чанков: рельеф, биомы, поверхность, деревья,
//! растительность. Пещеры, руды и шахты — в `caves.rs`, `ores.rs`, `mines.rs`.
//!
//! Все функции — чистые функции от (seed, координаты): чанк можно
//! сгенерировать в любом порядке и в любом потоке, результат воспроизводим.

use super::biome::BiomeId;
use super::block::{id, make, BlockId};
use super::caves::Caves;
use super::chunk::{ChunkData, ChunkPos, CHUNK_H, CHUNK_W};
use super::noise::{hash3, hash_f, Simplex};
use super::{mines, ores};

pub const SEA_LEVEL: i32 = 62;
pub const DEEP_LEVEL: i32 = 24;

pub struct WorldGen {
    pub seed: u64,
    continent: Simplex,
    hills: Simplex,
    detail: Simplex,
    mountain_mask: Simplex,
    ridge: Simplex,
    temperature: Simplex,
    humidity: Simplex,
    misc: Simplex,
    pub caves: Caves,
}

/// Параметры колонки (x, z).
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub height: i32,
    pub biome: BiomeId,
    pub temperature: f32,
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl WorldGen {
    pub fn new(seed: u64) -> Self {
        let s = |k: u64| Simplex::new(seed.wrapping_mul(6364136223846793005).wrapping_add(k));
        Self {
            seed,
            continent: s(1),
            hills: s(2),
            detail: s(3),
            mountain_mask: s(4),
            ridge: s(5),
            temperature: s(6),
            humidity: s(7),
            misc: s(8),
            caves: Caves::new(seed),
        }
    }

    /// Высота и биом колонки.
    pub fn column(&self, x: i32, z: i32) -> Column {
        let (fx, fz) = (x as f32, z as f32);
        let cont = self.continent.fbm2(fx / 700.0, fz / 700.0, 4, 2.0, 0.5) * 1.4 + 0.12;
        let temp = self.temperature.fbm2(fx / 900.0 + 31.0, fz / 900.0, 3, 2.0, 0.5) * 1.6;
        let hum = self.humidity.fbm2(fx / 750.0 - 17.0, fz / 750.0 + 9.0, 3, 2.0, 0.5) * 1.6;

        let land = smoothstep(-0.2, 0.15, cont);
        let dry_hot = smoothstep(0.2, 0.45, temp) * smoothstep(0.1, -0.15, hum);

        let mut h = SEA_LEVEL as f32 + 3.0 + cont * 26.0;
        let hills = self.hills.fbm2(fx / 140.0, fz / 140.0, 5, 2.0, 0.5);
        h += hills * 12.0 * land * (1.0 - 0.65 * dry_hot);
        h += self.detail.fbm2(fx / 40.0, fz / 40.0, 3, 2.0, 0.5) * 3.0 * land;

        let mmask = smoothstep(0.12, 0.5, self.mountain_mask.fbm2(fx / 520.0 + 77.0, fz / 520.0, 3, 2.0, 0.5)) * land;
        if mmask > 0.0 {
            let r = 1.0 - self.ridge.fbm2(fx / 190.0, fz / 190.0, 5, 2.1, 0.5).abs();
            h += mmask * (r * r * 90.0 + 12.0);
        }
        let height = (h as i32).clamp(8, CHUNK_H as i32 - 20);

        let biome = if height < SEA_LEVEL - 1 {
            BiomeId::Ocean
        } else if height <= SEA_LEVEL + 1 && mmask < 0.2 && temp > -0.35 {
            BiomeId::Beach
        } else if mmask > 0.45 && height > 92 {
            BiomeId::Mountains
        } else if temp < -0.32 {
            BiomeId::SnowyTaiga
        } else if dry_hot > 0.5 {
            BiomeId::Desert
        } else if hum > 0.12 {
            BiomeId::Forest
        } else {
            BiomeId::Plains
        };
        Column { height, biome, temperature: temp }
    }

    pub fn generate(&self, pos: ChunkPos) -> ChunkData {
        let mut c = ChunkData::empty();
        let bx = pos.x * CHUNK_W as i32;
        let bz = pos.z * CHUNK_W as i32;
        let mut cols = [[Column { height: 0, biome: BiomeId::Plains, temperature: 0.0 }; CHUNK_W]; CHUNK_W];
        for (z, row) in cols.iter_mut().enumerate() {
            for (x, col) in row.iter_mut().enumerate() {
                *col = self.column(bx + x as i32, bz + z as i32);
            }
        }

        // --- Рельеф и поверхность ---
        for z in 0..CHUNK_W {
            for x in 0..CHUNK_W {
                let col = cols[z][x];
                let (wx, wz) = (bx + x as i32, bz + z as i32);
                c.biomes[z * CHUNK_W + x] = col.biome as u8;
                let h = col.height;
                let soil = 3 + (hash3(self.seed, wx, 7, wz) % 2) as i32;
                // Переход к глубинному камню неровный.
                let deep = DEEP_LEVEL + (self.misc.noise2(wx as f32 / 16.0, wz as f32 / 16.0) * 4.0) as i32;
                for y in 0..=h {
                    let b: BlockId = if y == 0 || (y <= 3 && hash_f(self.seed, wx, y, wz) < 0.6 - y as f32 * 0.15) {
                        id::BEDROCK
                    } else if y < deep {
                        id::DEEP_STONE
                    } else if y < h - soil {
                        id::STONE
                    } else {
                        surface_block(col, y, h, soil)
                    };
                    c.set(x, y as usize, z, make(b, 0));
                }
                // Вода до уровня моря, лёд в холодных биомах.
                for y in (h + 1)..=SEA_LEVEL {
                    let frozen = y == SEA_LEVEL && col.temperature < -0.4;
                    c.set(x, y as usize, z, make(if frozen { id::ICE } else { id::WATER }, 0));
                }
            }
        }

        // --- Пещеры, руды, шахты ---
        self.caves.carve(&mut c, pos, &cols);
        ores::place(&mut c, pos, self.seed);
        mines::place(&mut c, pos, self);

        // --- Деревья и растительность ---
        self.decorate(&mut c, pos, &cols);
        c
    }

    /// Деревья ставятся по ячейкам 5×5 (один кандидат на ячейку) — это даёт
    /// равномерное распределение без слипшихся стволов. Деревья соседних
    /// чанков, чья крона заходит в этот чанк, тоже учитываются.
    fn decorate(&self, c: &mut ChunkData, pos: ChunkPos, cols: &[[Column; CHUNK_W]; CHUNK_W]) {
        let bx = pos.x * CHUNK_W as i32;
        let bz = pos.z * CHUNK_W as i32;
        const CELL: i32 = 5;
        let cx0 = (bx - 4).div_euclid(CELL);
        let cx1 = (bx + CHUNK_W as i32 + 4).div_euclid(CELL);
        let cz0 = (bz - 4).div_euclid(CELL);
        let cz1 = (bz + CHUNK_W as i32 + 4).div_euclid(CELL);
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                let hsh = hash3(self.seed ^ 0x7EE5, cx, 0, cz);
                let tx = cx * CELL + (hsh % CELL as u64) as i32;
                let tz = cz * CELL + ((hsh >> 8) % CELL as u64) as i32;
                let roll = ((hsh >> 16) & 0xFFFF) as f32 / 65535.0;
                let col = self.column(tx, tz);
                if col.height <= SEA_LEVEL {
                    continue;
                }
                let chance = match col.biome {
                    BiomeId::Forest => 0.62,
                    BiomeId::SnowyTaiga => 0.45,
                    BiomeId::Plains => 0.04,
                    BiomeId::Mountains => if col.height < 120 { 0.12 } else { 0.0 },
                    BiomeId::Desert => 0.08,
                    _ => 0.0,
                };
                if roll >= chance {
                    continue;
                }
                // Земля под деревом могла быть выкопана пещерой.
                if self.caves.is_cave(tx, col.height, tz, col.height) {
                    continue;
                }
                let kind = (hsh >> 32) % 100;
                let gy = col.height + 1;
                match col.biome {
                    BiomeId::Desert => self.cactus(c, bx, bz, tx, gy, tz, hsh),
                    BiomeId::SnowyTaiga | BiomeId::Mountains => self.spruce(c, bx, bz, tx, gy, tz, hsh),
                    BiomeId::Forest if kind < 25 => self.spruce(c, bx, bz, tx, gy, tz, hsh),
                    _ => self.oak(c, bx, bz, tx, gy, tz, hsh),
                }
            }
        }

        // Трава, цветы, кусты — только в пределах чанка.
        for z in 0..CHUNK_W {
            for x in 0..CHUNK_W {
                let col = cols[z][x];
                let y = col.height + 1;
                if y <= SEA_LEVEL || y >= CHUNK_H as i32 - 1 {
                    continue;
                }
                let (wx, wz) = (bx + x as i32, bz + z as i32);
                let below = super::block::block_id(c.get(x, (y - 1) as usize, z));
                if c.get(x, y as usize, z) != 0 {
                    continue;
                }
                let r = hash_f(self.seed ^ 0xF10, wx, 0, wz);
                let place = match col.biome {
                    BiomeId::Plains if below == id::GRASS => {
                        if r < 0.22 {
                            Some(id::TALL_GRASS)
                        } else if r < 0.235 {
                            Some(id::FLOWER_YELLOW)
                        } else if r < 0.25 {
                            Some(id::FLOWER_RED)
                        } else {
                            None
                        }
                    }
                    BiomeId::Forest if below == id::GRASS => {
                        if r < 0.12 {
                            Some(id::TALL_GRASS)
                        } else if r < 0.13 {
                            Some(id::FLOWER_RED)
                        } else {
                            None
                        }
                    }
                    BiomeId::Mountains if below == id::GRASS && r < 0.08 => Some(id::TALL_GRASS),
                    BiomeId::Desert if below == id::SAND && r < 0.012 => Some(id::DEAD_BUSH),
                    _ => None,
                };
                if let Some(b) = place {
                    c.set(x, y as usize, z, make(b, 0));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn put(&self, c: &mut ChunkData, bx: i32, bz: i32, x: i32, y: i32, z: i32, b: BlockId, overwrite: bool) {
        let lx = x - bx;
        let lz = z - bz;
        if !(0..CHUNK_W as i32).contains(&lx) || !(0..CHUNK_W as i32).contains(&lz) || !(1..CHUNK_H as i32).contains(&y) {
            return;
        }
        let cur = c.get(lx as usize, y as usize, lz as usize);
        let cur_id = super::block::block_id(cur);
        if overwrite || cur == 0 || super::block::def(cur_id).replaceable && cur_id != id::WATER {
            c.set(lx as usize, y as usize, lz as usize, make(b, 0));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn oak(&self, c: &mut ChunkData, bx: i32, bz: i32, x: i32, y: i32, z: i32, h: u64) {
        let height = 4 + ((h >> 40) % 3) as i32;
        let top = y + height;
        for dy in -2..=1i32 {
            let ly = top + dy;
            let r: i32 = if dy >= 0 { 1 } else { 2 };
            for dz in -r..=r {
                for dx in -r..=r {
                    let corner = dx.abs() == r && dz.abs() == r;
                    if corner && (dy == 1 || hash3(h, dx, ly, dz) % 3 == 0) {
                        continue;
                    }
                    self.put(c, bx, bz, x + dx, ly, z + dz, id::OAK_LEAVES, false);
                }
            }
        }
        self.put(c, bx, bz, x, top + 1, z, id::OAK_LEAVES, false);
        for dy in 0..height {
            self.put(c, bx, bz, x, y + dy, z, id::OAK_LOG, true);
        }
        self.put(c, bx, bz, x, y - 1, z, id::DIRT, true);
    }

    #[allow(clippy::too_many_arguments)]
    fn spruce(&self, c: &mut ChunkData, bx: i32, bz: i32, x: i32, y: i32, z: i32, h: u64) {
        let height = 6 + ((h >> 40) % 4) as i32;
        let top = y + height;
        // Конусообразная крона: радиус растёт книзу, ярусами.
        let mut r = 0i32;
        for ly in (y + 2..=top + 1).rev() {
            let layer = top + 1 - ly;
            r = match layer {
                0 => 0,
                1 => 1,
                _ => {
                    if layer % 2 == 0 {
                        (r + 1).min(3)
                    } else {
                        (r - 1).max(1)
                    }
                }
            };
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dz * dz > r * r + 1 {
                        continue;
                    }
                    self.put(c, bx, bz, x + dx, ly, z + dz, id::SPRUCE_LEAVES, false);
                }
            }
        }
        for dy in 0..height {
            self.put(c, bx, bz, x, y + dy, z, id::SPRUCE_LOG, true);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn cactus(&self, c: &mut ChunkData, bx: i32, bz: i32, x: i32, y: i32, z: i32, h: u64) {
        let height = 1 + ((h >> 40) % 3) as i32;
        for dy in 0..height {
            self.put(c, bx, bz, x, y + dy, z, id::CACTUS, false);
        }
    }
}

fn surface_block(col: Column, y: i32, h: i32, soil: i32) -> BlockId {
    let top = y == h;
    match col.biome {
        BiomeId::Desert => {
            if y >= h - soil + 1 {
                id::SAND
            } else {
                id::SANDSTONE
            }
        }
        BiomeId::Beach => {
            if y >= h - 2 {
                id::SAND
            } else {
                id::SANDSTONE
            }
        }
        BiomeId::Ocean => {
            if h < SEA_LEVEL - 12 {
                if top && (h & 3) == 0 { id::CLAY } else { id::GRAVEL }
            } else {
                id::SAND
            }
        }
        BiomeId::Mountains => {
            if h > 140 {
                if top { id::SNOW } else { id::STONE }
            } else if h > 108 {
                id::STONE
            } else if top {
                id::GRASS
            } else {
                id::DIRT
            }
        }
        BiomeId::SnowyTaiga => {
            if top {
                id::SNOW_GRASS
            } else {
                id::DIRT
            }
        }
        _ => {
            if top {
                id::GRASS
            } else {
                id::DIRT
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::block_id;

    #[test]
    fn generation_is_deterministic() {
        let a = WorldGen::new(99).generate(ChunkPos::new(3, -2));
        let b = WorldGen::new(99).generate(ChunkPos::new(3, -2));
        assert!(a.blocks == b.blocks);
        let c = WorldGen::new(100).generate(ChunkPos::new(3, -2));
        assert!(a.blocks != c.blocks);
    }

    #[test]
    fn dump_ocean_column() {
        let g = WorldGen::new(2024);
        let c = g.generate(ChunkPos::new(3, 1));
        for x in [0usize, 1, 7, 15] {
            let col: Vec<u8> = (55..66).map(|y| block_id(c.get(x, y, 4))).collect();
            println!("x={x}: {col:?}");
        }
    }
}
