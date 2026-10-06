//! Скриптовая самопроверка игровых механик (`--selftest`): прогоняет
//! сценарии на реальном мире и пишет PASS/FAIL в лог. Код выхода 0 — все
//! проверки пройдены.

use glam::{DVec3, IVec3};

use crate::game::{break_time, Game};
use crate::inventory::ItemStack;
use crate::item;
use crate::physics::RayHit;
use crate::player::GameMode;
use crate::world::block::{self, block_id, id};

pub struct SelfTest {
    phase: usize,
    timer: f32,
    pub results: Vec<(String, bool, String)>,
    expect_item: u16,
    ground_y: f64,
    pub done: bool,
}

impl SelfTest {
    pub fn new() -> Self {
        Self { phase: 0, timer: 0.0, results: Vec::new(), expect_item: 0, ground_y: 0.0, done: false }
    }

    fn check(&mut self, name: &str, ok: bool, info: String) {
        log::info!("[selftest] {} {name}: {info}", if ok { "PASS" } else { "FAIL" });
        self.results.push((name.to_string(), ok, info));
    }

    pub fn all_passed(&self) -> bool {
        !self.results.is_empty() && self.results.iter().all(|r| r.1)
    }

    fn next(&mut self) {
        self.phase += 1;
        self.timer = 0.0;
    }

    fn feet(game: &Game) -> IVec3 {
        let p = game.player.pos;
        IVec3::new(p.x.floor() as i32, (p.y + 0.01).floor() as i32, p.z.floor() as i32)
    }

    pub fn update(&mut self, game: &mut Game, dt: f32) {
        if self.done {
            return;
        }
        self.timer += dt;
        match self.phase {
            // Ждём загрузки мира и приземления.
            0 => {
                if game.player.on_ground && game.world.chunks.len() > 25 {
                    let f = Self::feet(game);
                    let below = game.world.get_id(f.x, f.y - 1, f.z);
                    self.ground_y = game.player.pos.y;
                    self.check(
                        "spawn_landing",
                        block::def(below).solid,
                        format!("игрок стоит на {} на высоте {:.2}", block::def(below).name, game.player.pos.y),
                    );
                    self.next();
                } else if self.timer > 40.0 {
                    self.check("spawn_landing", false, "мир не загрузился за 40 с".into());
                    self.phase = 99;
                }
            }
            // Добыча блока под ногами каменной киркой.
            1 => {
                game.player.mode = GameMode::Survival;
                game.player.inventory.slots[0] = Some(ItemStack::new(item::tool_id(block::ToolKind::Pickaxe, 2), 1));
                game.player.inventory.selected = 0;
                let f = Self::feet(game);
                let pos = f - IVec3::Y;
                let v = game.world.get(pos.x, pos.y, pos.z);
                let (t, harvest) = break_time(v, game.player.inventory.selected_stack());
                let items_before = game.entities.items.len();
                game.break_block(pos, true);
                let after = game.world.get_id(pos.x, pos.y, pos.z);
                let dropped = game.entities.items.len() > items_before;
                self.expect_item = game.entities.items.last().map(|i| i.stack.item).unwrap_or(0);
                let dur = game.player.inventory.slots[0].map(|s| s.durability).unwrap_or(0);
                self.check(
                    "break_block",
                    after == id::AIR && dropped && harvest && t.is_finite(),
                    format!(
                        "{} разрушен за {:.2} с (расчёт), выпал предмет: {}, прочность кирки {}",
                        block::def(block_id(v)).name,
                        t,
                        if dropped { item::name(self.expect_item) } else { "нет" },
                        dur
                    ),
                );
                self.next();
            }
            // Ждём, пока игрок упадёт в яму и подберёт предмет.
            2 => {
                if self.timer > 2.5 {
                    let n = game.player.inventory.count(self.expect_item);
                    self.check("pickup", n >= 1, format!("в инвентаре {} × {}", n, item::name(self.expect_item)));
                    self.next();
                }
            }
            // Установка блока.
            3 => {
                game.player.inventory.slots[1] = Some(ItemStack::new(id::PLANKS as u16, 10));
                game.player.inventory.selected = 1;
                let f = Self::feet(game);
                // Ищем рядом твёрдый блок с пустотой сверху, не пересекающий игрока.
                let mut placed = None;
                'search: for dz in -3i32..=3 {
                    for dx in -3i32..=3 {
                        if dx.abs() < 2 && dz.abs() < 2 {
                            continue;
                        }
                        for dy in -2..=2 {
                            let p = f + IVec3::new(dx, dy, dz);
                            if game.world.is_solid(p.x, p.y, p.z) && block::def(game.world.get_id(p.x, p.y + 1, p.z)).replaceable {
                                let hit = RayHit { pos: p, normal: IVec3::Y, dist: 2.0 };
                                if game.try_place(hit) {
                                    placed = Some(p + IVec3::Y);
                                    break 'search;
                                }
                            }
                        }
                    }
                }
                let ok = placed.map(|p| game.world.get_id(p.x, p.y, p.z) == id::PLANKS).unwrap_or(false);
                let left = game.player.inventory.count(id::PLANKS as u16);
                self.check("place_block", ok && left == 9, format!("доски поставлены в {placed:?}, осталось {left}"));
                self.next();
            }
            // Урон от падения с 10 блоков.
            4 => {
                let f = Self::feet(game);
                let top = game.world.surface_y(f.x, f.z).unwrap_or(f.y - 1);
                game.player.health = 20.0;
                game.player.pos = DVec3::new(f.x as f64 + 0.5, top as f64 + 11.0, f.z as f64 + 0.5);
                game.player.prev_pos = game.player.pos;
                game.player.vel = DVec3::ZERO;
                game.player.on_ground = false;
                self.ground_y = top as f64 + 1.0;
                self.next();
            }
            5 => {
                if game.player.on_ground && self.timer > 0.2 {
                    let dmg = 20.0 - game.player.health;
                    self.check(
                        "fall_damage",
                        (6.0..=8.0).contains(&dmg),
                        format!("падение ~10 блоков: урон {dmg:.0} (ожидалось 7)"),
                    );
                    self.next();
                } else if self.timer > 6.0 {
                    self.check("fall_damage", false, "не приземлился".into());
                    self.next();
                }
            }
            // Высокая скорость: без проваливания сквозь землю.
            6 => {
                game.player.mode = GameMode::Creative;
                game.player.flying = false;
                let f = Self::feet(game);
                let top = game.world.surface_y(f.x, f.z).unwrap_or(f.y - 1);
                self.ground_y = top as f64 + 1.0;
                game.player.pos = DVec3::new(f.x as f64 + 0.5, top as f64 + 80.0, f.z as f64 + 0.5);
                game.player.prev_pos = game.player.pos;
                game.player.vel = DVec3::new(0.0, -500.0, 0.0);
                self.next();
            }
            7 => {
                if game.player.on_ground && self.timer > 0.2 {
                    let y = game.player.pos.y;
                    self.check(
                        "no_tunnelling",
                        (y - self.ground_y).abs() < 0.01,
                        format!("приземлился на y={y:.3}, поверхность {:.1}", self.ground_y),
                    );
                    self.next();
                } else if self.timer > 8.0 {
                    self.check("no_tunnelling", false, format!("y={:.2}", game.player.pos.y));
                    self.next();
                }
            }
            // Стена: разгон в блок не должен заводить внутрь.
            8 => {
                let f = Self::feet(game);
                for dy in 0..3 {
                    game.world.set(f.x + 1, f.y + dy, f.z, block::make(id::STONE, 0));
                }
                game.player.vel = DVec3::new(200.0, 0.0, 0.0);
                self.next();
            }
            9 => {
                if self.timer > 0.5 {
                    let f = Self::feet(game);
                    let max_x = game.player.aabb().max.x;
                    let wall = (f.x + 1) as f64;
                    self.check("wall_collision", max_x <= wall + 1e-6, format!("правый край игрока {max_x:.4}, стена {wall}"));
                    self.next();
                }
            }
            // Пересчёт света: факел рядом с игроком, затем его удаление.
            10 => {
                let f = Self::feet(game);
                let p = f + IVec3::new(0, 0, -1);
                if game.world.get_id(p.x, p.y, p.z) != id::AIR {
                    game.world.set(p.x, p.y, p.z, 0);
                }
                game.world.set(p.x, p.y, p.z, block::make(id::TORCH, 255));
                self.expect_item = 0;
                self.ground_y = 0.0;
                self.next();
            }
            11 => {
                if self.timer > 2.0 {
                    let f = Self::feet(game);
                    let p = f + IVec3::new(0, 0, -1);
                    let (_, b) = game.world.light(p.x, p.y, p.z);
                    // Соседняя непрозрачная для света клетка должна получить 13.
                    let neighbors = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::Z, IVec3::NEG_Z];
                    let mut b2 = None;
                    for d in neighbors {
                        let q = p + d;
                        if !block::is_opaque(game.world.get(q.x, q.y, q.z)) {
                            b2 = Some(game.world.light(q.x, q.y, q.z).1);
                            break;
                        }
                    }
                    self.check(
                        "torch_light",
                        b == 14 && b2.map(|v| v == 13).unwrap_or(true),
                        format!("блочный свет у факела {b}, в соседней клетке {b2:?}"),
                    );
                    game.world.set(p.x, p.y, p.z, 0);
                    self.next();
                }
            }
            12 => {
                if self.timer > 2.0 {
                    let f = Self::feet(game);
                    let p = f + IVec3::new(0, 0, -1);
                    let (_, b) = game.world.light(p.x, p.y, p.z);
                    self.check("light_removed", b == 0, format!("после удаления факела блочный свет {b}"));
                    self.next();
                }
            }
            _ => {
                let passed = self.results.iter().filter(|r| r.1).count();
                log::info!("[selftest] итог: {passed}/{} проверок пройдено", self.results.len());
                self.done = true;
            }
        }
    }
}
