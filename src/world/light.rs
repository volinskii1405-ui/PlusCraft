//! Освещение flood fill (BFS) — небесное и блочное.
//!
//! Свет считается на фоновом потоке при мешинге чанка по окрестности 3×3
//! чанка: область = центральный чанк + 15 блоков с каждой стороны (дальше свет
//! уровня 15 всё равно не дотянется). Благодаря этому свет на границах чанков
//! всегда согласован, а пересчёт при установке/разрушении блоков сводится к
//! перемешиванию затронутых чанков.

use std::collections::VecDeque;

use super::block::{block_id, def};
use super::chunk::{CHUNK_H, CHUNK_W};
use super::neighborhood::Neighborhood;

/// Отступ области вокруг центрального чанка.
pub const MARGIN: i32 = 15;
pub const REGION_W: usize = CHUNK_W + 2 * MARGIN as usize; // 46

/// Поле света области: (sky << 4) | block для каждой ячейки.
pub struct LightField {
    pub data: Vec<u8>,
}

#[inline(always)]
fn ridx(x: usize, y: usize, z: usize) -> usize {
    (y * REGION_W + z) * REGION_W + x
}

impl LightField {
    /// x, z — в координатах центрального чанка (могут быть от -15 до 30).
    #[inline(always)]
    pub fn get(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        if y >= CHUNK_H as i32 {
            return (15, 0);
        }
        if y < 0 {
            return (0, 0);
        }
        let rx = x + MARGIN;
        let rz = z + MARGIN;
        if rx < 0 || rz < 0 || rx >= REGION_W as i32 || rz >= REGION_W as i32 {
            return (15, 0);
        }
        let v = self.data[ridx(rx as usize, y as usize, rz as usize)];
        (v >> 4, v & 15)
    }

    /// Свет только центрального чанка в формате `LightData`.
    pub fn center(&self) -> Vec<u8> {
        let mut out = vec![0u8; CHUNK_W * CHUNK_W * CHUNK_H];
        for y in 0..CHUNK_H {
            for z in 0..CHUNK_W {
                let src = ridx(MARGIN as usize, y, z + MARGIN as usize);
                let dst = (y << 8) | (z << 4);
                out[dst..dst + CHUNK_W].copy_from_slice(&self.data[src..src + CHUNK_W]);
            }
        }
        out
    }
}

/// Свойства ячейки для распространения света.
#[inline(always)]
fn props(v: u16) -> (bool, u8, u8) {
    let d = def(block_id(v));
    (d.opaque, d.absorb, d.light)
}

pub fn compute(nb: &Neighborhood, top: usize) -> LightField {
    let w = REGION_W;
    let h = CHUNK_H;
    let mut sky = vec![0u8; w * w * h];
    let mut blk = vec![0u8; w * w * h];
    // Кэш «непрозрачность/поглощение» для области — ускоряет BFS.
    let mut cell = vec![0u8; w * w * h]; // бит 7 — непрозрачный, младшие — поглощение
    let ymax = (top + 2).min(h);

    let mut queue: VecDeque<(u16, u8, u16)> = VecDeque::with_capacity(1 << 16);
    let mut bqueue: VecDeque<(u16, u8, u16)> = VecDeque::new();

    for rz in 0..w {
        for rx in 0..w {
            let x = rx as i32 - MARGIN;
            let z = rz as i32 - MARGIN;
            let mut level = 15u8;
            for y in (0..h).rev() {
                let i = ridx(rx, y, rz);
                if y >= ymax {
                    sky[i] = 15;
                    continue;
                }
                let v = nb.get(x, y as i32, z);
                let (opaque, absorb, emit) = props(v);
                cell[i] = if opaque { 0x80 } else { absorb };
                if opaque {
                    level = 0;
                } else if absorb > 0 {
                    level = level.saturating_sub(absorb + 1).max(0);
                }
                sky[i] = level;
                if emit > 0 {
                    blk[i] = emit;
                    bqueue.push_back((rx as u16, y as u8, rz as u16));
                }
            }
        }
    }

    // Затравка для бокового распространения неба: освещённые ячейки, у которых
    // есть сосед темнее более чем на 1.
    for rz in 0..w {
        for rx in 0..w {
            for y in 0..ymax {
                let i = ridx(rx, y, rz);
                let l = sky[i];
                if l <= 1 {
                    continue;
                }
                let mut needs = false;
                for (dx, dy, dz) in DIRS {
                    let nx = rx as i32 + dx;
                    let ny = y as i32 + dy;
                    let nz = rz as i32 + dz;
                    if nx < 0 || nz < 0 || ny < 0 || nx >= w as i32 || nz >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = ridx(nx as usize, ny as usize, nz as usize);
                    if cell[j] & 0x80 == 0 && sky[j] + 1 < l {
                        needs = true;
                        break;
                    }
                }
                if needs {
                    queue.push_back((rx as u16, y as u8, rz as u16));
                }
            }
        }
    }

    propagate(&mut sky, &cell, &mut queue);
    propagate(&mut blk, &cell, &mut bqueue);

    let mut data = vec![0u8; w * w * h];
    for i in 0..data.len() {
        data[i] = (sky[i] << 4) | blk[i];
    }
    LightField { data }
}

const DIRS: [(i32, i32, i32); 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];

fn propagate(light: &mut [u8], cell: &[u8], queue: &mut VecDeque<(u16, u8, u16)>) {
    let w = REGION_W as i32;
    let h = CHUNK_H as i32;
    while let Some((x, y, z)) = queue.pop_front() {
        let l = light[ridx(x as usize, y as usize, z as usize)];
        if l <= 1 {
            continue;
        }
        for (dx, dy, dz) in DIRS {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            let nz = z as i32 + dz;
            if nx < 0 || nz < 0 || ny < 0 || nx >= w || nz >= w || ny >= h {
                continue;
            }
            let j = ridx(nx as usize, ny as usize, nz as usize);
            let c = cell[j];
            if c & 0x80 != 0 {
                continue;
            }
            let nl = l.saturating_sub(1 + c);
            if nl > light[j] {
                light[j] = nl;
                queue.push_back((nx as u16, ny as u8, nz as u16));
            }
        }
    }
}
