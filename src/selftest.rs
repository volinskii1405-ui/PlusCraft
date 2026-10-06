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
    hall: IVec3,
    pub done: bool,
}

impl SelfTest {
    pub fn new() -> Self {
        Self { phase: 0, timer: 0.0, results: Vec::new(), expect_item: 0, ground_y: 0.0, hall: IVec3::ZERO, done: false }
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
            // Печь: плавка железа на угле, горящая печь меняет блок.
            13 => {
                let f = Self::feet(game);
                let p = f + IVec3::new(0, 0, 2);
                game.world.set(p.x, p.y, p.z, block::make(id::FURNACE, 4));
                let fur = game.block_entities.furnace_mut(p);
                fur.input = Some(ItemStack::new(item::id::RAW_IRON, 1));
                fur.fuel = Some(ItemStack::new(item::id::COAL, 1));
                self.next();
            }
            14 => {
                let p = Self::feet(game) + IVec3::new(0, 0, 2);
                if self.timer > 1.0 && self.ground_y == 0.0 {
                    // Отметим, что печь загорелась.
                    self.ground_y = if game.world.get_id(p.x, p.y, p.z) == id::FURNACE_LIT { 1.0 } else { -1.0 };
                }
                if self.timer > 10.0 {
                    let out = game.block_entities.furnace_mut(p).output;
                    let lit = self.ground_y > 0.0;
                    self.check(
                        "furnace_smelt",
                        out.map(|o| o.item) == Some(item::id::IRON_INGOT) && lit,
                        format!("выход печи: {:?}, горела: {lit}", out.map(|o| item::name(o.item))),
                    );
                    self.next();
                }
            }
            // Сундук: содержимое выпадает при разрушении.
            15 => {
                let p = Self::feet(game) + IVec3::new(2, 0, 0);
                game.world.set(p.x, p.y, p.z, block::make(id::CHEST, 4));
                let chest = game.block_entities.chest_mut(p, crate::blockentity::Chest::default);
                chest.slots[3] = Some(ItemStack::new(item::id::DIAMOND, 5));
                let before = game.entities.items.len();
                game.break_block(p, true);
                let diamonds: u32 = game.entities.items[before..]
                    .iter()
                    .filter(|i| i.stack.item == item::id::DIAMOND)
                    .map(|i| i.stack.count as u32)
                    .sum();
                self.check("chest_drops", diamonds == 5, format!("выпало алмазов: {diamonds}"));
                self.next();
            }
            // Моб атакует игрока в выживании.
            16 => {
                game.player.mode = GameMode::Survival;
                game.player.health = 20.0;
                // Чистая площадка 11×11 с каменным полом вокруг игрока.
                let f = Self::feet(game);
                game.run_command(&format!("/fill {} {} {} {} {} {} air", f.x - 5, f.y, f.z - 5, f.x + 5, f.y + 4, f.z + 5));
                game.run_command(&format!("/fill {} {} {} {} {} {} stone", f.x - 5, f.y - 1, f.z - 5, f.x + 5, f.y - 1, f.z + 5));
                game.player.pos = DVec3::new(f.x as f64 + 0.5, f.y as f64, f.z as f64 + 0.5);
                game.player.prev_pos = game.player.pos;
                let p = game.player.pos + DVec3::new(3.0, 0.2, 0.0);
                let m = crate::entities::mob::Mob::new(
                    crate::entities::mob::MobKind::Spider,
                    p,
                    crate::entities::mob::Mutation::default(),
                    &mut crate::world::noise::Rng::new(1),
                );
                game.entities.mobs.push(m);
                self.next();
            }
            17 => {
                if game.player.health < 20.0 {
                    self.check("mob_attacks", true, format!("паук атаковал, здоровье {:.0}", game.player.health));
                    self.next();
                } else if self.timer > 8.0 {
                    let info: Vec<String> = game
                        .entities
                        .mobs
                        .iter()
                        .filter(|m| m.kind == crate::entities::mob::MobKind::Spider)
                        .map(|m| format!("{:?} pos {:.1},{:.1},{:.1} dist {:.1}", m.state, m.pos.x, m.pos.y, m.pos.z, m.pos.distance(game.player.pos)))
                        .collect();
                    self.check("mob_attacks", false, format!("паук не атаковал за 8 с; игрок {:.1?}; {}", game.player.pos, info.join("; ")));
                    self.next();
                }
            }
            // Игрок убивает моба мечом — выпадает добыча.
            18 => {
                game.player.health = 20.0;
                game.player.inventory.slots[0] = Some(ItemStack::new(item::tool_id(block::ToolKind::Sword, 5), 1));
                game.player.inventory.selected = 0;
                let items_before = game.entities.items.len();
                let n_before = game.entities.mobs.len();
                for m in &mut game.entities.mobs {
                    m.hurt(100.0, game.player.pos);
                }
                self.expect_item = items_before as u16;
                self.check("mob_damage", game.entities.mobs.iter().all(|m| m.is_dead()), format!("мобов убито: {n_before}"));
                self.next();
            }
            19 => {
                if self.timer > 2.0 {
                    let dead = game.entities.mobs.iter().filter(|m| m.is_dead()).count();
                    let dropped = game.entities.items.len() > self.expect_item as usize;
                    self.check(
                        "mob_death_removed",
                        dead == 0 && dropped,
                        format!("мёртвых осталось: {dead}, предметов на земле: {}", game.entities.items.len()),
                    );
                    self.next();
                }
            }
            // Ночью в темноте появляются враждебные мобы.
            20 => {
                game.day_time = 0.75;
                self.next();
            }
            21 => {
                let hostile = game.entities.mobs.iter().filter(|m| m.kind.hostile()).count();
                if hostile > 0 {
                    let kinds: Vec<String> = game.entities.mobs.iter().map(|m| m.display_name()).collect();
                    self.check("night_spawn", true, format!("за {:.0} с появились: {}", self.timer, kinds.join(", ")));
                    self.next();
                } else if self.timer > 30.0 {
                    self.check("night_spawn", false, "за 30 с ночью никто не появился".into());
                    self.next();
                }
            }
            // --- Уникальные механики в живом мире ---
            // Подземный зал 9×3×9 в каменной оболочке: обвал без крепи.
            22 => {
                game.day_time = 0.25;
                game.entities.mobs.clear();
                let f = Self::feet(game);
                let c = IVec3::new(f.x, (f.y - 30).max(20), f.z);
                self.hall = c;
                game.run_command(&format!("/fill {} {} {} {} {} {} stone", c.x - 7, c.y - 2, c.z - 7, c.x + 7, c.y + 5, c.z + 7));
                game.run_command(&format!("/fill {} {} {} {} {} {} air", c.x - 4, c.y, c.z - 4, c.x + 4, c.y + 2, c.z + 4));
                game.player.mode = GameMode::Survival;
                game.player.health = 20.0;
                game.player.pos = DVec3::new(c.x as f64 + 3.5, c.y as f64, c.z as f64 + 3.5);
                game.player.prev_pos = game.player.pos;
                game.player.vel = DVec3::ZERO;
                self.next();
            }
            23 => {
                if self.timer > 1.5 {
                    let c = self.hall;
                    // Добываем блок свода в центре зала.
                    game.break_block(c + IVec3::new(0, 3, 0), true);
                    self.expect_item = game.mech.caveins.total as u16;
                    self.next();
                }
            }
            24 => {
                if self.timer > 5.0 {
                    let total = game.mech.caveins.total;
                    self.check("cave_in", total > self.expect_item as u32, format!("обвалов: {total}"));
                    self.next();
                }
            }
            // Тот же зал с крепью — свод держится.
            25 => {
                let c = self.hall;
                game.run_command(&format!("/fill {} {} {} {} {} {} stone", c.x - 7, c.y - 2, c.z - 7, c.x + 7, c.y + 5, c.z + 7));
                game.run_command(&format!("/fill {} {} {} {} {} {} air", c.x - 4, c.y, c.z - 4, c.x + 4, c.y + 2, c.z + 4));
                for y in 0..3 {
                    game.world.set(c.x + 1, c.y + y, c.z + 1, block::make(id::SUPPORT, 0));
                }
                game.mech.caveins.pending.clear();
                self.expect_item = game.mech.caveins.total as u16;
                game.break_block(c + IVec3::new(0, 3, 0), true);
                let pending = game.mech.caveins.pending.len();
                self.check("support_holds", pending == 0, format!("угроз обвала при крепи: {pending}"));
                self.next();
            }
            // Резонанс: добыча рядом с резонитом вызывает эхо-импульс.
            26 => {
                let c = self.hall;
                game.mech.pulses.clear();
                game.world.set(c.x - 5, c.y + 1, c.z, block::make(id::RESONITE_ORE, 0));
                game.break_block(IVec3::new(c.x - 5, c.y + 1, c.z + 1), true);
                let markers = game.mech.pulses.first().map(|p| p.markers.len()).unwrap_or(0);
                self.check("resonance_pulse", !game.mech.pulses.is_empty() && markers > 0, format!("импульсов {}, маркеров {markers}", game.mech.pulses.len()));
                self.next();
            }
            // Вода растекается по полу зала.
            27 => {
                let c = self.hall;
                game.world.set(c.x - 3, c.y, c.z - 3, block::make(id::WATER, 0));
                game.mech.water.notify(IVec3::new(c.x - 3, c.y, c.z - 3));
                self.next();
            }
            28 => {
                if self.timer > 3.0 {
                    let c = self.hall;
                    let n = (0..4).filter(|i| game.world.get_id(c.x - 3 + i, c.y, c.z - 3) == id::WATER).count();
                    self.check("water_flow", n >= 3, format!("клеток воды в ряду: {n}/4"));
                    self.next();
                }
            }
            // Полная темнота в закрытом зале — дебафф.
            29 => {
                if self.timer > 6.5 {
                    let d = game.mech.darkness.time;
                    self.check("darkness_debuff", game.mech.darkness.active(), format!("в темноте {d:.1} с"));
                    self.next();
                }
            }
            // Факел с малым запасом топлива гаснет.
            30 => {
                let c = self.hall;
                let p = c + IVec3::new(2, 0, -2);
                game.world.set(p.x, p.y, p.z, block::make(id::TORCH, 255));
                game.mech.torches.place(p, 1.0);
                self.next();
            }
            31 => {
                if self.timer > 2.5 {
                    let c = self.hall;
                    let p = c + IVec3::new(2, 0, -2);
                    let b = game.world.get_id(p.x, p.y, p.z);
                    self.check("torch_burnout", b == id::BURNT_TORCH, format!("блок: {}", block::def(b).name));
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
