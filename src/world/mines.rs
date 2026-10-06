//! Заброшенные шахты: сеть штреков на одном уровне, с крепями из досок,
//! рельсами, сундуками, паутиной и прогоревшими факелами.
//!
//! Мир разбит на регионы 6×6 чанков; в регионе с вероятностью ~40% есть шахта.
//! Её узлы лежат на сетке с шагом `GRID`, рёбра между соседними узлами
//! существуют с некоторой вероятностью. Всё — чистые функции от seed, поэтому
//! каждый чанк прорезает только свою часть сети.

use super::block::{block_id, id, make, BlockId};
use super::chunk::{ChunkData, ChunkPos, CHUNK_H, CHUNK_W};
use super::gen::WorldGen;
use super::noise::{hash3, hash_f};

const REGION: i32 = 6;
const GRID: i32 = 10;

/// Флаг метаданных сундука: содержимое ещё не сгенерировано (лут шахты).
pub const CHEST_LOOT_FLAG: u8 = 0x80;

struct Mine {
    cx: i32,
    cz: i32,
    y: i32,
    n: i32,
    seed: u64,
}

fn mine_in_region(seed: u64, rx: i32, rz: i32) -> Option<Mine> {
    let h = hash3(seed ^ 0x3141_5926, rx, 0, rz);
    if h % 100 >= 40 {
        return None;
    }
    let base_x = rx * REGION * CHUNK_W as i32;
    let base_z = rz * REGION * CHUNK_W as i32;
    let span = REGION * CHUNK_W as i32;
    Some(Mine {
        cx: base_x + 24 + ((h >> 8) % (span as u64 - 48)) as i32,
        cz: base_z + 24 + ((h >> 20) % (span as u64 - 48)) as i32,
        y: 18 + ((h >> 32) % 28) as i32,
        n: 2 + ((h >> 40) % 3) as i32,
        seed: h,
    })
}

impl Mine {
    fn node(&self, i: i32, j: i32) -> (i32, i32) {
        (self.cx + i * GRID, self.cz + j * GRID)
    }
    fn node_exists(&self, i: i32, j: i32) -> bool {
        i * i + j * j <= self.n * self.n + 1
    }
    /// Ребро от узла (i,j) к (i+1,j) (dir=0) или (i,j+1) (dir=1).
    fn edge(&self, i: i32, j: i32, dir: i32) -> bool {
        let (ni, nj) = if dir == 0 { (i + 1, j) } else { (i, j + 1) };
        if !self.node_exists(i, j) || !self.node_exists(ni, nj) {
            return false;
        }
        // Центральный «крест» всегда есть — шахта связна.
        if (dir == 0 && j == 0) || (dir == 1 && i == 0) {
            return true;
        }
        hash3(self.seed, i, dir, j) % 100 < 62
    }
}

struct Carver<'a> {
    c: &'a mut ChunkData,
    bx: i32,
    bz: i32,
}

impl Carver<'_> {
    fn local(&self, x: i32, y: i32, z: i32) -> Option<(usize, usize, usize)> {
        let lx = x - self.bx;
        let lz = z - self.bz;
        if (0..16).contains(&lx) && (0..16).contains(&lz) && (1..CHUNK_H as i32 - 1).contains(&y) {
            Some((lx as usize, y as usize, lz as usize))
        } else {
            None
        }
    }
    fn get(&self, x: i32, y: i32, z: i32) -> Option<BlockId> {
        self.local(x, y, z).map(|(a, b, c)| block_id(self.c.get(a, b, c)))
    }
    fn set(&mut self, x: i32, y: i32, z: i32, v: u16) {
        if let Some((a, b, c)) = self.local(x, y, z) {
            let cur = block_id(self.c.get(a, b, c));
            if cur == id::BEDROCK {
                return;
            }
            // Не вскрываем водоёмы.
            if v == 0 {
                if let Some(up) = self.get(x, y + 1, z) {
                    if up == id::WATER {
                        return;
                    }
                }
            }
            self.c.set(a, b, c, v);
        }
    }
}

pub fn place(c: &mut ChunkData, pos: ChunkPos, gen: &WorldGen) {
    let seed = gen.seed;
    let bx = pos.x * CHUNK_W as i32;
    let bz = pos.z * CHUNK_W as i32;
    let rx = pos.x.div_euclid(REGION);
    let rz = pos.z.div_euclid(REGION);
    let mut carver = Carver { c, bx, bz };
    for mrz in rz - 1..=rz + 1 {
        for mrx in rx - 1..=rx + 1 {
            let Some(mine) = mine_in_region(seed, mrx, mrz) else { continue };
            let reach = (mine.n + 1) * GRID + 3;
            if mine.cx + reach < bx || mine.cx - reach > bx + 16 || mine.cz + reach < bz || mine.cz - reach > bz + 16 {
                continue;
            }
            carve_mine(&mut carver, &mine);
        }
    }
}

fn carve_mine(cv: &mut Carver, m: &Mine) {
    let y = m.y;
    for j in -m.n..=m.n {
        for i in -m.n..=m.n {
            if !m.node_exists(i, j) {
                continue;
            }
            for dir in 0..2 {
                if m.edge(i, j, dir) {
                    corridor(cv, m, i, j, dir);
                }
            }
        }
    }
    // Узлы — небольшие залы 5×5 высотой 4.
    for j in -m.n..=m.n {
        for i in -m.n..=m.n {
            let connected = m.edge(i, j, 0) || m.edge(i, j, 1) || m.edge(i - 1, j, 0) || m.edge(i, j - 1, 1);
            if !m.node_exists(i, j) || !connected {
                continue;
            }
            let (nx, nz) = m.node(i, j);
            for dz in -2..=2 {
                for dx in -2..=2 {
                    for dy in 0..4 {
                        cv.set(nx + dx, y + dy, nz + dz, 0);
                    }
                    floor(cv, nx + dx, y - 1, nz + dz);
                }
            }
            // Центральная стойка-крепь в зале.
            if hash3(m.seed, i, 7, j) % 2 == 0 {
                for dy in 0..3 {
                    cv.set(nx, y + dy, nz, make(id::SUPPORT, 0));
                }
                cv.set(nx, y + 3, nz, make(id::PLANKS, 0));
            }
        }
    }
}

fn floor(cv: &mut Carver, x: i32, y: i32, z: i32) {
    if let Some(b) = cv.get(x, y, z) {
        if b == id::AIR || b == id::WATER || b == id::LAVA {
            cv.set(x, y, z, make(id::PLANKS, 0));
        }
    }
}

fn corridor(cv: &mut Carver, m: &Mine, i: i32, j: i32, dir: i32) {
    let y = m.y;
    let (ax, az) = m.node(i, j);
    let rails = hash3(m.seed, i, 10 + dir, j) % 100 < 65;
    for t in 0..=GRID {
        // Вдоль: (x, z) центральной линии; поперёк — смещение s.
        let (cx, cz) = if dir == 0 { (ax + t, az) } else { (ax, az + t) };
        let at = |s: i32| if dir == 0 { (cx, cz + s) } else { (cx + s, cz) };
        let support = t % 4 == 2;
        for s in -1..=1 {
            let (x, z) = at(s);
            for dy in 0..3 {
                cv.set(x, y + dy, z, 0);
            }
            floor(cv, x, y - 1, z);
        }
        if support {
            for s in [-1, 1] {
                let (x, z) = at(s);
                cv.set(x, y, z, make(id::SUPPORT, 0));
                cv.set(x, y + 1, z, make(id::SUPPORT, 0));
            }
            for s in -1..=1 {
                let (x, z) = at(s);
                cv.set(x, y + 2, z, make(id::PLANKS, 0));
            }
        } else {
            let (x, z) = at(0);
            let h = hash_f(m.seed, x, y, z);
            if rails && h < 0.92 {
                cv.set(x, y, z, make(id::RAIL, if dir == 0 { 1 } else { 0 }));
            }
            // Паутина под потолком.
            let side = if hash3(m.seed, x, y + 1, z) % 2 == 0 { -1 } else { 1 };
            let (wx, wz) = at(side);
            let r = hash_f(m.seed ^ 0xAB, x, y, z);
            if r < 0.06 {
                cv.set(wx, y + 2, wz, make(id::COBWEB, 0));
            } else if r < 0.075 {
                // Сундук у стены, лицом к центру штрека.
                let facing = match (dir, side) {
                    (0, -1) => 4, // сундук на -Z стороне смотрит в +Z
                    (0, _) => 5,
                    (_, -1) => 2,
                    _ => 3,
                };
                cv.set(wx, y, wz, make(id::CHEST, facing | CHEST_LOOT_FLAG));
            } else if r < 0.09 {
                cv.set(wx, y, wz, make(id::BURNT_TORCH, 0));
            } else if r < 0.1 {
                // Завал гравия.
                for s in -1..=1 {
                    let (gx, gz) = at(s);
                    cv.set(gx, y, gz, make(id::GRAVEL, 0));
                }
            }
        }
    }
}
