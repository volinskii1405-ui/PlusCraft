//! Построение меша чанка: greedy meshing полных кубов с ambient occlusion и
//! плавным освещением по вершинам, плюс отдельные модели (растения, факелы,
//! рельсы, лестницы, жидкости).
//!
//! Greedy-слияние граней выполняется только для граней с одинаковыми
//! значениями AO и света во всех четырёх углах — иначе интерполяция
//! освещения была бы неверной. На открытой местности днём это даёт большие
//! квады; в пещерах с факелами грани остаются поблочными.

use crate::renderer::vertex::{ChunkVertex, VFLAG_EMISSIVE, VFLAG_LIQUID, VFLAG_WAVE};

use super::biome;
use super::block::{self, block_id, block_meta, def, id, Layer, Shape};
use super::chunk::{CHUNK_H, CHUNK_W};
use super::light::{self, LightField};
use super::neighborhood::Neighborhood;

pub struct MeshOutput {
    pub opaque: Vec<ChunkVertex>,
    pub translucent: Vec<ChunkVertex>,
    pub min_y: f32,
    pub max_y: f32,
    /// Свет центрального чанка (для игровой логики).
    pub light: Vec<u8>,
}

/// Углы грани для каждого направления (против часовой стрелки снаружи).
/// Направления: 0=+Y, 1=-Y, 2=+X, 3=-X, 4=+Z, 5=-Z.
const CORNERS: [[[u8; 3]; 4]; 6] = [
    [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
    [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
    [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
    [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]],
    [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
    [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]],
];

const NORMALS: [[i32; 3]; 6] = [[0, 1, 0], [0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];

/// Ось нормали для направления.
const AXIS: [usize; 6] = [1, 1, 0, 0, 2, 2];

/// UV по позиции вершины (в блоках, локально в чанке). Значения всегда
/// неотрицательны: текстура повторяется (REPEAT), смещение на целое не важно.
#[inline]
fn face_uv(dir: usize, p: [f32; 3]) -> (f32, f32) {
    let top = CHUNK_H as f32;
    match dir {
        0 | 1 => (p[0], p[2]),
        2 => (16.0 - p[2], top - p[1]),
        3 => (p[2], top - p[1]),
        4 => (p[0], top - p[1]),
        _ => (16.0 - p[0], top - p[1]),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Face {
    tex: u16,
    ao: [u8; 4],
    sky: [u8; 4],
    blk: [u8; 4],
    tint: [u8; 3],
    flags: u8,
    translucent: bool,
}

impl Face {
    #[inline]
    fn uniform(&self) -> bool {
        self.ao.iter().all(|&a| a == self.ao[0])
            && self.sky.iter().all(|&a| a == self.sky[0])
            && self.blk.iter().all(|&a| a == self.blk[0])
    }
}

struct Builder<'a> {
    nb: &'a Neighborhood,
    light: &'a LightField,
    smooth: bool,
    opaque: Vec<ChunkVertex>,
    translucent: Vec<ChunkVertex>,
    /// Полупрозрачные грани жидкостей — рисуются раньше прочих полупрозрачных
    /// блоков (лёд, стекло обычно ближе к камере, чем вода под ними).
    liquid: Vec<ChunkVertex>,
    in_liquid: bool,
    min_y: f32,
    max_y: f32,
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn vert(p: [f32; 3], tex: u16, uv: (f32, f32), normal: u8, ao: u8, sky: u8, blk: u8, flags: u8, tint: [u8; 3], alpha: u8) -> ChunkVertex {
    ChunkVertex {
        pos: [
            (p[0] * 16.0).round().max(0.0) as u16,
            (p[1] * 16.0).round().max(0.0) as u16,
            (p[2] * 16.0).round().max(0.0) as u16,
            tex,
        ],
        uv: [(uv.0 * 16.0).round().max(0.0) as u16, (uv.1 * 16.0).round().max(0.0) as u16],
        info: [normal | (ao << 3), sky, blk, flags],
        color: [tint[0], tint[1], tint[2], alpha],
    }
}

/// Должна ли быть видна грань блока `a`, смотрящая на соседа `b`.
#[inline]
fn face_visible(a: u16, b: u16) -> bool {
    let db = def(block_id(b));
    if db.opaque {
        return false;
    }
    let da = def(block_id(a));
    if block_id(a) == block_id(b) && da.layer != Layer::Cutout {
        return false;
    }
    // Полупрозрачный блок (лёд, стекло) не рисует грань в сторону воды —
    // иначе при взгляде сверху грань накладывается дважды.
    if da.layer == Layer::Translucent && db.shape == Shape::Liquid && db.layer == Layer::Translucent {
        return false;
    }
    // Грань куба, смотрящая в жидкость того же типа, у которой есть уровень, — видна.
    true
}

fn tint_for(v: u16, face: usize, biome_id: u8) -> [u8; 3] {
    let d = def(block_id(v));
    if !d.tint {
        return [255, 255, 255];
    }
    let bid = block_id(v);
    if bid == id::GRASS && face != 0 {
        return [255, 255, 255];
    }
    let b = biome::get(biome_id);
    if bid == id::OAK_LEAVES {
        b.foliage
    } else {
        b.grass
    }
}

impl<'a> Builder<'a> {
    #[inline]
    fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        self.nb.get(x, y, z)
    }

    #[inline]
    fn occludes(&self, x: i32, y: i32, z: i32) -> bool {
        block::is_opaque(self.get(x, y, z))
    }

    fn push_quad(&mut self, translucent: bool, v: [ChunkVertex; 4], flip: bool) {
        for q in &v {
            let y = q.pos[1] as f32 / 16.0;
            self.min_y = self.min_y.min(y);
            self.max_y = self.max_y.max(y);
        }
        let out = if !translucent {
            &mut self.opaque
        } else if self.in_liquid {
            &mut self.liquid
        } else {
            &mut self.translucent
        };
        if flip {
            out.extend_from_slice(&[v[1], v[2], v[3], v[0]]);
        } else {
            out.extend_from_slice(&v);
        }
    }

    /// Вычисляет AO и свет для грани блока (x,y,z) в направлении dir.
    fn face_lighting(&self, x: i32, y: i32, z: i32, dir: usize) -> ([u8; 4], [u8; 4], [u8; 4]) {
        let n = NORMALS[dir];
        let (fx, fy, fz) = (x + n[0], y + n[1], z + n[2]);
        let (s0, b0) = self.light.get(fx, fy, fz);
        let axis = AXIS[dir];
        let mut ao = [3u8; 4];
        let mut sky = [s0 * 4; 4];
        let mut blk = [b0 * 4; 4];
        for (k, c) in CORNERS[dir].iter().enumerate() {
            // Касательные смещения к углу: 0 -> -1, 1 -> +1 по осям, кроме нормали.
            let mut t1 = [0i32; 3];
            let mut t2 = [0i32; 3];
            let mut first = true;
            for a in 0..3 {
                if a == axis {
                    continue;
                }
                let d = if c[a] == 1 { 1 } else { -1 };
                if first {
                    t1[a] = d;
                    first = false;
                } else {
                    t2[a] = d;
                }
            }
            let p1 = (fx + t1[0], fy + t1[1], fz + t1[2]);
            let p2 = (fx + t2[0], fy + t2[1], fz + t2[2]);
            let pc = (fx + t1[0] + t2[0], fy + t1[1] + t2[1], fz + t1[2] + t2[2]);
            let o1 = self.occludes(p1.0, p1.1, p1.2);
            let o2 = self.occludes(p2.0, p2.1, p2.2);
            let oc = self.occludes(pc.0, pc.1, pc.2);
            ao[k] = if o1 && o2 { 0 } else { 3 - (o1 as u8 + o2 as u8 + oc as u8) };
            if self.smooth {
                let mut ss = s0 as u32;
                let mut bs = b0 as u32;
                let mut cnt = 1u32;
                if !o1 {
                    let (s, b) = self.light.get(p1.0, p1.1, p1.2);
                    ss += s as u32;
                    bs += b as u32;
                    cnt += 1;
                }
                if !o2 {
                    let (s, b) = self.light.get(p2.0, p2.1, p2.2);
                    ss += s as u32;
                    bs += b as u32;
                    cnt += 1;
                }
                if !oc && !(o1 && o2) {
                    let (s, b) = self.light.get(pc.0, pc.1, pc.2);
                    ss += s as u32;
                    bs += b as u32;
                    cnt += 1;
                }
                sky[k] = ((ss * 4 + cnt / 2) / cnt) as u8;
                blk[k] = ((bs * 4 + cnt / 2) / cnt) as u8;
            }
        }
        if !self.smooth {
            ao = [3; 4];
        }
        (ao, sky, blk)
    }

    fn cube_face(&self, x: i32, y: i32, z: i32, v: u16, dir: usize) -> Option<Face> {
        let n = NORMALS[dir];
        let nbv = self.get(x + n[0], y + n[1], z + n[2]);
        if !face_visible(v, nbv) {
            return None;
        }
        let d = def(block_id(v));
        let (ao, sky, blk) = self.face_lighting(x, y, z, dir);
        let mut flags = 0;
        if d.waving {
            flags |= VFLAG_WAVE;
        }
        if d.emissive {
            flags |= VFLAG_EMISSIVE;
        }
        Some(Face {
            tex: block::face_tex(v, dir) as u16,
            ao,
            sky,
            blk,
            tint: tint_for(v, dir, self.nb.biome(x as usize, z as usize)),
            flags,
            translucent: d.layer == Layer::Translucent,
        })
    }

    fn emit_face(&mut self, dir: usize, cell: [i32; 3], w: i32, h: i32, f: &Face) {
        let axis = AXIS[dir];
        let (ua, va) = match axis {
            1 => (0, 2),
            0 => (2, 1),
            _ => (0, 1),
        };
        let mut verts = [ChunkVertex::default(); 4];
        let mut bright = [0u32; 4];
        for (k, c) in CORNERS[dir].iter().enumerate() {
            let mut p = [cell[0] as f32, cell[1] as f32, cell[2] as f32];
            p[axis] += c[axis] as f32;
            if c[ua] == 1 {
                p[ua] += w as f32;
            }
            if c[va] == 1 {
                p[va] += h as f32;
            }
            let uv = face_uv(dir, p);
            let alpha = 255;
            verts[k] = vert(p, f.tex, uv, dir as u8, f.ao[k], f.sky[k], f.blk[k], f.flags, f.tint, alpha);
            bright[k] = f.ao[k] as u32 * 20 + f.sky[k] as u32 + f.blk[k] as u32;
        }
        let flip = bright[0] + bright[2] < bright[1] + bright[3];
        self.push_quad(f.translucent, verts, flip);
    }

    /// Greedy meshing полных кубов по всем шести направлениям.
    fn greedy(&mut self, ytop: usize) {
        let h = ytop as i32;
        let mut mask: Vec<Option<Face>> = vec![None; CHUNK_W * CHUNK_H];
        for dir in 0..6 {
            let axis = AXIS[dir];
            // Размеры среза: (U, V) и число срезов.
            let (slices, du, dv) = match axis {
                1 => (h, 16, 16),
                0 => (16, 16, h),
                _ => (16, 16, h),
            };
            for s in 0..slices {
                // Заполнение маски.
                for vv in 0..dv {
                    for uu in 0..du {
                        let (x, y, z) = match axis {
                            1 => (uu, s, vv),
                            0 => (s, vv, uu),
                            _ => (uu, vv, s),
                        };
                        let v = self.get(x, y, z);
                        let d = def(block_id(v));
                        mask[(vv * du + uu) as usize] = if d.shape == Shape::Cube {
                            self.cube_face(x, y, z, v, dir)
                        } else {
                            None
                        };
                    }
                }
                // Слияние.
                for vv in 0..dv {
                    let mut uu = 0;
                    while uu < du {
                        let i = (vv * du + uu) as usize;
                        let Some(f) = mask[i] else {
                            uu += 1;
                            continue;
                        };
                        let mut w = 1;
                        let mut hh = 1;
                        if f.uniform() {
                            while uu + w < du && mask[(vv * du + uu + w) as usize] == Some(f) {
                                w += 1;
                            }
                            'outer: while vv + hh < dv {
                                for k in 0..w {
                                    if mask[((vv + hh) * du + uu + k) as usize] != Some(f) {
                                        break 'outer;
                                    }
                                }
                                hh += 1;
                            }
                        }
                        for dy in 0..hh {
                            for dx in 0..w {
                                mask[((vv + dy) * du + uu + dx) as usize] = None;
                            }
                        }
                        let cell = match axis {
                            1 => [uu, s, vv],
                            0 => [s, vv, uu],
                            _ => [uu, vv, s],
                        };
                        self.emit_face(dir, cell, w, hh, &f);
                        uu += w;
                    }
                }
            }
        }
    }

    fn cell_light(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        let (s, b) = self.light.get(x, y, z);
        (s * 4, b * 4)
    }

    /// Произвольный квад (растения, рельсы...). Углы — в блоках, локально.
    #[allow(clippy::too_many_arguments)]
    fn quad(&mut self, p: [[f32; 3]; 4], uv: [(f32, f32); 4], tex: u16, normal: u8, light: (u8, u8), flags: u8, tint: [u8; 3], translucent: bool, double: bool) {
        let mk = |k: usize| vert(p[k], tex, uv[k], normal, 3, light.0, light.1, flags, tint, 255);
        let v = [mk(0), mk(1), mk(2), mk(3)];
        self.push_quad(translucent, v, false);
        if double {
            self.push_quad(translucent, [v[3], v[2], v[1], v[0]], false);
        }
    }

    fn special(&mut self, x: i32, y: i32, z: i32, v: u16) {
        let bid = block_id(v);
        let d = def(bid);
        let (fx, fy, fz) = (x as f32, y as f32, z as f32);
        let mut flags = 0;
        if d.waving {
            flags |= VFLAG_WAVE;
        }
        if d.emissive {
            flags |= VFLAG_EMISSIVE;
        }
        let tint = tint_for(v, 2, self.nb.biome(x as usize, z as usize));
        match d.shape {
            Shape::Cross => {
                let l = self.cell_light(x, y, z);
                let tex = d.tex_side as u16;
                let (a, b) = (0.15, 0.85);
                let uv = [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
                self.quad(
                    [[fx + a, fy, fz + a], [fx + b, fy, fz + b], [fx + b, fy + 1.0, fz + b], [fx + a, fy + 1.0, fz + a]],
                    uv, tex, 2, l, flags, tint, false, true,
                );
                self.quad(
                    [[fx + a, fy, fz + b], [fx + b, fy, fz + a], [fx + b, fy + 1.0, fz + a], [fx + a, fy + 1.0, fz + b]],
                    uv, tex, 2, l, flags, tint, false, true,
                );
            }
            Shape::Torch => {
                let l = self.cell_light(x, y, z);
                let tex = d.tex_side as u16;
                let (a, b) = (7.0 / 16.0, 9.0 / 16.0);
                let top = 10.0 / 16.0;
                let (u0, u1, v0, v1) = (7.0 / 16.0, 9.0 / 16.0, 6.0 / 16.0, 1.0);
                // Четыре боковые грани.
                let sides: [([f32; 3], [f32; 3], u8); 4] = [
                    ([b, 0.0, a], [b, 0.0, b], 2),
                    ([a, 0.0, b], [a, 0.0, a], 3),
                    ([b, 0.0, b], [a, 0.0, b], 4),
                    ([a, 0.0, a], [b, 0.0, a], 5),
                ];
                for (p0, p1, n) in sides {
                    // Нижние углы p0 -> p1, верх — те же на высоте top.
                    self.quad(
                        [
                            [fx + p0[0], fy, fz + p0[2]],
                            [fx + p0[0], fy + top, fz + p0[2]],
                            [fx + p1[0], fy + top, fz + p1[2]],
                            [fx + p1[0], fy, fz + p1[2]],
                        ],
                        [(u1, v1), (u1, v0), (u0, v0), (u0, v1)],
                        tex, n, l, flags, tint, false, false,
                    );
                }
                self.quad(
                    [[fx + a, fy + top, fz + a], [fx + a, fy + top, fz + b], [fx + b, fy + top, fz + b], [fx + b, fy + top, fz + a]],
                    [(u0, v0), (u0, 8.0 / 16.0), (u1, 8.0 / 16.0), (u1, v0)],
                    tex, 0, l, flags, tint, false, false,
                );
            }
            Shape::Rail => {
                let l = self.cell_light(x, y, z);
                let tex = d.tex_top as u16;
                let hgt = fy + 1.0 / 16.0;
                let along_x = block_meta(v) & 1 == 1;
                let uv = if along_x {
                    [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                } else {
                    [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)]
                };
                self.quad(
                    [[fx, hgt, fz], [fx, hgt, fz + 1.0], [fx + 1.0, hgt, fz + 1.0], [fx + 1.0, hgt, fz]],
                    uv, tex, 0, l, flags, tint, false, true,
                );
            }
            Shape::Ladder => {
                let l = self.cell_light(x, y, z);
                let tex = d.tex_side as u16;
                // meta: сторона стены 2..5 (+X, -X, +Z, -Z), лестница — у этой стены.
                let e = 1.0 / 16.0;
                let uv = [(0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
                let p = match block_meta(v) & 7 {
                    2 => [[fx + 1.0 - e, fy, fz], [fx + 1.0 - e, fy + 1.0, fz], [fx + 1.0 - e, fy + 1.0, fz + 1.0], [fx + 1.0 - e, fy, fz + 1.0]],
                    3 => [[fx + e, fy, fz + 1.0], [fx + e, fy + 1.0, fz + 1.0], [fx + e, fy + 1.0, fz], [fx + e, fy, fz]],
                    4 => [[fx + 1.0, fy, fz + 1.0 - e], [fx + 1.0, fy + 1.0, fz + 1.0 - e], [fx, fy + 1.0, fz + 1.0 - e], [fx, fy, fz + 1.0 - e]],
                    _ => [[fx, fy, fz + e], [fx, fy + 1.0, fz + e], [fx + 1.0, fy + 1.0, fz + e], [fx + 1.0, fy, fz + e]],
                };
                self.quad(p, uv, tex, 2, l, flags, tint, false, true);
            }
            Shape::Liquid => {
                self.in_liquid = true;
                self.liquid(x, y, z, v);
                self.in_liquid = false;
            }
            Shape::Cube | Shape::Air => {}
        }
    }

    fn liquid_height(&self, x: i32, y: i32, z: i32, bid: u8) -> f32 {
        let v = self.get(x, y, z);
        if block_id(v) != bid {
            return -1.0;
        }
        if block_id(self.get(x, y + 1, z)) == bid {
            return 1.0;
        }
        let level = (block_meta(v) & 7) as f32;
        (8.0 - level) / 8.0 * 0.875
    }

    fn liquid(&mut self, x: i32, y: i32, z: i32, v: u16) {
        let bid = block_id(v);
        let d = def(bid);
        let translucent = d.layer == Layer::Translucent;
        let tex = d.tex_top as u16;
        let h = self.liquid_height(x, y, z, bid);
        let (fx, fy, fz) = (x as f32, y as f32, z as f32);
        let mut base_flags = 0;
        if d.emissive {
            base_flags |= VFLAG_EMISSIVE;
        }
        let tint = [255, 255, 255];
        for dir in 0..6 {
            let n = NORMALS[dir];
            let nv = self.get(x + n[0], y + n[1], z + n[2]);
            let nid = block_id(nv);
            if nid == bid {
                // Боковая грань к более низкой воде того же типа.
                if dir >= 2 {
                    let nh = self.liquid_height(x + n[0], y, z + n[2], bid);
                    if nh >= h - 0.01 {
                        continue;
                    }
                } else {
                    continue;
                }
            } else if block::is_opaque(nv) {
                continue;
            }
            let l = self.cell_light(x + n[0], y + n[1], z + n[2]);
            let l = if l.0 == 0 && l.1 == 0 { self.cell_light(x, y, z) } else { l };
            let mut flags = base_flags;
            if dir == 0 && h < 1.0 {
                flags |= VFLAG_LIQUID;
            }
            let mut verts = [ChunkVertex::default(); 4];
            for (k, c) in CORNERS[dir].iter().enumerate() {
                let p = [
                    fx + c[0] as f32,
                    fy + if c[1] == 1 { h } else { 0.0 },
                    fz + c[2] as f32,
                ];
                let uv = face_uv(dir, p);
                verts[k] = vert(p, tex, uv, dir as u8, 3, l.0, l.1, flags, tint, 255);
            }
            self.push_quad(translucent, verts, false);
        }
    }
}

/// Строит меш центрального чанка окрестности.
pub fn build(nb: &Neighborhood, smooth: bool) -> MeshOutput {
    let center_top = nb.center_data().map(|c| c.top_y()).unwrap_or(0);
    let mut top_all = center_top;
    for row in &nb.chunks {
        for c in row.iter().flatten() {
            top_all = top_all.max(c.top_y());
        }
    }
    let light = light::compute(nb, top_all);
    let mut b = Builder {
        nb,
        light: &light,
        smooth,
        opaque: Vec::with_capacity(1 << 14),
        translucent: Vec::new(),
        liquid: Vec::new(),
        in_liquid: false,
        min_y: f32::MAX,
        max_y: f32::MIN,
    };
    let ytop = (center_top + 1).min(CHUNK_H);
    b.greedy(ytop);
    for y in 0..ytop as i32 {
        for z in 0..16 {
            for x in 0..16 {
                let v = b.get(x, y, z);
                let s = def(block_id(v)).shape;
                if s != Shape::Cube && s != Shape::Air {
                    b.special(x, y, z, v);
                }
            }
        }
    }
    if b.min_y > b.max_y {
        b.min_y = 0.0;
        b.max_y = 0.0;
    }
    MeshOutput {
        min_y: b.min_y,
        max_y: b.max_y,
        opaque: b.opaque,
        translucent: {
            let mut t = b.liquid;
            t.extend_from_slice(&b.translucent);
            t
        },
        light: light.center(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::ChunkPos;
    use crate::world::gen::WorldGen;
    use std::sync::Arc;

    #[test]
    fn water_faces_near_border() {
        let g = WorldGen::new(2024);
        let c = ChunkPos::new(3, 1);
        let mut chunks: [[Option<Arc<crate::world::chunk::ChunkData>>; 3]; 3] = Default::default();
        for dz in -1..=1 {
            for dx in -1..=1 {
                chunks[(dz + 1) as usize][(dx + 1) as usize] = Some(Arc::new(g.generate(c.offset(dx, dz))));
            }
        }
        let nb = Neighborhood { center: c, chunks };
        let out = build(&nb, true);
        // Квады с флагом жидкости (верх воды) по столбцам x.
        let mut per_x = [0u32; 16];
        let mut ice_x = [0u32; 17];
        for q in out.translucent.chunks(4) {
            let tex = q[0].pos[3];
            let xs: Vec<u16> = q.iter().map(|v| v.pos[0] / 16).collect();
            if q[0].info[3] & VFLAG_LIQUID != 0 {
                per_x[(*xs.iter().min().unwrap()).min(15) as usize] += 1;
            } else if tex == crate::assets::textures::Tex::Ice as u16 {
                ice_x[*xs.iter().min().unwrap() as usize] += 1;
            }
        }
        println!("water tops per x: {per_x:?}");
        println!("ice quads by min x: {ice_x:?}");
        let ys: Vec<u16> = out.translucent.iter().filter(|v| v.pos[3] == crate::assets::textures::Tex::Ice as u16).map(|v| v.pos[1]).collect();
        println!("ice y values: min {:?} max {:?}", ys.iter().min(), ys.iter().max());
    }
}
