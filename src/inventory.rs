//! Инвентарь: стаки предметов, слоты, перекладывание.

use serde::{Deserialize, Serialize};

use crate::item::{self, ItemId, ItemKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u8,
    /// Оставшаяся прочность (для инструментов), иначе 0.
    pub durability: u16,
}

impl ItemStack {
    pub fn new(item: ItemId, count: u8) -> Self {
        let durability = match item::def(item).map(|d| d.kind) {
            Some(ItemKind::Tool { durability, .. }) => durability,
            Some(ItemKind::Resonator { durability }) => durability,
            _ => 0,
        };
        Self { item, count, durability }
    }

    pub fn max_stack(&self) -> u8 {
        item::max_stack(self.item)
    }

    pub fn max_durability(&self) -> u16 {
        match item::def(self.item).map(|d| d.kind) {
            Some(ItemKind::Tool { durability, .. }) => durability,
            Some(ItemKind::Resonator { durability }) => durability,
            _ => 0,
        }
    }

    /// Можно ли сложить в одну ячейку (одинаковый предмет, не инструмент).
    pub fn stackable_with(&self, other: &ItemStack) -> bool {
        self.item == other.item && self.max_stack() > 1 && self.durability == other.durability
    }
}

pub type Slot = Option<ItemStack>;

pub const HOTBAR: usize = 9;
pub const MAIN: usize = 27;
pub const SIZE: usize = HOTBAR + MAIN;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    /// 0..9 — хотбар, 9..36 — основной инвентарь.
    pub slots: Vec<Slot>,
    pub selected: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Self { slots: vec![None; SIZE], selected: 0 }
    }
}

impl Inventory {
    pub fn selected_stack(&self) -> Slot {
        self.slots[self.selected]
    }

    /// Добавляет предметы; возвращает остаток, который не поместился.
    pub fn add(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        // Сначала дополняем существующие стаки (хотбар приоритетнее).
        for slot in self.slots.iter_mut() {
            if let Some(s) = slot {
                if s.stackable_with(&stack) && s.count < s.max_stack() {
                    let room = s.max_stack() - s.count;
                    let n = room.min(stack.count);
                    s.count += n;
                    stack.count -= n;
                    if stack.count == 0 {
                        return None;
                    }
                }
            }
        }
        for slot in self.slots.iter_mut() {
            if slot.is_none() {
                let n = stack.count.min(stack.max_stack());
                *slot = Some(ItemStack { count: n, ..stack });
                stack.count -= n;
                if stack.count == 0 {
                    return None;
                }
            }
        }
        Some(stack)
    }

    /// Сколько предметов данного типа есть.
    pub fn count(&self, item: ItemId) -> u32 {
        self.slots.iter().flatten().filter(|s| s.item == item).map(|s| s.count as u32).sum()
    }

    /// Забирает `n` предметов типа `item`; false — если не хватило (ничего не меняется).
    pub fn take(&mut self, item: ItemId, n: u32) -> bool {
        if self.count(item) < n {
            return false;
        }
        let mut left = n;
        for slot in self.slots.iter_mut() {
            if left == 0 {
                break;
            }
            if let Some(s) = slot {
                if s.item == item {
                    let k = (s.count as u32).min(left);
                    s.count -= k as u8;
                    left -= k;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        true
    }

    /// Уменьшает выбранный стак на 1.
    pub fn consume_selected(&mut self) {
        let sel = self.selected;
        if let Some(s) = &mut self.slots[sel] {
            s.count = s.count.saturating_sub(1);
            if s.count == 0 {
                self.slots[sel] = None;
            }
        }
    }

    /// Изнашивает инструмент в выбранном слоте; true — если он сломался.
    pub fn damage_selected(&mut self, amount: u16) -> bool {
        let sel = self.selected;
        if let Some(s) = &mut self.slots[sel] {
            if s.max_durability() == 0 {
                return false;
            }
            s.durability = s.durability.saturating_sub(amount);
            if s.durability == 0 {
                self.slots[sel] = None;
                return true;
            }
        }
        false
    }
}

/// Операции «курсора» в окне инвентаря: ЛКМ — взять/положить/обменять стак,
/// ПКМ — взять половину / положить один предмет.
pub fn click_slot(slot: &mut Slot, cursor: &mut Slot, right: bool) {
    match (slot.as_mut(), cursor.as_mut()) {
        (None, None) => {}
        (Some(s), None) => {
            if right {
                let half = s.count.div_ceil(2);
                *cursor = Some(ItemStack { count: half, ..*s });
                s.count -= half;
                if s.count == 0 {
                    *slot = None;
                }
            } else {
                *cursor = slot.take();
            }
        }
        (None, Some(c)) => {
            if right {
                *slot = Some(ItemStack { count: 1, ..*c });
                c.count -= 1;
                if c.count == 0 {
                    *cursor = None;
                }
            } else {
                *slot = cursor.take();
            }
        }
        (Some(s), Some(c)) => {
            if s.stackable_with(c) {
                let room = s.max_stack().saturating_sub(s.count);
                let n = if right { 1.min(room) } else { room.min(c.count) };
                s.count += n;
                c.count -= n;
                if c.count == 0 {
                    *cursor = None;
                }
            } else if !right {
                std::mem::swap(slot, cursor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_take() {
        let mut inv = Inventory::default();
        assert!(inv.add(ItemStack::new(item::id::COBBLE, 100)).is_none());
        assert_eq!(inv.count(item::id::COBBLE), 100);
        assert_eq!(inv.slots[0].map(|s| s.count), Some(64));
        assert!(inv.take(item::id::COBBLE, 70));
        assert_eq!(inv.count(item::id::COBBLE), 30);
        assert!(!inv.take(item::id::COBBLE, 31));
    }

    #[test]
    fn split_stack() {
        let mut slot = Some(ItemStack::new(item::id::DIRT, 9));
        let mut cursor = None;
        click_slot(&mut slot, &mut cursor, true);
        assert_eq!(cursor.map(|c| c.count), Some(5));
        assert_eq!(slot.map(|c| c.count), Some(4));
        let mut empty = None;
        click_slot(&mut empty, &mut cursor, true);
        assert_eq!(empty.map(|c| c.count), Some(1));
        assert_eq!(cursor.map(|c| c.count), Some(4));
    }
}
