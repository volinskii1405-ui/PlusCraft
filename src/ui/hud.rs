//! HUD: прицел, хотбар, здоровье, голод, воздух, иконки предметов.

use crate::assets::textures::Tex;
use crate::inventory::{ItemStack, HOTBAR};
use crate::item::{self, ItemKind};
use crate::player::{GameMode, Player, MAX_HEALTH, MAX_HUNGER};
use crate::world::block::{self, Shape};

use super::draw::{UiBatch, WHITE};

/// Иконка предмета: полноразмерные блоки — изометрический кубик.
pub fn draw_item_icon(ui: &mut UiBatch, x: f32, y: f32, size: f32, item_id: item::ItemId) {
    let Some(def) = item::def(item_id) else { return };
    if let ItemKind::Block(b) = def.kind {
        let d = block::def(b);
        if d.shape == Shape::Cube {
            let v = block::make(b, 4);
            ui.iso_block(
                x,
                y,
                size,
                block::face_tex(v, 0) as u16,
                block::face_tex(v, 4) as u16,
                block::face_tex(v, 2) as u16,
            );
            return;
        }
    }
    ui.icon(x, y, size, def.tex as u16, WHITE);
}

/// Стак: иконка, количество, полоска прочности.
pub fn draw_stack(ui: &mut UiBatch, x: f32, y: f32, size: f32, stack: &ItemStack) {
    let pad = size * 0.1;
    draw_item_icon(ui, x + pad, y + pad, size - 2.0 * pad, stack.item);
    if stack.count > 1 {
        let t = stack.count.to_string();
        let tw = ui.text_width(&t, 0.8);
        ui.text_shadow(x + size - tw - 2.0 * ui.scale, y + size - ui.line_height(0.8), &t, 0.8, WHITE);
    }
    let max = stack.max_durability();
    if max > 0 && stack.durability < max {
        let frac = stack.durability as f32 / max as f32;
        let bw = size - 2.0 * pad;
        let by = y + size - pad - 3.0 * ui.scale;
        ui.rect(x + pad, by, bw, 2.0 * ui.scale, [0, 0, 0, 255]);
        let col = [((1.0 - frac) * 255.0) as u8, (frac * 255.0) as u8, 40, 255];
        ui.rect(x + pad, by, bw * frac, 1.5 * ui.scale, col);
    }
}

pub fn draw_crosshair(ui: &mut UiBatch) {
    let (w, h, s) = (ui.width, ui.height, ui.scale);
    ui.rect(w / 2.0 - 8.0 * s, h / 2.0 - 1.0 * s, 16.0 * s, 2.0 * s, [255, 255, 255, 190]);
    ui.rect(w / 2.0 - 1.0 * s, h / 2.0 - 8.0 * s, 2.0 * s, 16.0 * s, [255, 255, 255, 190]);
}

pub fn hotbar_geometry(ui: &UiBatch) -> (f32, f32, f32) {
    let size = 40.0 * ui.scale;
    let x0 = ui.width / 2.0 - size * HOTBAR as f32 / 2.0;
    let y0 = ui.height - size - 6.0 * ui.scale;
    (x0, y0, size)
}

pub fn draw_hotbar(ui: &mut UiBatch, player: &Player) {
    let (x0, y0, size) = hotbar_geometry(ui);
    let s = ui.scale;
    ui.rect(x0 - 3.0 * s, y0 - 3.0 * s, size * HOTBAR as f32 + 6.0 * s, size + 6.0 * s, [20, 18, 26, 200]);
    for i in 0..HOTBAR {
        let x = x0 + i as f32 * size;
        ui.slot(x + 1.0 * s, y0 + 1.0 * s, size - 2.0 * s, false);
        if let Some(st) = &player.inventory.slots[i] {
            draw_stack(ui, x + 1.0 * s, y0 + 1.0 * s, size - 2.0 * s, st);
        }
    }
    let sx = x0 + player.inventory.selected as f32 * size;
    ui.frame(sx - 2.0 * s, y0 - 2.0 * s, size + 4.0 * s, size + 4.0 * s, 3.0 * s, [240, 240, 240, 255]);

    // Название выбранного предмета над хотбаром.
    if let Some(st) = player.inventory.selected_stack() {
        let name = item::name(st.item);
        ui.text_centered(ui.width / 2.0, y0 - 64.0 * s, name, 0.9, WHITE);
    }
}

pub fn draw_status(ui: &mut UiBatch, player: &Player) {
    if player.mode == GameMode::Creative {
        return;
    }
    let (x0, y0, size) = hotbar_geometry(ui);
    let s = ui.scale;
    let icon = 16.0 * s;
    let y = y0 - icon - 4.0 * s;
    let shake = if player.hurt_timer > 0.0 { ((player.hurt_timer * 60.0).sin() * 2.0 * s).round() } else { 0.0 };
    // Здоровье слева.
    for i in 0..10 {
        let hp = player.health - i as f32 * 2.0;
        let tex = if hp >= 2.0 {
            Tex::IconHeart
        } else if hp >= 1.0 {
            Tex::IconHeartHalf
        } else {
            Tex::IconHeartEmpty
        };
        let low = player.health <= 4.0 && ((ui.width as i32 + i) % 2 == 0);
        let dy = if low { -1.0 * s } else { 0.0 };
        ui.icon(x0 + i as f32 * icon * 0.95, y + dy + shake, icon, tex as u16, WHITE);
    }
    // Голод справа (заполняется справа налево).
    let right = x0 + size * HOTBAR as f32;
    for i in 0..10 {
        let f = player.hunger - i as f32 * 2.0;
        let tex = if f >= 2.0 {
            Tex::IconFood
        } else if f >= 1.0 {
            Tex::IconFoodHalf
        } else {
            Tex::IconFoodEmpty
        };
        ui.icon(right - (i + 1) as f32 * icon * 0.95, y, icon, tex as u16, WHITE);
    }
    // Воздух под водой.
    if player.air < 10.0 {
        let bubbles = player.air.ceil() as i32;
        for i in 0..bubbles {
            ui.icon(right - (i + 1) as f32 * icon * 0.95, y - icon, icon, Tex::IconBubble as u16, WHITE);
        }
    }
    let _ = (MAX_HEALTH, MAX_HUNGER);
}
