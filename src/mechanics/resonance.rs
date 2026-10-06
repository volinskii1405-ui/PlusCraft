//! Резонансные кристаллы (уникальная механика). Резонит светится и
//! отзывается на добычу рядом: возникает эхо-импульс, который на несколько
//! секунд подсвечивает скрытые пустоты и руды сквозь породу. Импульс можно
//! вызвать и вручную — Резонатором. Но звук резонанса привлекает мобов.

use glam::{DVec3, IVec3, Vec3};

use crate::entities::mesh::{self, UNLIT};
use crate::renderer::vertex::EntityVertex;
use crate::world::block::{block_id, def, id};
use crate::world::World;

pub const PULSE_TIME: f32 = 7.0;

pub struct Marker {
    pub pos: IVec3,
    pub color: [u8; 4],
    pub ore: bool,
}

pub struct Pulse {
    pub origin: DVec3,
    pub radius: f32,
    pub age: f32,
    pub markers: Vec<Marker>,
}

fn ore_color(b: u8) -> Option<[u8; 4]> {
    Some(match b {
        id::COAL_ORE | id::DEEP_COAL_ORE => [60, 60, 60, 255],
        id::COPPER_ORE => [230, 130, 80, 255],
        id::IRON_ORE | id::DEEP_IRON_ORE => [230, 190, 160, 255],
        id::GOLD_ORE | id::DEEP_GOLD_ORE => [255, 220, 60, 255],
        id::DIAMOND_ORE | id::DEEP_DIAMOND_ORE => [90, 240, 230, 255],
        id::RESONITE_ORE => [190, 110, 255, 255],
        id::LAVA => [255, 100, 20, 255],
        id::WATER => [60, 120, 255, 255],
        _ => return None,
    })
}

/// Сканирует сферу: пустоты (замкнутые полости без неба) и руды.
pub fn scan(world: &World, origin: DVec3, radius: f32) -> Vec<Marker> {
    let c = origin.floor().as_ivec3();
    let r = radius as i32;
    let mut out = Vec::new();
    for dy in -r..=r {
        for dz in -r..=r {
            for dx in -r..=r {
                let d2 = (dx * dx + dy * dy + dz * dz) as f32;
                if d2 > radius * radius || d2 < 4.0 {
                    continue;
                }
                let p = c + IVec3::new(dx, dy, dz);
                let b = world.get_id(p.x, p.y, p.z);
                if let Some(col) = ore_color(b) {
                    // Жидкости — реже (иначе всё море в маркерах).
                    if (b == id::WATER || b == id::LAVA) && (p.x + p.y + p.z).rem_euclid(3) != 0 {
                        continue;
                    }
                    out.push(Marker { pos: p, color: col, ore: true });
                } else if b == id::AIR && (dx % 2 == 0) && (dy % 2 == 0) && (dz % 2 == 0) {
                    // Пустота: под землёй (нет неба) и граничит с породой.
                    let (sky, _) = world.light(p.x, p.y, p.z);
                    if sky > 0 {
                        continue;
                    }
                    let near_rock = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z]
                        .iter()
                        .any(|d| def(block_id(world.get(p.x + d.x, p.y + d.y, p.z + d.z))).solid);
                    if near_rock {
                        out.push(Marker { pos: p, color: [120, 220, 255, 255], ore: false });
                    }
                }
            }
        }
    }
    // Ограничиваем количество маркеров (производительность).
    if out.len() > 1500 {
        out.sort_by_key(|m| (!m.ore, (m.pos.as_dvec3().distance(origin) * 10.0) as i64));
        out.truncate(1500);
    }
    out
}

/// Есть ли резонит в радиусе от точки.
pub fn resonite_near(world: &World, p: IVec3, r: i32) -> Option<IVec3> {
    for dy in -r..=r {
        for dz in -r..=r {
            for dx in -r..=r {
                let q = p + IVec3::new(dx, dy, dz);
                if world.get_id(q.x, q.y, q.z) == id::RESONITE_ORE {
                    return Some(q);
                }
            }
        }
    }
    None
}

impl Pulse {
    pub fn new(world: &World, origin: DVec3, radius: f32) -> Self {
        Self { origin, radius, age: 0.0, markers: scan(world, origin, radius) }
    }

    /// Маркеры видны сквозь стены (рисуются в оверлее), волна расходится.
    pub fn build_mesh(&self, cam: DVec3, out: &mut Vec<EntityVertex>) {
        let fade = (1.0 - self.age / PULSE_TIME).clamp(0.0, 1.0);
        let wave = self.age * 12.0; // радиус волны
        for m in &self.markers {
            let center = m.pos.as_dvec3() + DVec3::splat(0.5);
            let dist = center.distance(self.origin) as f32;
            if dist > wave {
                continue; // волна ещё не дошла
            }
            let rel = (center - cam).as_vec3();
            let s = if m.ore { 0.22 } else { 0.08 };
            let mut col = m.color;
            col[3] = (fade * 255.0) as u8;
            // Свежие маркеры ярче (вспышка при прохождении волны).
            let flash = (1.0 - (wave - dist) / 4.0).clamp(0.0, 1.0);
            for c in col.iter_mut().take(3) {
                *c = (*c as f32 + (255.0 - *c as f32) * flash * 0.6) as u8;
            }
            let faces = [mesh::FaceTex::none(); 6];
            mesh::push_box(out, &glam::Mat4::IDENTITY, rel - Vec3::splat(s), rel + Vec3::splat(s), &faces, col, UNLIT);
        }
    }
}
