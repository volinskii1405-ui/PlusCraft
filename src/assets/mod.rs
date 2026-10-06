//! Ассеты: текстуры блоков (PNG-атлас) и шрифт интерфейса.

pub mod font;
pub mod textures;

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::renderer::TextureLayers;

/// Загружает текстуры блоков из `assets/textures/blocks.png`, а если файла
/// нет или он устарел — генерирует процедурно.
pub fn load_block_textures() -> TextureLayers {
    let path = crate::paths::assets_dir().join("textures").join("blocks.png");
    match load_png(&path).and_then(|(w, h, img)| {
        textures::split_atlas(w, h, &img).context("атлас меньше, чем нужно тайлов")
    }) {
        Ok(layers) => {
            log::info!("Текстуры загружены из {}", path.display());
            TextureLayers { size: textures::TILE as u32, layers }
        }
        Err(e) => {
            log::warn!("{}: {e:#} — генерирую текстуры процедурно", path.display());
            let layers = textures::ALL_TILES.iter().map(|&t| textures::generate(t)).collect();
            TextureLayers { size: textures::TILE as u32, layers }
        }
    }
}

/// Сохраняет процедурный атлас в PNG (используется скриптом упаковки и флагом
/// `--gen-assets`).
pub fn write_block_atlas(path: &Path) -> Result<()> {
    let (w, h, img) = textures::build_atlas();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::File::create(path).with_context(|| format!("создание {}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(&img)?;
    Ok(())
}

pub fn load_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path)?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    let (w, h) = (info.width, info.height);
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => data.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => bail!("неподдерживаемый формат PNG: {other:?}"),
    };
    Ok((w, h, rgba))
}
