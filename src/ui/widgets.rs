//! Простые виджеты immediate-mode: кнопки, ползунки, переключатели.

use crate::input::InputState;

use super::draw::{Color, UiBatch, WHITE};

pub fn hovered(input: &InputState, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (mx, my) = input.mouse_pos;
    mx >= x && mx < x + w && my >= y && my < y + h
}

/// Кнопка; возвращает true при клике.
pub fn button(ui: &mut UiBatch, input: &InputState, x: f32, y: f32, w: f32, h: f32, label: &str) -> bool {
    button_ex(ui, input, x, y, w, h, label, true)
}

#[allow(clippy::too_many_arguments)]
pub fn button_ex(ui: &mut UiBatch, input: &InputState, x: f32, y: f32, w: f32, h: f32, label: &str, enabled: bool) -> bool {
    let hov = enabled && hovered(input, x, y, w, h);
    let s = ui.scale;
    let (bg, edge): (Color, Color) = if !enabled {
        ([45, 42, 52, 230], [70, 66, 80, 255])
    } else if hov {
        ([92, 86, 120, 245], [190, 180, 240, 255])
    } else {
        ([62, 58, 78, 240], [110, 104, 132, 255])
    };
    ui.rect(x, y, w, h, bg);
    ui.frame(x, y, w, h, 2.0 * s, edge);
    let col = if enabled { WHITE } else { [140, 140, 150, 255] };
    let th = ui.line_height(1.0);
    ui.text_centered(x + w / 2.0, y + (h - th) / 2.0, label, 1.0, col);
    hov && input.mouse_pressed(0)
}

/// Ползунок; возвращает новое значение в [min, max].
#[allow(clippy::too_many_arguments)]
pub fn slider(ui: &mut UiBatch, input: &InputState, x: f32, y: f32, w: f32, h: f32, label: &str, value: f64, min: f64, max: f64) -> f64 {
    let s = ui.scale;
    let hov = hovered(input, x, y, w, h);
    ui.rect(x, y, w, h, [40, 38, 50, 240]);
    ui.frame(x, y, w, h, 2.0 * s, if hov { [190, 180, 240, 255] } else { [110, 104, 132, 255] });
    let t = ((value - min) / (max - min)).clamp(0.0, 1.0) as f32;
    let kx = x + 4.0 * s + t * (w - 16.0 * s);
    ui.rect(kx, y + 2.0 * s, 8.0 * s, h - 4.0 * s, [200, 195, 230, 255]);
    let th = ui.line_height(1.0);
    ui.text_centered(x + w / 2.0, y + (h - th) / 2.0, label, 1.0, WHITE);
    if hov && input.mouse_down(0) {
        let nt = ((input.mouse_pos.0 - x - 8.0 * s) / (w - 16.0 * s)).clamp(0.0, 1.0) as f64;
        return min + nt * (max - min);
    }
    value
}
