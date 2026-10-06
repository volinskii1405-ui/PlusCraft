//! Свет как ресурс (уникальная механика): факелы выгорают и требуют
//! топлива (уголь/древесный уголь — ПКМ по факелу), а в абсолютной темноте
//! игрок получает дебафф «страх темноты»: замедление и ускоренный голод.
//! Мракоеды появляются только во тьме и гасят факелы.

use std::collections::HashMap;

use glam::IVec3;
use serde::{Deserialize, Serialize};

use crate::world::block::{id, make};
use crate::world::World;

#[derive(Default, Serialize, Deserialize)]
pub struct TorchFuel {
    /// Позиция факела -> оставшиеся секунды горения.
    pub map: HashMap<(i32, i32, i32), f32>,
}

impl TorchFuel {
    pub fn place(&mut self, p: IVec3, seconds: f32) {
        self.map.insert((p.x, p.y, p.z), seconds);
    }

    pub fn remove(&mut self, p: IVec3) {
        self.map.remove(&(p.x, p.y, p.z));
    }

    pub fn get(&self, p: IVec3) -> Option<f32> {
        self.map.get(&(p.x, p.y, p.z)).copied()
    }

    /// Добавляет топливо (не больше `max`).
    pub fn refuel(&mut self, p: IVec3, add: f32, max: f32) {
        let e = self.map.entry((p.x, p.y, p.z)).or_insert(0.0);
        *e = (*e + add).min(max);
    }

    /// Сжигает топливо; погасшие факелы становятся прогоревшими.
    /// Возвращает число погасших факелов.
    pub fn tick(&mut self, world: &mut World, dt: f32) -> usize {
        let mut out = Vec::new();
        self.map.retain(|&(x, y, z), left| {
            if !world.is_loaded_at(x, z) {
                return true; // чанк выгружен — время замирает
            }
            if world.get_id(x, y, z) != id::TORCH {
                return false; // факел убрали
            }
            *left -= dt;
            if *left <= 0.0 {
                out.push(IVec3::new(x, y, z));
                false
            } else {
                true
            }
        });
        for p in &out {
            world.set(p.x, p.y, p.z, make(id::BURNT_TORCH, 0));
        }
        out.len()
    }
}

/// Дебафф абсолютной темноты.
#[derive(Default)]
pub struct Darkness {
    /// Сколько секунд игрок провёл в полной темноте.
    pub time: f32,
    pub warned: bool,
}

impl Darkness {
    pub const ONSET: f32 = 4.0;

    pub fn active(&self) -> bool {
        self.time >= Self::ONSET
    }

    /// Обновление по уровню света у глаз игрока (0..15).
    pub fn update(&mut self, light: u8, dt: f32) -> bool {
        if light == 0 {
            self.time += dt;
        } else {
            self.time = (self.time - dt * 3.0).max(0.0);
            if self.time == 0.0 {
                self.warned = false;
            }
        }
        if self.active() && !self.warned {
            self.warned = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::{Chunk, ChunkData, ChunkPos};

    #[test]
    fn torch_burns_out_and_refuels() {
        let mut w = World::new(1, None, false);
        let p0 = ChunkPos::new(0, 0);
        w.chunks.insert(p0, Chunk::new(p0, ChunkData::empty()));
        let mut f = TorchFuel::default();
        let a = IVec3::new(1, 20, 1);
        let b = IVec3::new(3, 20, 3);
        w.set(a.x, a.y, a.z, make(id::TORCH, 255));
        w.set(b.x, b.y, b.z, make(id::TORCH, 255));
        f.place(a, 10.0);
        f.place(b, 10.0);
        f.tick(&mut w, 6.0);
        f.refuel(b, 10.0, 15.0);
        assert_eq!(f.tick(&mut w, 6.0), 1);
        assert_eq!(w.get_id(a.x, a.y, a.z), id::BURNT_TORCH);
        assert_eq!(w.get_id(b.x, b.y, b.z), id::TORCH);
        assert!((f.get(b).unwrap() - 8.0).abs() < 1e-3);
    }

    #[test]
    fn darkness_debuff_onset() {
        let mut d = Darkness::default();
        assert!(!d.update(0, 2.0));
        assert!(d.update(0, 2.5), "предупреждение при наступлении");
        assert!(d.active());
        d.update(10, 5.0);
        assert!(!d.active());
    }
}
