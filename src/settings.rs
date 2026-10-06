//! Настройки игры: загрузка/сохранение `settings.toml`, привязки клавиш.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Action {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Sneak,
    Sprint,
    Inventory,
    Drop,
    Attack,
    Use,
    PickBlock,
    Debug,
    ToggleFly,
    Chat,
}

impl Action {
    pub const ALL: [Action; 15] = [
        Action::Forward,
        Action::Back,
        Action::Left,
        Action::Right,
        Action::Jump,
        Action::Sneak,
        Action::Sprint,
        Action::Inventory,
        Action::Drop,
        Action::Attack,
        Action::Use,
        Action::PickBlock,
        Action::Debug,
        Action::ToggleFly,
        Action::Chat,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Action::Forward => "Вперёд",
            Action::Back => "Назад",
            Action::Left => "Влево",
            Action::Right => "Вправо",
            Action::Jump => "Прыжок / плыть вверх",
            Action::Sneak => "Присесть",
            Action::Sprint => "Бег",
            Action::Inventory => "Инвентарь",
            Action::Drop => "Выбросить предмет",
            Action::Attack => "Атака / ломать",
            Action::Use => "Использовать / ставить",
            Action::PickBlock => "Выбрать блок",
            Action::Debug => "Отладка (F3)",
            Action::ToggleFly => "Полёт (креатив)",
            Action::Chat => "Команды",
        }
    }

    fn default_key(self) -> Input {
        use KeyCode::*;
        match self {
            Action::Forward => Input::Key(KeyW),
            Action::Back => Input::Key(KeyS),
            Action::Left => Input::Key(KeyA),
            Action::Right => Input::Key(KeyD),
            Action::Jump => Input::Key(Space),
            Action::Sneak => Input::Key(ShiftLeft),
            Action::Sprint => Input::Key(ControlLeft),
            Action::Inventory => Input::Key(KeyE),
            Action::Drop => Input::Key(KeyQ),
            Action::Attack => Input::Mouse(0),
            Action::Use => Input::Mouse(1),
            Action::PickBlock => Input::Mouse(2),
            Action::Debug => Input::Key(F3),
            Action::ToggleFly => Input::Key(KeyF),
            Action::Chat => Input::Key(KeyT),
        }
    }
}

/// Клавиша клавиатуры или кнопка мыши (0 — левая, 1 — правая, 2 — средняя).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Input {
    Key(KeyCode),
    Mouse(u8),
}

macro_rules! key_table {
    ($($k:ident => $n:expr),* $(,)?) => {
        const KEY_TABLE: &[(KeyCode, &str)] = &[$((KeyCode::$k, $n)),*];
    };
}

key_table! {
    KeyA => "A", KeyB => "B", KeyC => "C", KeyD => "D", KeyE => "E", KeyF => "F", KeyG => "G",
    KeyH => "H", KeyI => "I", KeyJ => "J", KeyK => "K", KeyL => "L", KeyM => "M", KeyN => "N",
    KeyO => "O", KeyP => "P", KeyQ => "Q", KeyR => "R", KeyS => "S", KeyT => "T", KeyU => "U",
    KeyV => "V", KeyW => "W", KeyX => "X", KeyY => "Y", KeyZ => "Z",
    Digit0 => "0", Digit1 => "1", Digit2 => "2", Digit3 => "3", Digit4 => "4", Digit5 => "5",
    Digit6 => "6", Digit7 => "7", Digit8 => "8", Digit9 => "9",
    Space => "Space", ShiftLeft => "LShift", ShiftRight => "RShift", ControlLeft => "LCtrl",
    ControlRight => "RCtrl", AltLeft => "LAlt", AltRight => "RAlt", Tab => "Tab", CapsLock => "CapsLock",
    Enter => "Enter", Backspace => "Backspace", Escape => "Escape",
    ArrowUp => "Up", ArrowDown => "Down", ArrowLeft => "Left", ArrowRight => "Right",
    F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6", F7 => "F7", F8 => "F8",
    F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
    Backquote => "`", Minus => "-", Equal => "=", BracketLeft => "[", BracketRight => "]",
    Semicolon => ";", Quote => "'", Comma => ",", Period => ".", Slash => "/", Backslash => "\\",
    Numpad0 => "Num0", Numpad1 => "Num1", Numpad2 => "Num2", Numpad3 => "Num3", Numpad4 => "Num4",
    Numpad5 => "Num5", Numpad6 => "Num6", Numpad7 => "Num7", Numpad8 => "Num8", Numpad9 => "Num9",
    Insert => "Insert", Delete => "Delete", Home => "Home", End => "End", PageUp => "PageUp", PageDown => "PageDown",
}

impl Input {
    pub fn to_name(self) -> String {
        match self {
            Input::Key(k) => KEY_TABLE
                .iter()
                .find(|(c, _)| *c == k)
                .map(|(_, n)| n.to_string())
                .unwrap_or_else(|| format!("{k:?}")),
            Input::Mouse(0) => "ЛКМ".into(),
            Input::Mouse(1) => "ПКМ".into(),
            Input::Mouse(2) => "СКМ".into(),
            Input::Mouse(b) => format!("Мышь{}", b + 1),
        }
    }

    pub fn from_name(s: &str) -> Option<Input> {
        match s {
            "ЛКМ" | "Mouse1" => return Some(Input::Mouse(0)),
            "ПКМ" | "Mouse2" => return Some(Input::Mouse(1)),
            "СКМ" | "Mouse3" => return Some(Input::Mouse(2)),
            _ => {}
        }
        if let Some(rest) = s.strip_prefix("Мышь") {
            return rest.parse::<u8>().ok().map(|b| Input::Mouse(b.saturating_sub(1)));
        }
        KEY_TABLE.iter().find(|(_, n)| *n == s).map(|(c, _)| Input::Key(*c))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "Мирная",
            Difficulty::Easy => "Лёгкая",
            Difficulty::Normal => "Нормальная",
            Difficulty::Hard => "Сложная",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Difficulty::Peaceful => Difficulty::Easy,
            Difficulty::Easy => Difficulty::Normal,
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Peaceful,
        }
    }
    /// Множитель урона по игроку.
    pub fn damage_mul(self) -> f64 {
        match self {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => 0.5,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobDifficulty {
    Weak,
    Normal,
    Brutal,
}

impl MobDifficulty {
    pub fn name(self) -> &'static str {
        match self {
            MobDifficulty::Weak => "Слабые",
            MobDifficulty::Normal => "Обычные",
            MobDifficulty::Brutal => "Свирепые",
        }
    }
    pub fn next(self) -> Self {
        match self {
            MobDifficulty::Weak => MobDifficulty::Normal,
            MobDifficulty::Normal => MobDifficulty::Brutal,
            MobDifficulty::Brutal => MobDifficulty::Weak,
        }
    }
    /// Множитель здоровья/скорости/частоты спавна враждебных мобов.
    pub fn mul(self) -> f64 {
        match self {
            MobDifficulty::Weak => 0.7,
            MobDifficulty::Normal => 1.0,
            MobDifficulty::Brutal => 1.4,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Graphics {
    /// Радиус прорисовки в чанках.
    pub render_distance: u32,
    pub fov: f64,
    pub vsync: bool,
    pub smooth_lighting: bool,
    pub fullscreen: bool,
    pub window_width: u32,
    pub window_height: u32,
    /// Ограничение FPS (0 — без ограничения).
    pub fps_limit: u32,
    pub ui_scale: f64,
    /// Яркость 0..1.
    pub brightness: f64,
    pub view_bobbing: bool,
}

impl Default for Graphics {
    fn default() -> Self {
        Self {
            render_distance: 8,
            fov: 75.0,
            vsync: true,
            smooth_lighting: true,
            fullscreen: false,
            window_width: 1280,
            window_height: 720,
            fps_limit: 0,
            ui_scale: 1.0,
            brightness: 0.5,
            view_bobbing: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Controls {
    pub mouse_sensitivity: f64,
    pub invert_y: bool,
    /// Действие -> имя клавиши.
    pub bindings: BTreeMap<Action, String>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 1.0,
            invert_y: false,
            bindings: Action::ALL.iter().map(|a| (*a, a.default_key().to_name())).collect(),
        }
    }
}

impl Controls {
    pub fn binding(&self, a: Action) -> Input {
        self.bindings
            .get(&a)
            .and_then(|s| Input::from_name(s))
            .unwrap_or_else(|| a.default_key())
    }
    pub fn set_binding(&mut self, a: Action, input: Input) {
        self.bindings.insert(a, input.to_name());
    }
    pub fn reset(&mut self) {
        *self = Controls { mouse_sensitivity: self.mouse_sensitivity, invert_y: self.invert_y, ..Default::default() };
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Gameplay {
    /// Длительность суток в минутах реального времени.
    pub day_length_minutes: f64,
    pub difficulty: Difficulty,
    pub mob_difficulty: MobDifficulty,
    /// Seed по умолчанию для новых миров (пусто — случайный).
    pub default_seed: String,
    /// Время горения факела в минутах (уникальная механика «свет как ресурс»).
    pub torch_burn_minutes: f64,
    /// Обвалы пород без крепей.
    pub cave_ins: bool,
}

impl Default for Gameplay {
    fn default() -> Self {
        Self {
            day_length_minutes: 20.0,
            difficulty: Difficulty::Normal,
            mob_difficulty: MobDifficulty::Normal,
            default_seed: String::new(),
            torch_burn_minutes: 12.0,
            cave_ins: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Audio {
    pub master: f64,
    pub effects: f64,
    pub ambient: f64,
}

impl Default for Audio {
    fn default() -> Self {
        Self { master: 0.8, effects: 1.0, ambient: 0.6 }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub graphics: Graphics,
    pub controls: Controls,
    pub gameplay: Gameplay,
    pub audio: Audio,
}

impl Settings {
    /// Загружает настройки; при ошибке — значения по умолчанию (и пишет файл).
    pub fn load() -> Self {
        let path = crate::paths::settings_file();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(mut s) => {
                    s.sanitize();
                    log::info!("Настройки загружены из {}", path.display());
                    s
                }
                Err(e) => {
                    log::warn!("{}: ошибка разбора ({e}); используются значения по умолчанию", path.display());
                    Settings::default()
                }
            },
            Err(_) => {
                let s = Settings::default();
                if let Err(e) = s.save() {
                    log::warn!("не удалось записать настройки по умолчанию: {e:#}");
                }
                s
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = crate::paths::settings_file();
        let text = toml::to_string_pretty(self).context("сериализация настроек")?;
        let header = "# Настройки PlusCraft. Файл перезаписывается из меню настроек.\n\n";
        std::fs::write(&path, format!("{header}{text}")).with_context(|| format!("запись {}", path.display()))?;
        Ok(())
    }

    pub fn sanitize(&mut self) {
        let g = &mut self.graphics;
        g.render_distance = g.render_distance.clamp(2, 16);
        g.fov = g.fov.clamp(40.0, 110.0);
        g.window_width = g.window_width.clamp(640, 7680);
        g.window_height = g.window_height.clamp(360, 4320);
        g.ui_scale = g.ui_scale.clamp(0.5, 3.0);
        g.brightness = g.brightness.clamp(0.0, 1.0);
        self.controls.mouse_sensitivity = self.controls.mouse_sensitivity.clamp(0.1, 5.0);
        self.gameplay.day_length_minutes = self.gameplay.day_length_minutes.clamp(1.0, 120.0);
        self.gameplay.torch_burn_minutes = self.gameplay.torch_burn_minutes.clamp(1.0, 600.0);
        for v in [&mut self.audio.master, &mut self.audio.effects, &mut self.audio.ambient] {
            *v = v.clamp(0.0, 1.0);
        }
    }
}
