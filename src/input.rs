//! Состояние ввода за кадр: нажатые клавиши/кнопки, движение мыши, колесо,
//! введённый текст.

use std::collections::HashSet;

use winit::keyboard::KeyCode;

use crate::settings::{Action, Controls, Input};

#[derive(Default)]
pub struct InputState {
    down: HashSet<Input>,
    pressed: HashSet<Input>,
    released: HashSet<Input>,
    pub mouse_delta: (f64, f64),
    pub scroll: f32,
    pub mouse_pos: (f32, f32),
    pub typed: Vec<char>,
    /// Последняя нажатая клавиша/кнопка (для экрана переназначения).
    pub last_pressed: Option<Input>,
}

impl InputState {
    pub fn on_press(&mut self, i: Input) {
        if self.down.insert(i) {
            self.pressed.insert(i);
        }
        self.last_pressed = Some(i);
    }

    pub fn on_release(&mut self, i: Input) {
        self.down.remove(&i);
        self.released.insert(i);
    }

    pub fn clear_all(&mut self) {
        self.down.clear();
        self.pressed.clear();
        self.released.clear();
    }

    pub fn end_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.mouse_delta = (0.0, 0.0);
        self.scroll = 0.0;
        self.typed.clear();
        self.last_pressed = None;
    }

    pub fn key_down(&self, k: KeyCode) -> bool {
        self.down.contains(&Input::Key(k))
    }

    pub fn key_pressed(&self, k: KeyCode) -> bool {
        self.pressed.contains(&Input::Key(k))
    }

    pub fn mouse_down(&self, b: u8) -> bool {
        self.down.contains(&Input::Mouse(b))
    }

    pub fn mouse_pressed(&self, b: u8) -> bool {
        self.pressed.contains(&Input::Mouse(b))
    }

    pub fn mouse_released(&self, b: u8) -> bool {
        self.released.contains(&Input::Mouse(b))
    }

    pub fn down(&self, c: &Controls, a: Action) -> bool {
        self.down.contains(&c.binding(a))
    }

    pub fn pressed(&self, c: &Controls, a: Action) -> bool {
        self.pressed.contains(&c.binding(a))
    }
}
