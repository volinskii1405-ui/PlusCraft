//! Построитель геометрии интерфейса: прямоугольники, текст, иконки.

use crate::assets::font::FontAtlas;
use crate::renderer::vertex::{UiVertex, UI_ATLAS_BIT, UI_SOLID};

pub type Color = [u8; 4];

pub const WHITE: Color = [255, 255, 255, 255];
pub const BLACK: Color = [0, 0, 0, 255];
pub const SHADOW: Color = [20, 20, 20, 200];

pub struct UiBatch<'a> {
    pub verts: Vec<UiVertex>,
    pub font: &'a FontAtlas,
    pub width: f32,
    pub height: f32,
    /// Общий масштаб интерфейса.
    pub scale: f32,
}

impl<'a> UiBatch<'a> {
    pub fn new(font: &'a FontAtlas, width: f32, height: f32, scale: f32) -> Self {
        Self { verts: Vec::with_capacity(4096), font, width, height, scale }
    }

    #[inline]
    fn push_quad(&mut self, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], color: Color, tex: u32) {
        let v = |i: usize| UiVertex { pos: p[i], uv: uv[i], color, tex };
        self.verts.extend_from_slice(&[v(0), v(1), v(2), v(2), v(3), v(0)]);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.push_quad(
            [[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            [[0.0; 2]; 4],
            color,
            UI_SOLID,
        );
    }

    /// Рамка толщиной t.
    pub fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, color: Color) {
        self.rect(x, y, w, t, color);
        self.rect(x, y + h - t, w, t, color);
        self.rect(x, y + t, t, h - 2.0 * t, color);
        self.rect(x + w - t, y + t, t, h - 2.0 * t, color);
    }

    /// Панель в стиле игры: тёмная подложка со светлой и тёмной кромкой.
    pub fn panel(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let s = self.scale;
        self.rect(x, y, w, h, [30, 28, 36, 235]);
        self.frame(x, y, w, h, 2.0 * s, [90, 86, 104, 255]);
        self.frame(x + 2.0 * s, y + 2.0 * s, w - 4.0 * s, h - 4.0 * s, 1.0 * s, [16, 14, 20, 255]);
    }

    /// Слот инвентаря (вдавленный квадрат).
    pub fn slot(&mut self, x: f32, y: f32, size: f32, highlight: bool) {
        let s = self.scale;
        self.rect(x, y, size, size, if highlight { [120, 116, 140, 255] } else { [55, 52, 64, 255] });
        self.rect(x, y, size, 1.0 * s, [22, 20, 28, 255]);
        self.rect(x, y, 1.0 * s, size, [22, 20, 28, 255]);
        self.rect(x, y + size - 1.0 * s, size, 1.0 * s, [110, 106, 126, 255]);
        self.rect(x + size - 1.0 * s, y, 1.0 * s, size, [110, 106, 126, 255]);
    }

    /// Иконка из массива текстур блоков.
    pub fn icon(&mut self, x: f32, y: f32, size: f32, layer: u16, tint: Color) {
        self.push_quad(
            [[x, y], [x + size, y], [x + size, y + size], [x, y + size]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            tint,
            layer as u32,
        );
    }

    /// Изометрическая иконка куба: верх, левая и правая грани.
    pub fn iso_block(&mut self, x: f32, y: f32, size: f32, top: u16, left: u16, right: u16) {
        let cx = x + size * 0.5;
        let w = size * 0.46;
        let hh = size * 0.25;
        let top_y = y + size * 0.04;
        // Вершины ромба верхней грани.
        let t0 = [cx, top_y];
        let t1 = [cx + w, top_y + hh];
        let t2 = [cx, top_y + 2.0 * hh];
        let t3 = [cx - w, top_y + hh];
        let depth = size * 0.5;
        self.push_quad(
            [t3, t0, t1, t2],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            [255, 255, 255, 255],
            top as u32,
        );
        self.push_quad(
            [t3, t2, [t2[0], t2[1] + depth], [t3[0], t3[1] + depth]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            [200, 200, 200, 255],
            left as u32,
        );
        self.push_quad(
            [t2, t1, [t1[0], t1[1] + depth], [t2[0], t2[1] + depth]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            [150, 150, 150, 255],
            right as u32,
        );
    }

    /// Высота строки текста при масштабе `size` (1.0 = базовый размер шрифта).
    pub fn line_height(&self, size: f32) -> f32 {
        self.font.line_height * size * self.scale
    }

    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        self.font.measure(text, size * self.scale)
    }

    /// Текст; (x, y) — левый верхний угол строки.
    pub fn text(&mut self, x: f32, y: f32, text: &str, size: f32, color: Color) {
        let k = size * self.scale;
        let baseline = y + self.font.ascent * k;
        let mut pen = x;
        for ch in text.chars() {
            let Some(g) = self.font.glyphs.get(&ch).or_else(|| self.font.glyphs.get(&'?')).copied() else {
                continue;
            };
            if g.width > 0.0 {
                let gx = pen + g.xmin * k;
                let gy = baseline - (g.ymin + g.height) * k;
                let (w, h) = (g.width * k, g.height * k);
                self.push_quad(
                    [[gx, gy], [gx + w, gy], [gx + w, gy + h], [gx, gy + h]],
                    [[g.uv[0], g.uv[1]], [g.uv[2], g.uv[1]], [g.uv[2], g.uv[3]], [g.uv[0], g.uv[3]]],
                    color,
                    UI_ATLAS_BIT,
                );
            }
            pen += g.advance * k;
        }
    }

    /// Текст с тенью.
    pub fn text_shadow(&mut self, x: f32, y: f32, text: &str, size: f32, color: Color) {
        let o = (1.0 * self.scale * size).max(1.0);
        self.text(x + o, y + o, text, size, [0, 0, 0, (color[3] as f32 * 0.75) as u8]);
        self.text(x, y, text, size, color);
    }

    pub fn text_centered(&mut self, cx: f32, y: f32, text: &str, size: f32, color: Color) {
        let w = self.text_width(text, size);
        self.text_shadow(cx - w * 0.5, y, text, size, color);
    }
}
