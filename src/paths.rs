//! Поиск каталогов игры: ассеты, настройки, сохранения.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Корень игры — каталог с исполняемым файлом, если рядом лежит `assets/`,
/// иначе текущий каталог (удобно при `cargo run`).
pub fn game_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                if dir.join("assets").is_dir() {
                    return dir.to_path_buf();
                }
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    })
}

pub fn assets_dir() -> PathBuf {
    game_root().join("assets")
}

pub fn settings_file() -> PathBuf {
    game_root().join("settings.toml")
}

pub fn saves_dir() -> PathBuf {
    game_root().join("saves")
}
