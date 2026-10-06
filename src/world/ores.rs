//! Рудные жилы: разные глубины и редкость. В глубинном камне руды получают
//! «глубинные» варианты.

use super::block::{block_id, id, make, BlockId};
use super::chunk::{ChunkData, ChunkPos, CHUNK_H};
use super::noise::{hash3, Rng};

struct OreKind {
    block: BlockId,
    deep_block: BlockId,
    min_y: i32,
    max_y: i32,
    /// Среднее число жил на чанк (дробная часть — вероятность ещё одной).
    veins: f32,
    size: (i32, i32),
    /// Только у стен пустот (резонит растёт в пещерах).
    near_air: bool,
}

const ORES: &[OreKind] = &[
    OreKind { block: id::COAL_ORE, deep_block: id::DEEP_COAL_ORE, min_y: 5, max_y: 140, veins: 16.0, size: (6, 13), near_air: false },
    OreKind { block: id::COPPER_ORE, deep_block: id::COPPER_ORE, min_y: 20, max_y: 100, veins: 9.0, size: (4, 10), near_air: false },
    OreKind { block: id::IRON_ORE, deep_block: id::DEEP_IRON_ORE, min_y: 4, max_y: 72, veins: 8.0, size: (4, 8), near_air: false },
    OreKind { block: id::GOLD_ORE, deep_block: id::DEEP_GOLD_ORE, min_y: 2, max_y: 34, veins: 2.5, size: (3, 7), near_air: false },
    OreKind { block: id::DIAMOND_ORE, deep_block: id::DEEP_DIAMOND_ORE, min_y: 2, max_y: 17, veins: 1.1, size: (2, 6), near_air: false },
    OreKind { block: id::RESONITE_ORE, deep_block: id::RESONITE_ORE, min_y: 4, max_y: 44, veins: 2.2, size: (2, 5), near_air: true },
    OreKind { block: id::GRAVEL, deep_block: id::GRAVEL, min_y: 5, max_y: 110, veins: 4.0, size: (10, 22), near_air: false },
    OreKind { block: id::DIRT, deep_block: id::DIRT, min_y: 30, max_y: 110, veins: 3.0, size: (10, 22), near_air: false },
];

pub fn place(c: &mut ChunkData, pos: ChunkPos, seed: u64) {
    for (k, ore) in ORES.iter().enumerate() {
        let mut rng = Rng::new(hash3(seed ^ 0x0BE5, pos.x, k as i32, pos.z));
        let mut count = ore.veins as i32;
        if rng.f32() < ore.veins.fract() {
            count += 1;
        }
        for _ in 0..count {
            let mut x = rng.range(1, 15);
            let mut y = rng.range(ore.min_y, ore.max_y + 1);
            let mut z = rng.range(1, 15);
            if ore.near_air && !touches_air(c, x, y, z) {
                // Ищем стену пустоты поблизости по вертикали.
                let mut found = false;
                for dy in 1..12 {
                    for yy in [y - dy, y + dy] {
                        if yy > 2 && yy < ore.max_y && touches_air(c, x, yy, z) {
                            y = yy;
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                }
                if !found {
                    continue;
                }
            }
            let size = rng.range(ore.size.0, ore.size.1 + 1);
            for _ in 0..size {
                if (1..CHUNK_H as i32 - 1).contains(&y) {
                    let cur = block_id(c.get(x as usize, y as usize, z as usize));
                    if cur == id::STONE {
                        c.set(x as usize, y as usize, z as usize, make(ore.block, 0));
                    } else if cur == id::DEEP_STONE {
                        c.set(x as usize, y as usize, z as usize, make(ore.deep_block, 0));
                    }
                }
                match rng.range(0, 6) {
                    0 => x += 1,
                    1 => x -= 1,
                    2 => y += 1,
                    3 => y -= 1,
                    4 => z += 1,
                    _ => z -= 1,
                }
                x = x.clamp(0, 15);
                z = z.clamp(0, 15);
            }
        }
    }
}

fn touches_air(c: &ChunkData, x: i32, y: i32, z: i32) -> bool {
    let cur = block_id(c.get(x as usize, y as usize, z as usize));
    if cur != id::STONE && cur != id::DEEP_STONE {
        return false;
    }
    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
        let (nx, ny, nz) = (x + dx, y + dy, z + dz);
        if !(0..16).contains(&nx) || !(0..16).contains(&nz) || !(1..CHUNK_H as i32).contains(&ny) {
            continue;
        }
        if c.get(nx as usize, ny as usize, nz as usize) == 0 {
            return true;
        }
    }
    false
}
