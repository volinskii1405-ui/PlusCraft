//! Динамика воды (уникальная механика): вода течёт по уровням, падает
//! вниз, заполняет шахты; при достаточном давлении столба прорывается через
//! слабые блоки (земля, песок, гравий, глина).
//!
//! Метаданные воды: биты 0..2 — уровень (0 — источник, 1..7 — течение),
//! бит 3 — падающая вода.

use std::collections::{BTreeMap, HashSet};

use glam::IVec3;

use crate::world::block::{block_id, block_meta, def, id, make};
use crate::world::noise::Rng;
use crate::world::World;

const FLOW_DELAY: u64 = 5; // тиков (20 тиков = 1 с)
const MAX_UPDATES_PER_TICK: usize = 300;
const FALLING: u8 = 8;

const HORIZ: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

#[derive(Default)]
pub struct Water {
    tick: u64,
    queue: BTreeMap<u64, Vec<IVec3>>,
    scheduled: HashSet<IVec3>,
    /// Сколько блоков прорвало давлением (для статистики/сообщений).
    pub breakthroughs: u32,
}

fn is_water(v: u16) -> bool {
    block_id(v) == id::WATER
}

/// Уровень для растекания: источник и падающая вода — 0.
fn eff_level(v: u16) -> u8 {
    let m = block_meta(v);
    if m & FALLING != 0 {
        0
    } else {
        m & 7
    }
}

fn is_source(v: u16) -> bool {
    is_water(v) && block_meta(v) & (FALLING | 7) == 0
}

/// Может ли вода занять клетку (воздух, трава, цветы...).
fn can_flow_into(v: u16) -> bool {
    let b = block_id(v);
    b != id::WATER && b != id::LAVA && def(b).replaceable
}

/// Слабые блоки, которые вода может размыть под давлением.
fn weak(b: u8) -> bool {
    matches!(b, id::DIRT | id::SAND | id::GRAVEL | id::CLAY | id::SNOW | id::GRASS)
}

impl Water {
    pub fn schedule(&mut self, p: IVec3, delay: u64) {
        if self.scheduled.insert(p) {
            self.queue.entry(self.tick + delay.max(1)).or_default().push(p);
        }
    }

    /// Изменение блока: будим воду вокруг.
    pub fn notify(&mut self, p: IVec3) {
        self.schedule(p, FLOW_DELAY);
        for d in HORIZ.iter().chain([IVec3::Y, IVec3::NEG_Y].iter()) {
            self.schedule(p + *d, FLOW_DELAY);
        }
    }

    /// Будит воду в кубе радиуса r (например, после взрыва/обвала или рядом с шахтой).
    pub fn wake_area(&mut self, world: &World, c: IVec3, r: i32) {
        for dy in -r..=r {
            for dz in -r..=r {
                for dx in -r..=r {
                    let p = c + IVec3::new(dx, dy, dz);
                    if is_water(world.get(p.x, p.y, p.z)) {
                        self.schedule(p, FLOW_DELAY + (dx.unsigned_abs() as u64 % 3));
                    }
                }
            }
        }
    }

    pub fn pending(&self) -> usize {
        self.scheduled.len()
    }

    /// Один игровой тик (1/20 с).
    pub fn tick(&mut self, world: &mut World, rng: &mut Rng) {
        self.tick += 1;
        let mut budget = MAX_UPDATES_PER_TICK;
        while budget > 0 {
            let Some((&t, _)) = self.queue.iter().next() else { break };
            if t > self.tick {
                break;
            }
            let mut list = self.queue.remove(&t).unwrap_or_default();
            while let Some(p) = list.pop() {
                if budget == 0 {
                    // Остаток — на следующий тик.
                    self.queue.entry(self.tick + 1).or_default().extend(list.drain(..).chain(std::iter::once(p)));
                    break;
                }
                budget -= 1;
                self.scheduled.remove(&p);
                self.update(world, p, rng);
            }
        }
    }

    fn set(&mut self, world: &mut World, p: IVec3, v: u16) {
        if world.set(p.x, p.y, p.z, v) {
            self.notify(p);
        }
    }

    fn update(&mut self, world: &mut World, p: IVec3, rng: &mut Rng) {
        if !world.is_loaded_at(p.x, p.z) || !(1..255).contains(&p.y) {
            return;
        }
        let v = world.get(p.x, p.y, p.z);
        if !is_water(v) {
            return;
        }
        let up = world.get(p.x, p.y + 1, p.z);
        let below = world.get(p.x, p.y - 1, p.z);

        if !is_source(v) {
            // Пересчёт состояния текущей воды.
            let new_meta = if is_water(up) {
                Some(FALLING | 1)
            } else {
                let mut min = 8u8;
                let mut sources = 0;
                for d in HORIZ {
                    let n = world.get(p.x + d.x, p.y, p.z + d.z);
                    if is_water(n) {
                        if is_source(n) {
                            sources += 1;
                        }
                        // Падающая вода растекается как уровень 0 только по опоре.
                        let l = eff_level(n);
                        min = min.min(l);
                    }
                }
                let below_solid = def(block_id(below)).solid || is_source(below);
                if sources >= 2 && below_solid {
                    Some(0) // «бесконечный» источник
                } else if min < 7 {
                    Some(min + 1)
                } else {
                    None
                }
            };
            match new_meta {
                None => {
                    self.set(world, p, 0);
                    return;
                }
                Some(m) if m != block_meta(v) => {
                    self.set(world, p, make(id::WATER, m));
                }
                _ => {}
            }
        }
        let v = world.get(p.x, p.y, p.z);
        if !is_water(v) {
            return;
        }

        // Падение вниз.
        if can_flow_into(below) {
            self.set(world, p - IVec3::Y, make(id::WATER, FALLING | 1));
            return;
        }
        // Растекание в стороны.
        let level = eff_level(v);
        if level < 7 && !is_water(below) || is_source(v) || block_meta(v) & FALLING != 0 {
            for d in HORIZ {
                let q = p + d;
                let n = world.get(q.x, q.y, q.z);
                if can_flow_into(n) {
                    self.set(world, q, make(id::WATER, (level + 1).min(7)));
                } else if is_water(n) && !is_source(n) && eff_level(n) > level + 1 {
                    self.schedule(q, FLOW_DELAY);
                }
            }
        }

        // Давление: столб воды над клеткой размывает слабые соседние блоки.
        if level <= 1 {
            let mut head = 1;
            while head < 10 && is_water(world.get(p.x, p.y + head, p.z)) {
                head += 1;
            }
            if head >= 3 {
                let mut any_weak = false;
                for d in HORIZ.iter().chain(std::iter::once(&IVec3::NEG_Y)) {
                    let q = p + *d;
                    let b = world.get_id(q.x, q.y, q.z);
                    if weak(b) {
                        any_weak = true;
                        if rng.chance(0.04 * head as f32) {
                            self.breakthroughs += 1;
                            self.set(world, q, make(id::WATER, 1));
                        }
                    }
                }
                // Давление сохраняется — проверим снова позже.
                if any_weak {
                    self.schedule(p, 20 + rng.range(0, 40) as u64);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::{Chunk, ChunkData, ChunkPos};

    fn flat() -> World {
        let mut w = World::new(1, None, false);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut d = ChunkData::empty();
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 1..=10 {
                            d.set(x, y, z, make(id::STONE, 0));
                        }
                    }
                }
                let p = ChunkPos::new(cx, cz);
                w.chunks.insert(p, Chunk::new(p, d));
            }
        }
        w
    }

    fn run(water: &mut Water, w: &mut World, ticks: usize) {
        let mut rng = Rng::new(9);
        for _ in 0..ticks {
            water.tick(w, &mut rng);
        }
    }

    #[test]
    fn spreads_with_levels() {
        let mut w = flat();
        let mut water = Water::default();
        w.set(0, 11, 0, make(id::WATER, 0));
        water.notify(IVec3::new(0, 11, 0));
        run(&mut water, &mut w, 200);
        assert_eq!(w.get(1, 11, 0), make(id::WATER, 1));
        assert_eq!(w.get(4, 11, 0), make(id::WATER, 4));
        assert_eq!(w.get(7, 11, 0), make(id::WATER, 7));
        assert_eq!(w.get_id(8, 11, 0), id::AIR, "дальше 7 блоков вода не течёт");
    }

    #[test]
    fn falls_into_pit_and_dries_when_source_removed() {
        let mut w = flat();
        let mut water = Water::default();
        for y in 7..=10 {
            w.set(2, y, 0, 0); // яма глубиной 4 рядом с источником
        }
        w.set(0, 11, 0, make(id::WATER, 0));
        water.notify(IVec3::new(0, 11, 0));
        run(&mut water, &mut w, 200);
        assert_eq!(w.get_id(2, 7, 0), id::WATER, "вода стекла на дно ямы");
        // Убираем источник — течение пересыхает.
        w.set(0, 11, 0, 0);
        water.notify(IVec3::new(0, 11, 0));
        run(&mut water, &mut w, 400);
        assert_eq!(w.get_id(1, 11, 0), id::AIR);
        assert_eq!(w.get_id(2, 11, 0), id::AIR);
    }

    #[test]
    fn pressure_breaks_weak_block() {
        let mut w = flat();
        let mut water = Water::default();
        // Колодец 1×1 глубиной 5, заполненный источниками; сбоку у дна — земля.
        for y in 6..=10 {
            w.set(0, y, 0, make(id::WATER, 0));
        }
        w.set(1, 6, 0, make(id::DIRT, 0));
        w.set(2, 6, 0, 0); // за землёй — пустота
        water.wake_area(&w, IVec3::new(0, 8, 0), 3);
        run(&mut water, &mut w, 2000);
        assert_eq!(w.get_id(1, 6, 0), id::WATER, "земля должна быть размыта давлением");
        assert!(water.breakthroughs >= 1);
    }

    #[test]
    fn low_pressure_does_not_break() {
        let mut w = flat();
        let mut water = Water::default();
        w.set(0, 10, 0, make(id::WATER, 0));
        w.set(1, 10, 0, make(id::DIRT, 0));
        water.wake_area(&w, IVec3::new(0, 10, 0), 2);
        run(&mut water, &mut w, 2000);
        assert_eq!(w.get_id(1, 10, 0), id::DIRT);
    }
}
