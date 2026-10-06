//! Пещеры: «спагетти»-туннели из 3D-шума, большие полости на глубине и
//! «червяки» (Perlin worms) — длинные извилистые ходы.

use super::block::{block_id, id, make};
use super::chunk::{ChunkData, ChunkPos, CHUNK_H, CHUNK_W};
use super::gen::{Column, SEA_LEVEL};
use super::noise::{hash3, Rng, Simplex};

pub struct Caves {
    seed: u64,
    tunnel_a: Simplex,
    tunnel_b: Simplex,
    cavern: Simplex,
    entrance: Simplex,
}

/// Уровень, ниже которого полости заполняются лавой.
pub const LAVA_LEVEL: i32 = 10;

impl Caves {
    pub fn new(seed: u64) -> Self {
        let s = |k: u64| Simplex::new(seed ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        Self { seed, tunnel_a: s(101), tunnel_b: s(102), cavern: s(103), entrance: s(104) }
    }

    /// Есть ли шумовая пещера в точке (без червяков — они не чистая функция точки).
    pub fn is_cave(&self, x: i32, y: i32, z: i32, surface: i32) -> bool {
        if y <= 2 {
            return false;
        }
        let (fx, fy, fz) = (x as f32, y as f32, z as f32);
        // Возле поверхности пещеры выходят наружу редко — только там, где высок
        // шум «входов».
        let depth = surface - y;
        if depth < 6 {
            let e = self.entrance.noise2(fx / 90.0, fz / 90.0);
            if e < 0.55 {
                return false;
            }
        }
        // Спагетти: пересечение двух «листов» шума -> трубки.
        let a = self.tunnel_a.noise3(fx / 48.0, fy / 28.0, fz / 48.0);
        let b = self.tunnel_b.noise3(fx / 48.0, fy / 28.0, fz / 48.0);
        let t = 0.075 + (y as f32 / 128.0).min(1.0) * 0.02;
        if a.abs() < t && b.abs() < t {
            return true;
        }
        // Большие полости глубже 50.
        if y < 50 {
            let c = self.cavern.fbm3(fx / 70.0, fy / 30.0, fz / 70.0, 2, 2.0, 0.5);
            let thresh = 0.42 + (y as f32 - 25.0).abs() / 60.0;
            if c > thresh {
                return true;
            }
        }
        false
    }

    pub fn carve(&self, c: &mut ChunkData, pos: ChunkPos, cols: &[[Column; CHUNK_W]; CHUNK_W]) {
        let bx = pos.x * CHUNK_W as i32;
        let bz = pos.z * CHUNK_W as i32;
        let mut carved = vec![false; CHUNK_W * CHUNK_W * CHUNK_H];

        for z in 0..CHUNK_W {
            for x in 0..CHUNK_W {
                let h = cols[z][x].height;
                let ocean = h < SEA_LEVEL + 2;
                for y in 1..=h.min(CHUNK_H as i32 - 2) {
                    // Под морским дном не копаем близко к воде.
                    if ocean && y > h - 5 {
                        continue;
                    }
                    if self.is_cave(bx + x as i32, y, bz + z as i32, h) {
                        carved[(y as usize * CHUNK_W + z) * CHUNK_W + x] = true;
                    }
                }
            }
        }

        self.worms(pos, &mut carved, cols);

        for z in 0..CHUNK_W {
            for x in 0..CHUNK_W {
                for y in 1..CHUNK_H - 1 {
                    if !carved[(y * CHUNK_W + z) * CHUNK_W + x] {
                        continue;
                    }
                    let cur = block_id(c.get(x, y, z));
                    if cur == id::BEDROCK || cur == id::WATER || cur == id::ICE {
                        continue;
                    }
                    // Не вскрываем дно водоёмов.
                    let above = block_id(c.get(x, y + 1, z));
                    if above == id::WATER || above == id::ICE {
                        continue;
                    }
                    let fill = if (y as i32) <= LAVA_LEVEL { make(id::LAVA, 0) } else { 0 };
                    c.set(x, y, z, fill);
                    // Трава под открытым небом над вскрытой землёй: если сверху
                    // выкопали, нижний блок земли становится травой (для красоты входов).
                    if y > 0 && fill == 0 {
                        let below = block_id(c.get(x, y - 1, z));
                        if below == id::DIRT && y as i32 >= cols[z][x].height - 1 {
                            c.set(x, y - 1, z, make(id::GRASS, 0));
                        }
                    }
                }
            }
        }
    }

    /// «Червяки»: из чанков в радиусе R стартуют извилистые туннели; каждый
    /// прорезает в текущем чанке те участки, что в него попадают.
    fn worms(&self, pos: ChunkPos, carved: &mut [bool], cols: &[[Column; CHUNK_W]; CHUNK_W]) {
        const R: i32 = 6;
        let bx = (pos.x * CHUNK_W as i32) as f32;
        let bz = (pos.z * CHUNK_W as i32) as f32;
        for sz in pos.z - R..=pos.z + R {
            for sx in pos.x - R..=pos.x + R {
                let h = hash3(self.seed ^ 0xC0FFEE, sx, 0, sz);
                if h % 100 >= 14 {
                    continue;
                }
                let mut rng = Rng::new(h);
                let mut p = [
                    (sx * 16) as f32 + rng.f32() * 16.0,
                    12.0 + rng.f32() * 58.0,
                    (sz * 16) as f32 + rng.f32() * 16.0,
                ];
                let mut yaw = rng.f32() * std::f32::consts::TAU;
                let mut pitch = (rng.f32() - 0.5) * 0.6;
                let mut dyaw = 0.0f32;
                let mut dpitch = 0.0f32;
                let len = 90 + rng.range(0, 120);
                let base_r = 1.6 + rng.f32() * 1.6;
                for step in 0..len {
                    let r = base_r * (0.75 + 0.45 * ((step as f32) * 0.12).sin());
                    // Быстрая проверка пересечения с чанком.
                    if p[0] + r + 1.0 >= bx && p[0] - r - 1.0 < bx + 16.0 && p[2] + r + 1.0 >= bz && p[2] - r - 1.0 < bz + 16.0 {
                        let (x0, x1) = ((p[0] - r).floor() as i32, (p[0] + r).ceil() as i32);
                        let (y0, y1) = ((p[1] - r).floor() as i32, (p[1] + r).ceil() as i32);
                        let (z0, z1) = ((p[2] - r).floor() as i32, (p[2] + r).ceil() as i32);
                        for wz in z0..=z1 {
                            for wx in x0..=x1 {
                                let lx = wx - bx as i32;
                                let lz = wz - bz as i32;
                                if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
                                    continue;
                                }
                                let surf = cols[lz as usize][lx as usize].height;
                                for wy in y0.max(1)..=y1.min(CHUNK_H as i32 - 2) {
                                    // Червяки не выходят на поверхность.
                                    if wy > surf - 4 {
                                        continue;
                                    }
                                    let d = [wx as f32 + 0.5 - p[0], (wy as f32 + 0.5 - p[1]) * 1.3, wz as f32 + 0.5 - p[2]];
                                    if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < r * r {
                                        carved[(wy as usize * CHUNK_W + lz as usize) * CHUNK_W + lx as usize] = true;
                                    }
                                }
                            }
                        }
                    }
                    let (sy, cy) = yaw.sin_cos();
                    let cp = pitch.cos();
                    p[0] += cy * cp;
                    p[1] += pitch.sin();
                    p[2] += sy * cp;
                    pitch *= 0.85;
                    pitch += dpitch * 0.1;
                    yaw += dyaw * 0.1;
                    dpitch = dpitch * 0.9 + (rng.f32() - rng.f32()) * 2.0;
                    dyaw = dyaw * 0.75 + (rng.f32() - rng.f32()) * 4.0;
                    if p[1] < 6.0 {
                        pitch = pitch.abs();
                    }
                }
            }
        }
    }
}
