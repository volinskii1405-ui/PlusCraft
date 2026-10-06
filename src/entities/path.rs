//! Поиск пути A* по вокселям для мобов: шаги по горизонтали, подъём на
//! 1 блок, спуск до 3 блоков. Узел — клетка, где стоят ноги моба.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use glam::IVec3;

use crate::world::block::{block_id, def, id};
use crate::world::World;

/// Может ли моб высотой `height` блоков стоять в клетке `p`.
pub fn standable(world: &World, p: IVec3, height: i32) -> bool {
    if !world.is_loaded_at(p.x, p.z) {
        return false;
    }
    for dy in 0..height {
        let v = world.get(p.x, p.y + dy, p.z);
        let d = def(block_id(v));
        if d.solid || block_id(v) == id::LAVA {
            return false;
        }
    }
    let below = world.get(p.x, p.y - 1, p.z);
    def(block_id(below)).solid || block_id(world.get(p.x, p.y, p.z)) == id::WATER
}

#[derive(PartialEq)]
struct Node {
    f: f32,
    p: IVec3,
}

impl Eq for Node {}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Мин-куча по f.
        other.f.total_cmp(&self.f)
    }
}

fn heuristic(a: IVec3, b: IVec3) -> f32 {
    let d = (a - b).abs();
    (d.x + d.z) as f32 + d.y as f32 * 0.5
}

/// Ищет путь от `start` к `goal`. Возвращает клетки пути (без стартовой).
/// Если цель недостижима за `max_nodes` шагов — путь к ближайшей найденной клетке.
pub fn find_path(world: &World, start: IVec3, goal: IVec3, height: i32, max_nodes: usize) -> Option<Vec<IVec3>> {
    if start == goal {
        return Some(vec![]);
    }
    let mut open = BinaryHeap::new();
    let mut came: HashMap<IVec3, IVec3> = HashMap::new();
    let mut g: HashMap<IVec3, f32> = HashMap::new();
    open.push(Node { f: heuristic(start, goal), p: start });
    g.insert(start, 0.0);
    let mut best = (heuristic(start, goal), start);
    let mut expanded = 0;
    while let Some(Node { p, .. }) = open.pop() {
        if p == goal {
            best = (0.0, p);
            break;
        }
        expanded += 1;
        if expanded > max_nodes {
            break;
        }
        let gp = g[&p];
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let diagonal = dx != 0 && dz != 0;
            // Диагональ только если оба соседних прямых шага свободны (не режем углы).
            if diagonal
                && (!standable(world, p + IVec3::new(dx, 0, 0), height) || !standable(world, p + IVec3::new(0, 0, dz), height))
            {
                continue;
            }
            let base = p + IVec3::new(dx, 0, dz);
            let mut target = None;
            if standable(world, base, height) {
                target = Some((base, 0.0));
            } else if !diagonal
                && standable(world, base + IVec3::Y, height)
                && !def(world.get_id(p.x, p.y + height, p.z)).solid
            {
                // Подъём на блок (прыжок).
                target = Some((base + IVec3::Y, 0.6));
            } else if !diagonal {
                // Спуск до 3 блоков.
                for drop in 1..=3 {
                    let q = base - IVec3::Y * drop;
                    let mut clear = true;
                    for k in 0..drop {
                        if def(world.get_id(base.x, base.y - k, base.z)).solid {
                            clear = false;
                        }
                    }
                    if !clear {
                        break;
                    }
                    if standable(world, q, height) {
                        target = Some((q, 0.3 * drop as f32));
                        break;
                    }
                }
            }
            let Some((q, extra)) = target else { continue };
            let step = if diagonal { 1.414 } else { 1.0 };
            let ng = gp + step + extra;
            if g.get(&q).map(|&old| ng < old).unwrap_or(true) {
                g.insert(q, ng);
                came.insert(q, p);
                let h = heuristic(q, goal);
                if h < best.0 {
                    best = (h, q);
                }
                open.push(Node { f: ng + h, p: q });
            }
        }
    }
    let end = best.1;
    if end == start {
        return None;
    }
    let mut path = vec![end];
    let mut cur = end;
    while let Some(&prev) = came.get(&cur) {
        if prev == start {
            break;
        }
        path.push(prev);
        cur = prev;
    }
    path.reverse();
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::make;
    use crate::world::chunk::{Chunk, ChunkData, ChunkPos};

    fn arena() -> World {
        let mut w = World::new(1, None, false);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut d = ChunkData::empty();
                for z in 0..16 {
                    for x in 0..16 {
                        d.set(x, 10, z, make(id::STONE, 0));
                    }
                }
                let p = ChunkPos::new(cx, cz);
                w.chunks.insert(p, Chunk::new(p, d));
            }
        }
        w
    }

    #[test]
    fn path_around_wall() {
        let mut w = arena();
        // Стена x=5, z от -3 до 3, высотой 3 — обходить сбоку.
        for z in -3..=3 {
            for y in 11..14 {
                w.set(5, y, z, make(id::STONE, 0));
            }
        }
        let p = find_path(&w, IVec3::new(2, 11, 0), IVec3::new(8, 11, 0), 2, 2000).expect("путь");
        assert_eq!(*p.last().unwrap(), IVec3::new(8, 11, 0));
        assert!(p.iter().all(|c| c.x != 5 || c.z.abs() > 3), "путь прошёл сквозь стену: {p:?}");
    }

    #[test]
    fn path_climbs_one_block() {
        let mut w = arena();
        for z in -8..=8 {
            for x in 5..10 {
                w.set(x, 11, z, make(id::STONE, 0));
            }
        }
        let p = find_path(&w, IVec3::new(2, 11, 0), IVec3::new(7, 12, 0), 2, 2000).expect("путь");
        assert_eq!(*p.last().unwrap(), IVec3::new(7, 12, 0));
    }
}
