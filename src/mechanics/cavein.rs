//! Нестабильные породы (уникальная механика): после добычи блока порода
//! в потолке без опоры может обрушиться. Крепи (блок «Крепь») в радиусе
//! 4 блоков удерживают свод.
//!
//! Правило: «висящий» блок неустойчивой породы (камень, руды, булыжник)
//! с пустотой снизу считается опасным, если до ближайшей стены/столба по
//! потолку не меньше `MIN_SPAN` блоков и рядом нет крепи. Тогда с некоторой
//! вероятностью через 1.5–3.5 с он и неустойчивые блоки над ним падают.

use glam::{DVec3, IVec3};

use crate::world::block::{block_id, def, id, make};
use crate::world::noise::Rng;
use crate::world::World;

pub const SUPPORT_RADIUS: i32 = 4;
const MIN_SPAN: i32 = 3;
const CHANCE: f32 = 0.3;
/// Максимум блоков, «затрещавших» от одной добычи.
const MAX_PER_EVENT: usize = 3;

pub struct Pending {
    pub pos: IVec3,
    pub timer: f32,
}

#[derive(Default)]
pub struct CaveIns {
    pub pending: Vec<Pending>,
    pub total: u32,
}

/// Результат обрушения для игры: куда упали блоки (для урона игроку).
pub struct Collapse {
    pub columns: Vec<(IVec3, i32)>,
}

fn hanging(world: &World, p: IVec3) -> bool {
    let v = world.get(p.x, p.y, p.z);
    let d = def(block_id(v));
    d.unstable && !def(world.get_id(p.x, p.y - 1, p.z)).solid
}

/// Есть ли крепь в радиусе вокруг точки (ниже или на уровне свода).
pub fn supported(world: &World, p: IVec3) -> bool {
    let r = SUPPORT_RADIUS;
    for dy in -5..=1 {
        for dz in -r..=r {
            for dx in -r..=r {
                if world.get_id(p.x + dx, p.y + dy, p.z + dz) == id::SUPPORT {
                    return true;
                }
            }
        }
    }
    false
}

/// Расстояние по своду до ближайшей опоры (клетки с твёрдым блоком снизу).
pub fn span(world: &World, p: IVec3) -> i32 {
    let mut best = 99;
    for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
        for k in 1..=6 {
            let q = p + d * k;
            let solid_here = def(world.get_id(q.x, q.y, q.z)).solid;
            if !solid_here {
                // Свод прерывается — опоры в этом направлении нет.
                break;
            }
            if def(world.get_id(q.x, q.y - 1, q.z)).solid {
                best = best.min(k);
                break;
            }
        }
    }
    best
}

impl CaveIns {
    /// Проверка после удаления блока игроком. Возвращает true, если свод
    /// «затрещал» (запланирован обвал).
    pub fn on_block_removed(&mut self, world: &World, p: IVec3, rng: &mut Rng) -> bool {
        // Кандидаты — висящие блоки свода рядом, ближайшие первыми.
        let mut cands = Vec::new();
        for dy in 0..=3 {
            for dz in -2..=2 {
                for dx in -2..=2 {
                    let q = p + IVec3::new(dx, dy, dz);
                    if !hanging(world, q) || self.pending.iter().any(|x| x.pos == q) {
                        continue;
                    }
                    if span(world, q) < MIN_SPAN || supported(world, q) {
                        continue;
                    }
                    cands.push(q);
                }
            }
        }
        cands.sort_by_key(|q| (*q - p).length_squared());
        let mut n = 0;
        for q in cands {
            if n >= MAX_PER_EVENT {
                break;
            }
            if rng.chance(CHANCE) {
                self.pending.push(Pending { pos: q, timer: 1.5 + rng.f32() * 2.0 });
                n += 1;
            }
        }
        n > 0
    }

    /// Тик: срабатывание запланированных обвалов.
    pub fn tick(&mut self, world: &mut World, dt: f32, rng: &mut Rng) -> Option<Collapse> {
        if self.pending.is_empty() {
            return None;
        }
        let mut fired = Vec::new();
        self.pending.retain_mut(|x| {
            x.timer -= dt;
            if x.timer <= 0.0 {
                fired.push(x.pos);
                false
            } else {
                true
            }
        });
        if fired.is_empty() {
            return None;
        }
        let mut columns = Vec::new();
        for p in fired {
            // К моменту обвала могли поставить крепь.
            if !hanging(world, p) || supported(world, p) {
                continue;
            }
            // Падает блок и неустойчивые блоки над ним (до 3).
            let mut top = p.y;
            while top - p.y < 3 && def(world.get_id(p.x, top + 1, p.z)).unstable {
                top += 1;
            }
            let mut land = p.y;
            while land > 1 && def(world.get_id(p.x, land - 1, p.z)).replaceable {
                land -= 1;
            }
            for y in (p.y..=top).rev() {
                world.set(p.x, y, p.z, 0);
            }
            for i in 0..=(top - p.y) {
                // Порода дробится: гравий и булыжник.
                let b = if rng.chance(0.5) { id::GRAVEL } else { id::COBBLE };
                world.set(p.x, land + i, p.z, make(b, 0));
            }
            self.total += 1;
            columns.push((IVec3::new(p.x, land, p.z), top - p.y + 1));
        }
        if columns.is_empty() {
            None
        } else {
            Some(Collapse { columns })
        }
    }

    /// Есть ли угроза обвала рядом с точкой (для подсказки игроку).
    pub fn danger_near(&self, p: DVec3) -> bool {
        self.pending.iter().any(|x| x.pos.as_dvec3().distance(p) < 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::{Chunk, ChunkData, ChunkPos};

    /// Каменный монолит с залом 9×3×9 (y 50..52).
    fn hall() -> World {
        let mut w = World::new(1, None, false);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut d = ChunkData::empty();
                for y in 1..70 {
                    for z in 0..16 {
                        for x in 0..16 {
                            d.set(x, y, z, make(id::STONE, 0));
                        }
                    }
                }
                let p = ChunkPos::new(cx, cz);
                w.chunks.insert(p, Chunk::new(p, d));
            }
        }
        for y in 50..53 {
            for z in -4..=4 {
                for x in -4..=4 {
                    w.set(x, y, z, 0);
                }
            }
        }
        w
    }

    #[test]
    fn wide_ceiling_collapses_without_support() {
        let mut w = hall();
        let mut c = CaveIns::default();
        let mut rng = Rng::new(3);
        // Центр свода — далеко от стен.
        assert!(span(&w, IVec3::new(0, 53, 0)) >= MIN_SPAN);
        let mut any = false;
        for _ in 0..10 {
            any |= c.on_block_removed(&w, IVec3::new(0, 52, 0), &mut rng);
        }
        assert!(any, "обвал должен быть запланирован");
        let mut collapsed = false;
        for _ in 0..100 {
            if c.tick(&mut w, 0.1, &mut rng).is_some() {
                collapsed = true;
            }
        }
        assert!(collapsed);
        assert!(c.total > 0);
    }

    #[test]
    fn support_prevents_collapse() {
        let mut w = hall();
        w.set(0, 50, 1, make(id::SUPPORT, 0));
        w.set(0, 51, 1, make(id::SUPPORT, 0));
        let mut c = CaveIns::default();
        let mut rng = Rng::new(3);
        for _ in 0..20 {
            c.on_block_removed(&w, IVec3::new(0, 52, 0), &mut rng);
        }
        assert!(c.pending.is_empty(), "крепь должна удерживать свод");
    }

    #[test]
    fn narrow_tunnel_is_stable() {
        let mut w = hall();
        // Узкий туннель шириной 1: свод опирается на стены.
        for y in 50..52 {
            w.set(10, y, 0, 0);
        }
        assert!(span(&w, IVec3::new(10, 52, 0)) < MIN_SPAN);
    }
}
