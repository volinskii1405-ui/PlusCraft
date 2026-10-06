//! Физика: AABB-коллизии с вокселями (раздельно по осям, со «стреловидным»
//! объёмом перемещения — без туннелирования на высоких скоростях) и
//! DDA-raycast для выбора блока.

use glam::{DVec3, IVec3, Vec3};

use crate::world::block::{block_id, def, Shape};
use crate::world::World;

pub const EPS: f64 = 1e-7;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    /// Бокс по позиции ног, ширине и высоте.
    pub fn from_feet(pos: DVec3, width: f64, height: f64) -> Self {
        let h = width * 0.5;
        Self { min: DVec3::new(pos.x - h, pos.y, pos.z - h), max: DVec3::new(pos.x + h, pos.y + height, pos.z + h) }
    }

    pub fn offset(&self, d: DVec3) -> Self {
        Self { min: self.min + d, max: self.max + d }
    }

    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x - EPS
            && self.max.x > o.min.x + EPS
            && self.min.y < o.max.y - EPS
            && self.max.y > o.min.y + EPS
            && self.min.z < o.max.z - EPS
            && self.max.z > o.min.z + EPS
    }

    pub fn block(x: i32, y: i32, z: i32) -> Self {
        Self {
            min: DVec3::new(x as f64, y as f64, z as f64),
            max: DVec3::new(x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0),
        }
    }

    /// Целочисленные ячейки, которые пересекает бокс.
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32, i32)> {
        let x0 = (self.min.x + EPS).floor() as i32;
        let y0 = (self.min.y + EPS).floor() as i32;
        let z0 = (self.min.z + EPS).floor() as i32;
        let x1 = (self.max.x - EPS).floor() as i32;
        let y1 = (self.max.y - EPS).floor() as i32;
        let z1 = (self.max.z - EPS).floor() as i32;
        (y0..=y1).flat_map(move |y| (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| (x, y, z))))
    }
}

/// Есть ли твёрдые блоки внутри бокса.
pub fn collides(world: &World, b: &Aabb) -> bool {
    b.cells().any(|(x, y, z)| world.is_solid(x, y, z))
}

/// Перемещает бокс на `delta` с коллизиями. Порядок осей: Y, X, Z.
/// Возвращает фактическое смещение и флаги столкновений по осям.
pub fn move_box(world: &World, b: &mut Aabb, delta: DVec3) -> (DVec3, [bool; 3]) {
    let mut actual = DVec3::ZERO;
    let mut hit = [false; 3];
    for axis in [1usize, 0, 2] {
        let d = delta[axis];
        if d == 0.0 {
            continue;
        }
        let allowed = clip_axis(world, b, axis, d);
        if (allowed - d).abs() > 1e-9 {
            hit[axis] = true;
        }
        let mut off = DVec3::ZERO;
        off[axis] = allowed;
        *b = b.offset(off);
        actual[axis] = allowed;
    }
    (actual, hit)
}

/// Насколько можно сдвинуть бокс вдоль оси, не войдя в твёрдый блок.
fn clip_axis(world: &World, b: &Aabb, axis: usize, d: f64) -> f64 {
    // Объём, заметаемый боксом при движении.
    let mut sweep = *b;
    if d > 0.0 {
        sweep.max[axis] += d;
    } else {
        sweep.min[axis] += d;
    }
    let mut allowed = d;
    for (x, y, z) in sweep.cells() {
        if !world.is_solid(x, y, z) {
            continue;
        }
        let blk = Aabb::block(x, y, z);
        // Перекрытие по двум другим осям.
        let overlap = (0..3).filter(|&a| a != axis).all(|a| blk.max[a] > b.min[a] + EPS && blk.min[a] < b.max[a] - EPS);
        if !overlap {
            continue;
        }
        if d > 0.0 && blk.min[axis] >= b.max[axis] - EPS {
            allowed = allowed.min(blk.min[axis] - b.max[axis]);
        } else if d < 0.0 && blk.max[axis] <= b.min[axis] + EPS {
            allowed = allowed.max(blk.max[axis] - b.min[axis]);
        }
    }
    if d > 0.0 {
        allowed.max(0.0)
    } else {
        allowed.min(0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub pos: IVec3,
    /// Нормаль грани, в которую попали.
    pub normal: IVec3,
    pub dist: f32,
}

/// DDA-обход ячеек вдоль луча (Amanatides & Woo). Цель — любой блок, для
/// которого `pick` возвращает true.
pub fn raycast(world: &World, origin: DVec3, dir: Vec3, max_dist: f32, pick: impl Fn(u16) -> bool) -> Option<RayHit> {
    let dir = dir.normalize_or_zero().as_dvec3();
    if dir == DVec3::ZERO {
        return None;
    }
    let mut cell = origin.floor().as_ivec3();
    let step = IVec3::new(dir.x.signum() as i32, dir.y.signum() as i32, dir.z.signum() as i32);
    let next_boundary = |o: f64, c: i32, s: i32| if s > 0 { c as f64 + 1.0 - o } else { o - c as f64 };
    let inv = |v: f64| if v.abs() < 1e-12 { f64::INFINITY } else { 1.0 / v.abs() };
    let t_delta = DVec3::new(inv(dir.x), inv(dir.y), inv(dir.z));
    let mut t_max = DVec3::new(
        next_boundary(origin.x, cell.x, step.x) * t_delta.x,
        next_boundary(origin.y, cell.y, step.y) * t_delta.y,
        next_boundary(origin.z, cell.z, step.z) * t_delta.z,
    );
    let mut normal = IVec3::ZERO;
    let mut t = 0.0;
    while t <= max_dist as f64 {
        let v = world.get(cell.x, cell.y, cell.z);
        if v != 0 && pick(v) {
            return Some(RayHit { pos: cell, normal, dist: t as f32 });
        }
        if t_max.x < t_max.y && t_max.x < t_max.z {
            cell.x += step.x;
            t = t_max.x;
            t_max.x += t_delta.x;
            normal = IVec3::new(-step.x, 0, 0);
        } else if t_max.y < t_max.z {
            cell.y += step.y;
            t = t_max.y;
            t_max.y += t_delta.y;
            normal = IVec3::new(0, -step.y, 0);
        } else {
            cell.z += step.z;
            t = t_max.z;
            t_max.z += t_delta.z;
            normal = IVec3::new(0, 0, -step.z);
        }
    }
    None
}

/// Фильтр для выбора блока: всё, кроме воздуха и жидкостей.
pub fn pickable(v: u16) -> bool {
    let d = def(block_id(v));
    d.shape != Shape::Air && d.shape != Shape::Liquid
}
