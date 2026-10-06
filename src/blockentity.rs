//! Блоки с содержимым: печи и сундуки.

use std::collections::HashMap;

use glam::IVec3;
use serde::{Deserialize, Serialize};

use crate::crafting::Recipes;
use crate::inventory::{ItemStack, Slot};
use crate::item::{self, id as it};
use crate::world::block::{id, ToolKind};
use crate::world::noise::{hash3, Rng};

pub const CHEST_SLOTS: usize = 27;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Furnace {
    pub input: Slot,
    pub fuel: Slot,
    pub output: Slot,
    /// Оставшееся время горения текущего топлива, с.
    pub burn_left: f32,
    pub burn_max: f32,
    /// Прогресс плавки текущего предмета, с.
    pub progress: f32,
}

impl Furnace {
    pub fn is_burning(&self) -> bool {
        self.burn_left > 0.0
    }

    /// Шаг печи; возвращает true, если изменилось состояние «горит/не горит».
    pub fn tick(&mut self, recipes: &Recipes, dt: f32) -> bool {
        let was = self.is_burning();
        let recipe = self.input.and_then(|s| recipes.smelt(s.item));
        let can_output = |out: crate::item::ItemId, slot: &Slot| match slot {
            None => true,
            Some(s) => s.item == out && s.count < s.max_stack(),
        };
        let smeltable = recipe.map(|r| can_output(r.output, &self.output)).unwrap_or(false);

        // Подкидываем топливо, если есть что плавить.
        if self.burn_left <= 0.0 && smeltable {
            if let Some(f) = self.fuel {
                if let Some(burn) = recipes.fuel(f.item) {
                    self.burn_left = burn;
                    self.burn_max = burn;
                    let mut f = f;
                    f.count -= 1;
                    self.fuel = if f.count == 0 { None } else { Some(f) };
                }
            }
        }
        if self.burn_left > 0.0 {
            self.burn_left = (self.burn_left - dt).max(0.0);
            if let (true, Some(r)) = (smeltable, recipe) {
                self.progress += dt;
                if self.progress >= r.time {
                    self.progress = 0.0;
                    if let Some(inp) = &mut self.input {
                        inp.count -= 1;
                        if inp.count == 0 {
                            self.input = None;
                        }
                    }
                    match &mut self.output {
                        Some(o) => o.count += 1,
                        None => self.output = Some(ItemStack::new(r.output, 1)),
                    }
                }
            } else {
                self.progress = 0.0;
            }
        } else {
            // Без огня прогресс медленно остывает.
            self.progress = (self.progress - dt * 2.0).max(0.0);
        }
        was != self.is_burning()
    }

    pub fn items(&self) -> Vec<ItemStack> {
        [self.input, self.fuel, self.output].into_iter().flatten().collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chest {
    pub slots: Vec<Slot>,
}

impl Default for Chest {
    fn default() -> Self {
        Self { slots: vec![None; CHEST_SLOTS] }
    }
}

impl Chest {
    /// Лут сундука заброшенной шахты (детерминирован по позиции и seed).
    pub fn mine_loot(seed: u64, pos: IVec3) -> Self {
        let mut rng = Rng::new(hash3(seed ^ 0x100D, pos.x, pos.y, pos.z));
        let table: [(item::ItemId, u8, u8, f32); 14] = [
            (it::COAL, 2, 8, 0.7),
            (it::TORCH, 2, 6, 0.6),
            (it::RAW_IRON, 1, 5, 0.5),
            (it::RAW_COPPER, 2, 7, 0.5),
            (it::IRON_INGOT, 1, 3, 0.3),
            (it::GOLD_INGOT, 1, 3, 0.2),
            (it::DIAMOND, 1, 2, 0.08),
            (it::RESONITE_SHARD, 1, 3, 0.15),
            (it::BREAD, 1, 3, 0.5),
            (it::STICK, 2, 8, 0.4),
            (id::RAIL as item::ItemId, 4, 12, 0.4),
            (id::SUPPORT as item::ItemId, 2, 6, 0.35),
            (it::STRING, 1, 4, 0.3),
            (it::BONE, 1, 4, 0.3),
        ];
        let mut chest = Chest::default();
        for (item_id, lo, hi, p) in table {
            if rng.chance(p) {
                let n = rng.range(lo as i32, hi as i32 + 1) as u8;
                let slot = rng.range(0, CHEST_SLOTS as i32) as usize;
                if chest.slots[slot].is_none() {
                    chest.slots[slot] = Some(ItemStack::new(item_id, n));
                }
            }
        }
        // Изредка — старый инструмент с остатком прочности.
        if rng.chance(0.25) {
            let mut tool = ItemStack::new(item::tool_id(ToolKind::Pickaxe, if rng.chance(0.3) { 4 } else { 2 }), 1);
            tool.durability = (tool.durability as f32 * (0.2 + rng.f32() * 0.6)) as u16;
            let slot = rng.range(0, CHEST_SLOTS as i32) as usize;
            chest.slots[slot].get_or_insert(tool);
        }
        chest
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockEntity {
    Furnace(Furnace),
    Chest(Chest),
}

/// Хранилище блоков-сущностей по мировым координатам.
#[derive(Default, Serialize, Deserialize)]
pub struct BlockEntities {
    pub map: HashMap<(i32, i32, i32), BlockEntity>,
}

impl BlockEntities {
    pub fn furnace_mut(&mut self, p: IVec3) -> &mut Furnace {
        let e = self.map.entry((p.x, p.y, p.z)).or_insert_with(|| BlockEntity::Furnace(Furnace::default()));
        if !matches!(e, BlockEntity::Furnace(_)) {
            *e = BlockEntity::Furnace(Furnace::default());
        }
        match e {
            BlockEntity::Furnace(f) => f,
            BlockEntity::Chest(_) => unreachable!("тип исправлен выше"),
        }
    }

    pub fn chest_mut(&mut self, p: IVec3, init: impl FnOnce() -> Chest) -> &mut Chest {
        let e = self.map.entry((p.x, p.y, p.z)).or_insert_with(|| BlockEntity::Chest(init()));
        if !matches!(e, BlockEntity::Chest(_)) {
            *e = BlockEntity::Chest(Chest::default());
        }
        match e {
            BlockEntity::Chest(c) => c,
            BlockEntity::Furnace(_) => unreachable!("тип исправлен выше"),
        }
    }

    /// Удаляет сущность и возвращает её содержимое.
    pub fn remove(&mut self, p: IVec3) -> Vec<ItemStack> {
        match self.map.remove(&(p.x, p.y, p.z)) {
            Some(BlockEntity::Furnace(f)) => f.items(),
            Some(BlockEntity::Chest(c)) => c.slots.into_iter().flatten().collect(),
            None => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn furnace_smelts_with_fuel() {
        let r = Recipes::load();
        let mut f = Furnace { input: Some(ItemStack::new(it::RAW_IRON, 2)), fuel: Some(ItemStack::new(it::COAL, 1)), ..Default::default() };
        let mut t = 0.0;
        while t < 20.0 {
            f.tick(&r, 0.05);
            t += 0.05;
        }
        assert_eq!(f.output.map(|s| (s.item, s.count)), Some((it::IRON_INGOT, 2)));
        assert!(f.input.is_none());
        assert!(f.fuel.is_none());
    }

    #[test]
    fn mine_loot_is_deterministic() {
        let a = Chest::mine_loot(7, IVec3::new(10, 30, -5));
        let b = Chest::mine_loot(7, IVec3::new(10, 30, -5));
        assert_eq!(a.slots, b.slots);
        // В среднем сундуки шахт не пустые.
        let filled = (0..50).filter(|i| Chest::mine_loot(7, IVec3::new(*i, 30, 0)).slots.iter().any(|s| s.is_some())).count();
        assert!(filled > 40, "непустых сундуков {filled}/50");
    }

    #[test]
    fn furnace_without_fuel_does_nothing() {
        let r = Recipes::load();
        let mut f = Furnace { input: Some(ItemStack::new(it::RAW_IRON, 1)), ..Default::default() };
        for _ in 0..400 {
            f.tick(&r, 0.05);
        }
        assert!(f.output.is_none());
    }
}
