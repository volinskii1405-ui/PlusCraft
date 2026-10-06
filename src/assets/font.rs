//! Растеризация шрифта (fontdue) в атлас интерфейса. Поддерживает латиницу и
//! кириллицу.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

pub const ATLAS_SIZE: u32 = 512;
/// Размер растеризации в пикселях (базовый масштаб UI = 1.0).
pub const FONT_PX: f32 = 18.0;

#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    /// UV в атласе (0..1).
    pub uv: [f32; 4],
    pub width: f32,
    pub height: f32,
    pub xmin: f32,
    pub ymin: f32,
    pub advance: f32,
}

pub struct FontAtlas {
    pub glyphs: HashMap<char, Glyph>,
    pub ascent: f32,
    pub line_height: f32,
    /// RGBA-пиксели атласа (белый цвет, форма — в альфе).
    pub pixels: Vec<u8>,
}

const EMBEDDED_FONT: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");

fn charset() -> Vec<char> {
    let mut v: Vec<char> = (32u8..127).map(|c| c as char).collect();
    v.extend('А'..='я');
    v.extend(['Ё', 'ё', '°', '×', '→', '←', '↑', '↓', '…', '—', '–', '«', '»', '•', '№', '♥', '✦']);
    v
}

impl FontAtlas {
    pub fn load() -> Result<Self> {
        let path = crate::paths::assets_dir().join("fonts").join("DejaVuSans.ttf");
        let data = std::fs::read(&path).unwrap_or_else(|_| EMBEDDED_FONT.to_vec());
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .map_err(|e| anyhow!("ошибка загрузки шрифта: {e}"))?;
        let lm = font
            .horizontal_line_metrics(FONT_PX)
            .ok_or_else(|| anyhow!("шрифт без горизонтальных метрик"))?;

        let mut pixels = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];
        // Белый пиксель в углу (0,0)-(2,2) — для сплошной заливки, если понадобится.
        for y in 0..2 {
            for x in 0..2 {
                let i = ((y * ATLAS_SIZE + x) * 4) as usize;
                pixels[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let mut glyphs = HashMap::new();
        let (mut cx, mut cy, mut row_h) = (4u32, 4u32, 0u32);
        for ch in charset() {
            let (m, bitmap) = font.rasterize(ch, FONT_PX);
            let (w, h) = (m.width as u32, m.height as u32);
            if cx + w + 2 >= ATLAS_SIZE {
                cx = 4;
                cy += row_h + 2;
                row_h = 0;
            }
            if cy + h + 2 >= ATLAS_SIZE {
                log::warn!("атлас шрифта переполнен на символе {ch:?}");
                break;
            }
            for y in 0..h {
                for x in 0..w {
                    let a = bitmap[(y * w + x) as usize];
                    let i = (((cy + y) * ATLAS_SIZE + cx + x) * 4) as usize;
                    pixels[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
                }
            }
            let s = ATLAS_SIZE as f32;
            glyphs.insert(
                ch,
                Glyph {
                    uv: [cx as f32 / s, cy as f32 / s, (cx + w) as f32 / s, (cy + h) as f32 / s],
                    width: w as f32,
                    height: h as f32,
                    xmin: m.xmin as f32,
                    ymin: m.ymin as f32,
                    advance: m.advance_width,
                },
            );
            cx += w + 2;
            row_h = row_h.max(h);
        }
        Ok(Self {
            glyphs,
            ascent: lm.ascent,
            line_height: lm.new_line_size,
            pixels,
        })
    }

    /// Ширина строки в пикселях при масштабе `scale`.
    pub fn measure(&self, text: &str, scale: f32) -> f32 {
        text.chars()
            .map(|c| self.glyphs.get(&c).or_else(|| self.glyphs.get(&'?')).map(|g| g.advance).unwrap_or(0.0))
            .sum::<f32>()
            * scale
    }
}
