//! Экраны с инвентарём: инвентарь игрока (крафт 2×2 или палитра креатива),
//! верстак (3×3), печь, сундук. Immediate-mode: отрисовка и обработка
//! кликов в одном проходе.

use glam::IVec3;
use winit::keyboard::KeyCode;

use crate::blockentity::{Chest, CHEST_SLOTS};
use crate::crafting;
use crate::game::Game;
use crate::input::InputState;
use crate::inventory::{click_slot, ItemStack, Slot, HOTBAR, SIZE};
use crate::item;
use crate::world::block::{self, block_id, block_meta, id};
use crate::world::mines::CHEST_LOOT_FLAG;

use super::draw::{UiBatch, WHITE};
use super::hud::draw_stack;
use super::widgets::hovered;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Inventory,
    Crafting(IVec3),
    Furnace(IVec3),
    Chest(IVec3),
}

/// Состояние открытого экрана.
pub struct ContainerUi {
    pub screen: Screen,
    pub grid: Vec<Slot>,
    pub cursor: Slot,
    pub creative_scroll: usize,
    /// Экран открыт в этом кадре — не закрывать той же клавишей.
    pub fresh: bool,
}

impl ContainerUi {
    pub fn new(screen: Screen) -> Self {
        let n = match screen {
            Screen::Crafting(_) => 9,
            _ => 4,
        };
        Self { screen, grid: vec![None; n], cursor: None, creative_scroll: 0, fresh: true }
    }
    fn grid_size(&self) -> usize {
        if self.grid.len() == 9 {
            3
        } else {
            2
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SlotRef {
    Inv(usize),
    Grid(usize),
    Result,
    FurnIn,
    FurnFuel,
    FurnOut,
    Chest(usize),
    Creative(item::ItemId),
}

/// Открывает экран; для сундуков шахт генерирует лут при первом открытии.
pub fn open(game: &mut Game, screen: Screen) {
    if let Screen::Chest(p) = screen {
        let v = game.world.get(p.x, p.y, p.z);
        let loot = block_meta(v) & CHEST_LOOT_FLAG != 0;
        let seed = game.world.seed;
        game.block_entities.chest_mut(p, || if loot { Chest::mine_loot(seed, p) } else { Chest::default() });
        if loot {
            game.world.set(p.x, p.y, p.z, block::make(id::CHEST, block_meta(v) & !CHEST_LOOT_FLAG));
        }
    }
    game.container = Some(ContainerUi::new(screen));
}

/// Закрывает экран: предметы с сетки крафта и курсора возвращаются в инвентарь.
pub fn close(game: &mut Game) {
    let Some(c) = game.container.take() else { return };
    let mut back: Vec<ItemStack> = c.grid.into_iter().flatten().collect();
    back.extend(c.cursor);
    for s in back {
        if let Some(rest) = game.player.inventory.add(s) {
            game.throw_stack(rest);
        }
    }
}

fn slot_mut<'a>(game: &'a mut Game, r: SlotRef) -> Option<&'a mut Slot> {
    match r {
        SlotRef::Inv(i) => game.player.inventory.slots.get_mut(i),
        SlotRef::Grid(i) => game.container.as_mut()?.grid.get_mut(i),
        SlotRef::Chest(i) => {
            let Screen::Chest(p) = game.container.as_ref()?.screen else { return None };
            game.block_entities.chest_mut(p, Chest::default).slots.get_mut(i)
        }
        SlotRef::FurnIn | SlotRef::FurnFuel | SlotRef::FurnOut => {
            let Screen::Furnace(p) = game.container.as_ref()?.screen else { return None };
            let f = game.block_entities.furnace_mut(p);
            Some(match r {
                SlotRef::FurnIn => &mut f.input,
                SlotRef::FurnFuel => &mut f.fuel,
                _ => &mut f.output,
            })
        }
        SlotRef::Result | SlotRef::Creative(_) => None,
    }
}

fn slot_value(game: &mut Game, r: SlotRef) -> Slot {
    match r {
        SlotRef::Result => craft_result(game),
        SlotRef::Creative(it) => Some(ItemStack::new(it, item::max_stack(it))),
        _ => slot_mut(game, r).and_then(|s| *s),
    }
}

fn craft_result(game: &Game) -> Slot {
    let c = game.container.as_ref()?;
    game.recipes.match_grid(&c.grid, c.grid_size())
}

/// Перемещает стак в первый подходящий слот диапазона; возвращает остаток.
fn move_into(game: &mut Game, mut stack: ItemStack, targets: &[SlotRef]) -> Option<ItemStack> {
    for pass in 0..2 {
        for &t in targets {
            let Some(slot) = slot_mut(game, t) else { continue };
            match slot {
                Some(s) if pass == 0 && s.stackable_with(&stack) && s.count < s.max_stack() => {
                    let n = (s.max_stack() - s.count).min(stack.count);
                    s.count += n;
                    stack.count -= n;
                }
                None if pass == 1 => {
                    *slot = Some(stack);
                    return None;
                }
                _ => {}
            }
            if stack.count == 0 {
                return None;
            }
        }
    }
    Some(stack)
}

fn inv_range(a: usize, b: usize) -> Vec<SlotRef> {
    (a..b).map(SlotRef::Inv).collect()
}

/// Shift+клик: быстрый перенос между инвентарём и контейнером.
fn quick_move(game: &mut Game, r: SlotRef) {
    let screen = match &game.container {
        Some(c) => c.screen,
        None => return,
    };
    if r == SlotRef::Result {
        // Крафтим максимум, сколько позволяют ингредиенты и место.
        for _ in 0..64 {
            let Some(res) = craft_result(game) else { break };
            let targets: Vec<SlotRef> = inv_range(0, SIZE);
            let mut probe = game.player.inventory.clone();
            if probe.add(res).is_some() {
                break;
            }
            if move_into(game, res, &targets).is_some() {
                break;
            }
            if let Some(c) = game.container.as_mut() {
                crafting::consume_grid(&mut c.grid);
            }
        }
        return;
    }
    let Some(stack) = slot_value(game, r) else { return };
    let targets: Vec<SlotRef> = match (r, screen) {
        (SlotRef::Inv(i), Screen::Chest(_)) => {
            let _ = i;
            (0..CHEST_SLOTS).map(SlotRef::Chest).collect()
        }
        (SlotRef::Inv(_), Screen::Furnace(_)) => {
            if game.recipes.fuel(stack.item).is_some() && game.recipes.smelt(stack.item).is_none() {
                vec![SlotRef::FurnFuel]
            } else {
                vec![SlotRef::FurnIn]
            }
        }
        (SlotRef::Inv(i), _) => {
            if i < HOTBAR {
                inv_range(HOTBAR, SIZE)
            } else {
                inv_range(0, HOTBAR)
            }
        }
        _ => {
            let mut v = inv_range(0, HOTBAR);
            v.extend(inv_range(HOTBAR, SIZE));
            v
        }
    };
    if let Some(slot) = slot_mut(game, r) {
        *slot = None;
    }
    let rest = move_into(game, stack, &targets);
    if let Some(rest) = rest {
        if let Some(slot) = slot_mut(game, r) {
            *slot = Some(rest);
        }
    }
}

fn click(game: &mut Game, r: SlotRef, right: bool, shift: bool) {
    let creative = game.player.creative();
    if shift && !matches!(r, SlotRef::Creative(_)) {
        quick_move(game, r);
        return;
    }
    match r {
        SlotRef::Creative(it) => {
            let Some(c) = game.container.as_mut() else { return };
            if c.cursor.is_some() {
                c.cursor = None; // палитра — «корзина»
            } else {
                let n = if right { 1 } else { item::max_stack(it) };
                c.cursor = Some(ItemStack::new(it, n));
            }
        }
        SlotRef::Result => {
            let Some(res) = craft_result(game) else { return };
            let Some(c) = game.container.as_mut() else { return };
            match &mut c.cursor {
                None => c.cursor = Some(res),
                Some(cur) if cur.stackable_with(&res) && cur.count as u16 + res.count as u16 <= cur.max_stack() as u16 => {
                    cur.count += res.count;
                }
                _ => return,
            }
            crafting::consume_grid(&mut c.grid);
        }
        SlotRef::FurnOut => {
            // Из выхода печи можно только забирать.
            let mut cursor = game.container.as_ref().and_then(|c| c.cursor);
            if let Some(slot) = slot_mut(game, r) {
                match (&mut cursor, slot.as_mut()) {
                    (None, Some(_)) => cursor = slot.take(),
                    (Some(cur), Some(s)) if cur.stackable_with(s) => {
                        let n = (cur.max_stack() - cur.count).min(s.count);
                        cur.count += n;
                        s.count -= n;
                        if s.count == 0 {
                            *slot = None;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(c) = game.container.as_mut() {
                c.cursor = cursor;
            }
        }
        _ => {
            let mut cursor = game.container.as_ref().and_then(|c| c.cursor);
            if let Some(slot) = slot_mut(game, r) {
                click_slot(slot, &mut cursor, right);
            }
            if let Some(c) = game.container.as_mut() {
                c.cursor = cursor;
            }
        }
    }
    let _ = creative;
}

/// Отрисовка и обработка экрана. Возвращает false, если экран закрыт.
pub fn update_and_draw(game: &mut Game, ui: &mut UiBatch, input: &InputState, inventory_key: bool) -> bool {
    let Some(screen) = game.container.as_ref().map(|c| c.screen) else { return false };
    let fresh = game.container.as_mut().map(|c| std::mem::replace(&mut c.fresh, false)).unwrap_or(false);
    if !fresh && (input.key_pressed(KeyCode::Escape) || inventory_key) {
        close(game);
        return false;
    }
    // Контейнер исчез (сломали блок) — закрываем.
    let still_there = match screen {
        Screen::Inventory => true,
        Screen::Crafting(p) => game.world.get_id(p.x, p.y, p.z) == id::CRAFTING_TABLE,
        Screen::Furnace(p) => matches!(game.world.get_id(p.x, p.y, p.z), id::FURNACE | id::FURNACE_LIT),
        Screen::Chest(p) => game.world.get_id(p.x, p.y, p.z) == id::CHEST,
    };
    if !still_there {
        close(game);
        return false;
    }

    let s = ui.scale;
    let sz = 36.0 * s;
    let pad = 10.0 * s;
    let creative_palette = screen == Screen::Inventory && game.player.creative();
    let top_h = match screen {
        Screen::Inventory if creative_palette => 5.0 * sz + 30.0 * s,
        Screen::Inventory => 2.0 * sz + 30.0 * s,
        Screen::Crafting(_) | Screen::Furnace(_) => 3.0 * sz + 30.0 * s,
        Screen::Chest(_) => 3.0 * sz + 30.0 * s,
    };
    let w = 9.0 * sz + 2.0 * pad;
    let h = top_h + 4.0 * sz + 3.0 * pad + 24.0 * s;
    let x0 = (ui.width - w) / 2.0;
    let y0 = (ui.height - h) / 2.0;
    ui.rect(0.0, 0.0, ui.width, ui.height, [0, 0, 0, 120]);
    ui.panel(x0, y0, w, h);

    let mut slots: Vec<(SlotRef, f32, f32)> = Vec::new();
    let title = match screen {
        Screen::Inventory if creative_palette => "Креатив: все предметы (колесо — прокрутка)",
        Screen::Inventory => "Инвентарь",
        Screen::Crafting(_) => "Верстак",
        Screen::Furnace(_) => "Печь",
        Screen::Chest(_) => "Сундук",
    };
    ui.text_shadow(x0 + pad, y0 + pad * 0.6, title, 0.9, WHITE);
    let ty = y0 + pad + 22.0 * s;

    match screen {
        Screen::Inventory if creative_palette => {
            let all = item::all_items();
            let rows = all.len().div_ceil(9);
            let max_scroll = rows.saturating_sub(5);
            if let Some(c) = game.container.as_mut() {
                if input.scroll < 0.0 {
                    c.creative_scroll = (c.creative_scroll + 1).min(max_scroll);
                } else if input.scroll > 0.0 {
                    c.creative_scroll = c.creative_scroll.saturating_sub(1);
                }
            }
            let scroll = game.container.as_ref().map(|c| c.creative_scroll).unwrap_or(0);
            for (i, &it) in all.iter().skip(scroll * 9).take(45).enumerate() {
                slots.push((SlotRef::Creative(it), x0 + pad + (i % 9) as f32 * sz, ty + (i / 9) as f32 * sz));
            }
        }
        Screen::Inventory | Screen::Crafting(_) => {
            let n = game.container.as_ref().map(|c| c.grid_size()).unwrap_or(2);
            let gx = x0 + pad + 1.5 * sz;
            for i in 0..n * n {
                slots.push((SlotRef::Grid(i), gx + (i % n) as f32 * sz, ty + (i / n) as f32 * sz));
            }
            let ax = gx + n as f32 * sz + 0.4 * sz;
            let ay = ty + (n as f32 * sz) / 2.0 - 0.5 * sz;
            ui.text_shadow(ax, ay + 6.0 * s, "→", 1.6, [210, 210, 210, 255]);
            slots.push((SlotRef::Result, ax + 1.4 * sz, ay));
        }
        Screen::Furnace(p) => {
            let fx = x0 + pad + 2.5 * sz;
            slots.push((SlotRef::FurnIn, fx, ty));
            slots.push((SlotRef::FurnFuel, fx, ty + 2.0 * sz));
            slots.push((SlotRef::FurnOut, fx + 3.0 * sz, ty + sz));
            let f = game.block_entities.furnace_mut(p).clone();
            // Огонь (остаток топлива).
            let frac = if f.burn_max > 0.0 { f.burn_left / f.burn_max } else { 0.0 };
            let fh = sz * 0.7;
            ui.rect(fx + sz * 0.3, ty + sz + 0.15 * sz, sz * 0.4, fh, [50, 40, 40, 255]);
            ui.rect(fx + sz * 0.3, ty + sz + 0.15 * sz + fh * (1.0 - frac), sz * 0.4, fh * frac, [255, 140, 30, 255]);
            // Прогресс плавки.
            let total = f.input.and_then(|i| game.recipes.smelt(i.item)).map(|r| r.time).unwrap_or(1.0);
            let pr = (f.progress / total).clamp(0.0, 1.0);
            let ax = fx + 1.2 * sz;
            let ay = ty + sz + 0.4 * sz;
            ui.rect(ax, ay, 1.5 * sz, 0.2 * sz, [90, 88, 105, 255]);
            ui.rect(ax, ay, 1.5 * sz * pr, 0.2 * sz, [240, 240, 250, 255]);
            let lab = [200, 200, 215, 255];
            ui.text(fx - 2.3 * sz, ty + 0.25 * sz, "Сырьё", 0.75, lab);
            ui.text(fx - 2.3 * sz, ty + 2.25 * sz, "Топливо", 0.75, lab);
            ui.text(fx + 4.2 * sz, ty + 1.25 * sz, "Результат", 0.75, lab);
        }
        Screen::Chest(_) => {
            for i in 0..CHEST_SLOTS {
                slots.push((SlotRef::Chest(i), x0 + pad + (i % 9) as f32 * sz, ty + (i / 9) as f32 * sz));
            }
        }
    }

    // Инвентарь игрока: 3 ряда + хотбар.
    let iy = y0 + pad + 22.0 * s + top_h;
    for i in HOTBAR..SIZE {
        let k = i - HOTBAR;
        slots.push((SlotRef::Inv(i), x0 + pad + (k % 9) as f32 * sz, iy + (k / 9) as f32 * sz));
    }
    for i in 0..HOTBAR {
        slots.push((SlotRef::Inv(i), x0 + pad + i as f32 * sz, iy + 3.0 * sz + 6.0 * s));
    }

    // Отрисовка и клики.
    let shift = input.key_down(KeyCode::ShiftLeft) || input.key_down(KeyCode::ShiftRight);
    let mut hovered_item: Option<ItemStack> = None;
    let mut clicked: Option<(SlotRef, bool)> = None;
    for &(r, x, y) in &slots {
        let hov = hovered(input, x, y, sz, sz);
        let big = r == SlotRef::Result || r == SlotRef::FurnOut;
        if big {
            ui.rect(x - 3.0 * s, y - 3.0 * s, sz + 6.0 * s, sz + 6.0 * s, [90, 86, 104, 255]);
        }
        ui.slot(x + 1.0 * s, y + 1.0 * s, sz - 2.0 * s, hov);
        let value = slot_value(game, r);
        if let Some(st) = &value {
            draw_stack(ui, x + 1.0 * s, y + 1.0 * s, sz - 2.0 * s, st);
            if hov {
                hovered_item = Some(*st);
            }
        }
        if hov {
            if input.mouse_pressed(0) {
                clicked = Some((r, false));
            } else if input.mouse_pressed(1) {
                clicked = Some((r, true));
            }
        }
    }
    if let Some((r, right)) = clicked {
        click(game, r, right, shift);
    }

    // Клик вне панели с предметом на курсоре — выбросить.
    if (input.mouse_pressed(0) || input.mouse_pressed(1)) && clicked.is_none() && !hovered(input, x0, y0, w, h) {
        if let Some(c) = game.container.as_mut() {
            if let Some(mut st) = c.cursor.take() {
                if input.mouse_pressed(1) && st.count > 1 {
                    st.count -= 1;
                    c.cursor = Some(st);
                    st.count = 1;
                }
                game.throw_stack(st);
            }
        }
    }

    // Предмет на курсоре.
    let (mx, my) = input.mouse_pos;
    if let Some(st) = game.container.as_ref().and_then(|c| c.cursor) {
        draw_stack(ui, mx - sz * 0.45, my - sz * 0.45, sz * 0.9, &st);
    } else if let Some(st) = hovered_item {
        // Подсказка с названием.
        let mut text = item::name(st.item).to_string();
        let max = st.max_durability();
        if max > 0 {
            text = format!("{text} ({}/{max})", st.durability);
        }
        let tw = ui.text_width(&text, 0.9);
        ui.rect(mx + 12.0 * s, my - 4.0 * s, tw + 10.0 * s, ui.line_height(0.9) + 6.0 * s, [20, 10, 30, 235]);
        ui.text(mx + 17.0 * s, my - 1.0 * s, &text, 0.9, WHITE);
    }
    let _ = block_id(0);
    true
}
