//! Консоль команд: строка ввода (клавиша T), история и вывод.

use winit::keyboard::KeyCode;

use crate::input::InputState;

use super::draw::{UiBatch, WHITE};

#[derive(Default)]
pub struct Console {
    pub open: bool,
    pub text: String,
    history: Vec<String>,
    history_pos: Option<usize>,
    /// Пропустить символы, набранные в кадре открытия (сама клавиша T).
    skip_frame: bool,
}

impl Console {
    pub fn open_with(&mut self, prefix: &str) {
        self.open = true;
        self.text = prefix.to_string();
        self.history_pos = None;
        self.skip_frame = true;
    }

    /// Обрабатывает ввод. Возвращает введённую команду при нажатии Enter.
    pub fn update(&mut self, input: &InputState) -> Option<String> {
        if !self.open {
            return None;
        }
        if self.skip_frame {
            self.skip_frame = false;
            return None;
        }
        if input.key_pressed(KeyCode::Escape) {
            self.open = false;
            return None;
        }
        for &c in &input.typed {
            if self.text.chars().count() < 120 {
                self.text.push(c);
            }
        }
        if input.key_pressed(KeyCode::Backspace) {
            self.text.pop();
        }
        if input.key_pressed(KeyCode::ArrowUp) && !self.history.is_empty() {
            let i = match self.history_pos {
                None => self.history.len() - 1,
                Some(i) => i.saturating_sub(1),
            };
            self.history_pos = Some(i);
            self.text = self.history[i].clone();
        }
        if input.key_pressed(KeyCode::ArrowDown) {
            if let Some(i) = self.history_pos {
                if i + 1 < self.history.len() {
                    self.history_pos = Some(i + 1);
                    self.text = self.history[i + 1].clone();
                } else {
                    self.history_pos = None;
                    self.text.clear();
                }
            }
        }
        if input.key_pressed(KeyCode::Enter) || input.key_pressed(KeyCode::NumpadEnter) {
            self.open = false;
            let cmd = self.text.trim().to_string();
            self.text.clear();
            if cmd.is_empty() {
                return None;
            }
            self.history.push(cmd.clone());
            return Some(cmd);
        }
        None
    }

    pub fn draw(&self, ui: &mut UiBatch, time: f64) {
        if !self.open {
            return;
        }
        let s = ui.scale;
        let h = ui.line_height(1.0) + 8.0 * s;
        let y = ui.height - h - 70.0 * s;
        ui.rect(6.0 * s, y, ui.width - 12.0 * s, h, [0, 0, 0, 170]);
        let caret = if (time * 2.0) as i64 % 2 == 0 { "_" } else { " " };
        ui.text(12.0 * s, y + 4.0 * s, &format!("> {}{}", self.text, caret), 1.0, WHITE);
    }
}
