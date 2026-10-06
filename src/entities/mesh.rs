//! Построение геометрии сущностей из примитивов (кубоиды, карточки, рамки).
//! Все позиции — относительно камеры.

use glam::{Mat4, Vec3};

use crate::renderer::vertex::{EntityVertex, NO_TEXTURE};

/// Упакованный свет для вершины сущности.
pub fn pack_light(sky: u8, block: u8) -> u16 {
    ((sky as u16) << 8) | block as u16
}

/// Свет «без освещения» (полная яркость, не зависит от времени суток).
pub const UNLIT: u16 = 0xFF00;

const SHADE: [f32; 6] = [1.0, 0.55, 0.8, 0.8, 0.68, 0.68];

/// Грань кубоида: слой текстуры и UV-прямоугольник (u0, v0, u1, v1) в 0..1.
#[derive(Clone, Copy)]
pub struct FaceTex {
    pub layer: u16,
    pub uv: [f32; 4],
}

impl FaceTex {
    pub fn full(layer: u16) -> Self {
        Self { layer, uv: [0.0, 0.0, 1.0, 1.0] }
    }
    pub fn none() -> Self {
        Self { layer: NO_TEXTURE, uv: [0.0; 4] }
    }
}

fn shade(c: [u8; 4], k: f32) -> [u8; 4] {
    [(c[0] as f32 * k) as u8, (c[1] as f32 * k) as u8, (c[2] as f32 * k) as u8, c[3]]
}

/// Кубоид [min, max] в локальных координатах, преобразованный `model`.
/// Грани: 0=+Y, 1=-Y, 2=+X, 3=-X, 4=+Z, 5=-Z.
pub fn push_box(out: &mut Vec<EntityVertex>, model: &Mat4, min: Vec3, max: Vec3, faces: &[FaceTex; 6], color: [u8; 4], light: u16) {
    let c = |x: f32, y: f32, z: f32| model.transform_point3(Vec3::new(x, y, z));
    let (a, b) = (min, max);
    // Углы граней против часовой стрелки снаружи (как в мешере чанков).
    let quads: [[Vec3; 4]; 6] = [
        [c(a.x, b.y, a.z), c(a.x, b.y, b.z), c(b.x, b.y, b.z), c(b.x, b.y, a.z)],
        [c(a.x, a.y, a.z), c(b.x, a.y, a.z), c(b.x, a.y, b.z), c(a.x, a.y, b.z)],
        [c(b.x, a.y, a.z), c(b.x, b.y, a.z), c(b.x, b.y, b.z), c(b.x, a.y, b.z)],
        [c(a.x, a.y, a.z), c(a.x, a.y, b.z), c(a.x, b.y, b.z), c(a.x, b.y, a.z)],
        [c(a.x, a.y, b.z), c(b.x, a.y, b.z), c(b.x, b.y, b.z), c(a.x, b.y, b.z)],
        [c(a.x, a.y, a.z), c(a.x, b.y, a.z), c(b.x, b.y, a.z), c(b.x, a.y, a.z)],
    ];
    for (f, q) in quads.iter().enumerate() {
        let t = faces[f];
        let [u0, v0, u1, v1] = t.uv;
        // UV углов: для боковых граней верх текстуры — сверху.
        let uv: [[f32; 2]; 4] = match f {
            0 | 1 => [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
            2 => [[u1, v1], [u1, v0], [u0, v0], [u0, v1]],
            3 => [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
            4 => [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
            _ => [[u1, v1], [u1, v0], [u0, v0], [u0, v1]],
        };
        let col = shade(color, SHADE[f]);
        let v = |i: usize| EntityVertex { pos: q[i].to_array(), uv: uv[i], color: col, extra: [t.layer, light] };
        out.extend_from_slice(&[v(0), v(1), v(2), v(2), v(3), v(0)]);
    }
}

/// Двусторонняя «карточка» (спрайт предмета) в плоскости XY локальных координат.
pub fn push_card(out: &mut Vec<EntityVertex>, model: &Mat4, size: f32, layer: u16, color: [u8; 4], light: u16) {
    let h = size * 0.5;
    let p = [
        model.transform_point3(Vec3::new(-h, -h, 0.0)),
        model.transform_point3(Vec3::new(h, -h, 0.0)),
        model.transform_point3(Vec3::new(h, h, 0.0)),
        model.transform_point3(Vec3::new(-h, h, 0.0)),
    ];
    let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let v = |i: usize| EntityVertex { pos: p[i].to_array(), uv: uv[i], color, extra: [layer, light] };
    out.extend_from_slice(&[v(0), v(1), v(2), v(2), v(3), v(0)]);
    out.extend_from_slice(&[v(0), v(2), v(1), v(0), v(3), v(2)]);
}

/// Рёбра бокса для пайплайна линий.
pub fn push_wire_box(out: &mut Vec<EntityVertex>, min: Vec3, max: Vec3, color: [u8; 4]) {
    let c = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in edges {
        for i in [a, b] {
            out.push(EntityVertex { pos: c[i].to_array(), uv: [0.0, 0.0], color, extra: [NO_TEXTURE, UNLIT] });
        }
    }
}

/// Полупрозрачная оверлей-грань (трещины разрушения) на 6 сторонах блока.
pub fn push_overlay_cube(out: &mut Vec<EntityVertex>, min: Vec3, max: Vec3, layer: u16, light: u16) {
    let model = Mat4::IDENTITY;
    let faces = [FaceTex::full(layer); 6];
    push_box(out, &model, min, max, &faces, [255, 255, 255, 255], light);
}
