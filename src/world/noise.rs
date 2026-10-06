//! Собственная реализация Simplex-шума (2D/3D) по Стефану Густавсону с
//! сидируемой таблицей перестановок + фрактальный шум (fBm).

#[derive(Clone)]
pub struct Simplex {
    perm: [u8; 512],
}

const GRAD3: [[f32; 3]; 12] = [
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, -1.0], [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0], [0.0, -1.0, 1.0], [0.0, 1.0, -1.0], [0.0, -1.0, -1.0],
];

/// SplitMix64 — генератор для перемешивания таблицы и прочих нужд.
pub fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Simplex {
    pub fn new(seed: u64) -> Self {
        let mut p: [u8; 256] = [0; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        let mut s = seed ^ 0xA5A5_5A5A_DEAD_BEEF;
        for i in (1..256).rev() {
            let j = (splitmix(&mut s) % (i as u64 + 1)) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Self { perm }
    }

    #[inline(always)]
    fn hash(&self, i: i32) -> usize {
        self.perm[(i & 255) as usize] as usize
    }

    /// 2D simplex, результат примерно в [-1, 1].
    pub fn noise2(&self, xin: f32, yin: f32) -> f32 {
        const F2: f32 = 0.366_025_42; // 0.5*(sqrt(3)-1)
        const G2: f32 = 0.211_324_87; // (3-sqrt(3))/6
        let s = (xin + yin) * F2;
        let i = (xin + s).floor() as i32;
        let j = (yin + s).floor() as i32;
        let t = (i + j) as f32 * G2;
        let x0 = xin - (i as f32 - t);
        let y0 = yin - (j as f32 - t);
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f32 + G2;
        let y1 = y0 - j1 as f32 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;
        let gi0 = self.hash(i + self.hash(j) as i32) % 12;
        let gi1 = self.hash(i + i1 + self.hash(j + j1) as i32) % 12;
        let gi2 = self.hash(i + 1 + self.hash(j + 1) as i32) % 12;
        let mut n = 0.0;
        let t0 = 0.5 - x0 * x0 - y0 * y0;
        if t0 > 0.0 {
            let t0 = t0 * t0;
            n += t0 * t0 * (GRAD3[gi0][0] * x0 + GRAD3[gi0][1] * y0);
        }
        let t1 = 0.5 - x1 * x1 - y1 * y1;
        if t1 > 0.0 {
            let t1 = t1 * t1;
            n += t1 * t1 * (GRAD3[gi1][0] * x1 + GRAD3[gi1][1] * y1);
        }
        let t2 = 0.5 - x2 * x2 - y2 * y2;
        if t2 > 0.0 {
            let t2 = t2 * t2;
            n += t2 * t2 * (GRAD3[gi2][0] * x2 + GRAD3[gi2][1] * y2);
        }
        70.0 * n
    }

    /// 3D simplex, результат примерно в [-1, 1].
    pub fn noise3(&self, xin: f32, yin: f32, zin: f32) -> f32 {
        const F3: f32 = 1.0 / 3.0;
        const G3: f32 = 1.0 / 6.0;
        let s = (xin + yin + zin) * F3;
        let i = (xin + s).floor() as i32;
        let j = (yin + s).floor() as i32;
        let k = (zin + s).floor() as i32;
        let t = (i + j + k) as f32 * G3;
        let x0 = xin - (i as f32 - t);
        let y0 = yin - (j as f32 - t);
        let z0 = zin - (k as f32 - t);
        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };
        let x1 = x0 - i1 as f32 + G3;
        let y1 = y0 - j1 as f32 + G3;
        let z1 = z0 - k1 as f32 + G3;
        let x2 = x0 - i2 as f32 + 2.0 * G3;
        let y2 = y0 - j2 as f32 + 2.0 * G3;
        let z2 = z0 - k2 as f32 + 2.0 * G3;
        let x3 = x0 - 1.0 + 3.0 * G3;
        let y3 = y0 - 1.0 + 3.0 * G3;
        let z3 = z0 - 1.0 + 3.0 * G3;
        let h = |a: i32, b: i32, c: i32| -> usize {
            self.hash(a + self.hash(b + self.hash(c) as i32) as i32) % 12
        };
        let gi0 = h(i, j, k);
        let gi1 = h(i + i1, j + j1, k + k1);
        let gi2 = h(i + i2, j + j2, k + k2);
        let gi3 = h(i + 1, j + 1, k + 1);
        let corner = |g: usize, x: f32, y: f32, z: f32| -> f32 {
            let t = 0.6 - x * x - y * y - z * z;
            if t < 0.0 {
                0.0
            } else {
                let t = t * t;
                t * t * (GRAD3[g][0] * x + GRAD3[g][1] * y + GRAD3[g][2] * z)
            }
        };
        32.0 * (corner(gi0, x0, y0, z0)
            + corner(gi1, x1, y1, z1)
            + corner(gi2, x2, y2, z2)
            + corner(gi3, x3, y3, z3))
    }

    /// Фрактальный шум 2D: сумма октав, нормирована в [-1, 1].
    pub fn fbm2(&self, x: f32, y: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for o in 0..octaves {
            // Небольшой сдвиг на октаву убирает артефакты в начале координат.
            let off = o as f32 * 17.31;
            sum += self.noise2(x * freq + off, y * freq - off) * amp;
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        sum / norm
    }

    pub fn fbm3(&self, x: f32, y: f32, z: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for o in 0..octaves {
            let off = o as f32 * 11.7;
            sum += self.noise3(x * freq + off, y * freq, z * freq - off) * amp;
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        sum / norm
    }
}

/// Детерминированный хэш координат (для деревьев, руд, шахт).
#[inline]
pub fn hash3(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let mut h = seed
        ^ (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h = (h ^ (h >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h = (h ^ (h >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^ (h >> 33)
}

/// Хэш -> число в [0, 1).
#[inline]
pub fn hash_f(seed: u64, x: i32, y: i32, z: i32) -> f32 {
    (hash3(seed, x, y, z) >> 40) as f32 / (1u64 << 24) as f32
}

/// Простой детерминированный ГПСЧ.
#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x1234_5678_9ABC_DEF0)
    }
    pub fn next_u64(&mut self) -> u64 {
        splitmix(&mut self.0)
    }
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo) as u64) as i32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
}
