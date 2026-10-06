//! Компиляция GLSL -> SPIR-V.
//!
//! Если в системе есть `glslangValidator` (или `glslc`), шейдеры из `shaders/`
//! компилируются заново. Иначе используются заранее собранные файлы из
//! `shaders/spv/` — так `cargo build` работает без Vulkan SDK.

use std::path::{Path, PathBuf};
use std::process::Command;

const SHADERS: &[&str] = &[
    "chunk.vert", "chunk.frag", "entity.vert", "entity.frag", "sky.vert", "sky.frag", "ui.vert",
    "ui.frag",
];

fn tool_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("shaders");
    std::fs::create_dir_all(&out).expect("create OUT_DIR/shaders");
    let src = Path::new("shaders");
    // Явно перечисляем файлы: отслеживание каталога целиком ненадёжно.
    println!("cargo:rerun-if-changed=shaders/common.glsl");
    for name in SHADERS {
        println!("cargo:rerun-if-changed=shaders/{name}");
        println!("cargo:rerun-if-changed=shaders/spv/{name}.spv");
    }

    let glslang = tool_available("glslangValidator");
    let glslc = !glslang && tool_available("glslc");

    for name in SHADERS {
        let input = src.join(name);
        let output = out.join(format!("{name}.spv"));
        let precompiled = src.join("spv").join(format!("{name}.spv"));
        if glslang || glslc {
            let status = if glslang {
                Command::new("glslangValidator")
                    .args(["-V", "--target-env", "vulkan1.1"])
                    .arg(format!("-I{}", src.display()))
                    .arg("-o")
                    .arg(&output)
                    .arg(&input)
                    .output()
            } else {
                Command::new("glslc")
                    .args(["--target-env=vulkan1.1"])
                    .arg(format!("-I{}", src.display()))
                    .arg("-o")
                    .arg(&output)
                    .arg(&input)
                    .output()
            }
            .expect("запуск компилятора шейдеров");
            if !status.status.success() {
                panic!(
                    "ошибка компиляции шейдера {name}:\n{}\n{}",
                    String::from_utf8_lossy(&status.stdout),
                    String::from_utf8_lossy(&status.stderr)
                );
            }
        } else if precompiled.exists() {
            std::fs::copy(&precompiled, &output).expect("копирование SPIR-V");
        } else {
            panic!("нет glslangValidator/glslc и нет готового {}", precompiled.display());
        }
    }
    if !glslang && !glslc {
        println!("cargo:warning=glslangValidator не найден — используются готовые SPIR-V из shaders/spv");
    }
}
