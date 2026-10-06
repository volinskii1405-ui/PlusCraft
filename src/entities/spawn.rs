//! Спавн и деспавн мобов: по биому, освещению, времени суток и глубине.

use glam::DVec3;

use crate::settings::{Difficulty, MobDifficulty};
use crate::world::biome::BiomeId;
use crate::world::block::{block_id, id};
use crate::world::noise::Rng;
use crate::world::World;

use super::mob::{Mob, MobKind, Mutation};
use super::path::standable;

/// Эффективная освещённость клетки (0..15) с учётом времени суток.
pub fn effective_light(world: &World, x: i32, y: i32, z: i32, daylight: f32) -> u8 {
    let (sky, blk) = world.light(x, y, z);
    let s = (sky as f32 * daylight).round() as u8;
    s.max(blk)
}

pub struct SpawnRules {
    pub daylight: f32,
    pub difficulty: Difficulty,
    pub mob_difficulty: MobDifficulty,
}

/// Одна попытка заспавнить мобов вокруг игрока.
pub fn try_spawn(world: &World, mobs: &mut Vec<Mob>, player: DVec3, rules: &SpawnRules, rng: &mut Rng) {
    let near = |m: &&Mob| m.pos.distance(player) < 72.0;
    let hostile = mobs.iter().filter(near).filter(|m| m.kind.hostile()).count();
    let peaceful = mobs.iter().filter(near).filter(|m| !m.kind.hostile()).count();
    let hostile_cap = if rules.difficulty == Difficulty::Peaceful {
        0
    } else {
        (9.0 * rules.mob_difficulty.mul()).round() as usize
    };
    let peaceful_cap = 8;

    for _ in 0..3 {
        let ang = rng.f32() * std::f32::consts::TAU;
        let dist = 20.0 + rng.f32() * 30.0;
        let x = (player.x + (ang.cos() * dist) as f64).floor() as i32;
        let z = (player.z + (ang.sin() * dist) as f64).floor() as i32;
        if !world.is_loaded_at(x, z) {
            continue;
        }
        let Some(surface) = world.surface_y(x, z) else { continue };
        let biome = world.biome_at(x, z);

        // Мирные — днём на траве под открытым небом.
        if peaceful < peaceful_cap && rules.daylight > 0.5 && rng.chance(0.25) {
            let y = surface + 1;
            let below = world.get_id(x, surface, z);
            if (below == id::GRASS || below == id::SNOW_GRASS) && standable(world, glam::IVec3::new(x, y, z), 2) {
                let kind = match biome {
                    b if b == BiomeId::Forest as u8 || b == BiomeId::SnowyTaiga as u8 => MobKind::Boar,
                    b if b == BiomeId::Plains as u8 || b == BiomeId::Mountains as u8 => MobKind::Grazer,
                    _ => continue,
                };
                let n = rng.range(1, 4);
                for i in 0..n {
                    let p = DVec3::new(x as f64 + 0.5 + i as f64 * 0.7, y as f64, z as f64 + 0.5);
                    mobs.push(Mob::new(kind, p, Mutation::default(), rng));
                }
                return;
            }
        }

        if hostile >= hostile_cap {
            continue;
        }
        // Враждебные: под землёй в темноте или ночью на поверхности.
        let underground = rng.chance(0.65);
        let y = if underground {
            let top = (surface - 4).max(6);
            let start = rng.range(4, top.max(5));
            // Ищем пол вверх от случайной высоты.
            let mut found = None;
            for yy in start..(start + 24).min(top) {
                if standable(world, glam::IVec3::new(x, yy, z), 2) {
                    found = Some(yy);
                    break;
                }
            }
            match found {
                Some(v) => v,
                None => continue,
            }
        } else {
            surface + 1
        };
        if !standable(world, glam::IVec3::new(x, y, z), 2) {
            continue;
        }
        let light = effective_light(world, x, y, z, rules.daylight);
        if light > 6 {
            continue;
        }
        let (sky, blk) = world.light(x, y, z);
        let kind = if sky == 0 && blk == 0 && underground && rng.chance(0.3) {
            // Мракоед — только в абсолютной темноте.
            MobKind::Gloom
        } else if underground && rng.chance(0.4) {
            MobKind::Spider
        } else {
            MobKind::Crawler
        };
        if block_id(world.get(x, y - 1, z)) == id::WATER {
            continue;
        }
        let mutation = Mutation::for_depth(y as f64, rng);
        let p = DVec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
        mobs.push(Mob::new(kind, p, mutation, rng));
        return;
    }
}

/// Удаляет далёких мобов (и иногда — средне-далёких враждебных).
pub fn despawn(mobs: &mut Vec<Mob>, player: DVec3, dt: f32, rng: &mut Rng) {
    mobs.retain(|m| {
        let d = m.pos.distance(player);
        if d > 110.0 || m.pos.y < -20.0 {
            return false;
        }
        if m.kind.hostile() && d > 40.0 && rng.chance(dt / 30.0) {
            return false;
        }
        true
    });
}
