//! Игрок: движение (ходьба, бег, присед, прыжок, плавание, лестницы, полёт),
//! здоровье, голод, урон от падения.

use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::inventory::Inventory;
use crate::physics::{self, Aabb};
use crate::world::block::id;
use crate::world::World;

pub const WIDTH: f64 = 0.6;
pub const HEIGHT: f64 = 1.8;
pub const SNEAK_HEIGHT: f64 = 1.5;
pub const EYE: f64 = 1.62;
pub const SNEAK_EYE: f64 = 1.32;

const GRAVITY: f64 = 32.0;
const TERMINAL: f64 = 78.0;
const JUMP_VEL: f64 = 9.0;
const WALK: f64 = 4.3;
const SPRINT: f64 = 5.8;
const SNEAK: f64 = 1.4;
const SWIM: f64 = 2.4;
const FLY: f64 = 11.0;

pub const MAX_HEALTH: f32 = 20.0;
pub const MAX_HUNGER: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    Survival,
    Creative,
}

/// Намерения игрока на этот шаг физики (из ввода).
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveInput {
    /// Направление в плоскости XZ (мировое, длина <= 1).
    pub dir: Vec3,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub pos: DVec3,
    #[serde(skip)]
    pub prev_pos: DVec3,
    pub vel: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    #[serde(skip)]
    pub on_ground: bool,
    #[serde(skip)]
    pub in_water: bool,
    #[serde(skip)]
    pub head_in_water: bool,
    #[serde(skip)]
    pub in_lava: bool,
    #[serde(skip)]
    pub on_ladder: bool,
    #[serde(skip)]
    pub sneaking: bool,
    #[serde(skip)]
    pub sprinting: bool,
    pub flying: bool,
    pub mode: GameMode,
    pub health: f32,
    pub hunger: f32,
    /// Накопленная «усталость»: каждые 4 единицы снимают 1 голод.
    pub exhaustion: f32,
    pub inventory: Inventory,
    pub spawn: DVec3,
    #[serde(skip)]
    fall_start: Option<f64>,
    #[serde(skip)]
    regen_timer: f32,
    #[serde(skip)]
    pub hurt_timer: f32,
    #[serde(skip)]
    pub walk_dist: f64,
    #[serde(skip)]
    pub air: f32,
    /// Сообщения об уроне за шаг (для звука/эффекта).
    #[serde(skip)]
    pub damage_taken: f32,
}

impl Player {
    pub fn new(spawn: DVec3, mode: GameMode) -> Self {
        Self {
            pos: spawn,
            prev_pos: spawn,
            vel: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            in_water: false,
            head_in_water: false,
            in_lava: false,
            on_ladder: false,
            sneaking: false,
            sprinting: false,
            flying: false,
            mode,
            health: MAX_HEALTH,
            hunger: MAX_HUNGER,
            exhaustion: 0.0,
            inventory: Inventory::default(),
            spawn,
            fall_start: None,
            regen_timer: 0.0,
            hurt_timer: 0.0,
            walk_dist: 0.0,
            air: 10.0,
            damage_taken: 0.0,
        }
    }

    pub fn height(&self) -> f64 {
        if self.sneaking && !self.flying {
            SNEAK_HEIGHT
        } else {
            HEIGHT
        }
    }

    pub fn eye_height(&self) -> f64 {
        if self.sneaking && !self.flying {
            SNEAK_EYE
        } else {
            EYE
        }
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::from_feet(self.pos, WIDTH, self.height())
    }

    pub fn eye_pos(&self) -> DVec3 {
        self.pos + DVec3::new(0.0, self.eye_height(), 0.0)
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    pub fn creative(&self) -> bool {
        self.mode == GameMode::Creative
    }

    pub fn damage(&mut self, amount: f32) {
        if self.creative() || amount <= 0.0 || self.is_dead() {
            return;
        }
        self.health = (self.health - amount).max(0.0);
        self.hurt_timer = 0.4;
        self.damage_taken += amount;
    }

    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(MAX_HEALTH);
    }

    pub fn add_exhaustion(&mut self, e: f32) {
        if !self.creative() {
            self.exhaustion += e;
        }
    }

    /// Если игрок застрял в блоке (например, после генерации), выталкивает вверх.
    pub fn unstuck(&mut self, world: &World) {
        for _ in 0..300 {
            if !physics::collides(world, &self.aabb()) {
                return;
            }
            self.pos.y += 1.0;
        }
    }

    fn sample_env(&mut self, world: &World) {
        let b = self.aabb();
        self.in_water = false;
        self.in_lava = false;
        self.on_ladder = false;
        for (x, y, z) in b.cells() {
            let bid = world.get_id(x, y, z);
            match bid {
                id::WATER => self.in_water = true,
                id::LAVA => self.in_lava = true,
                id::LADDER => self.on_ladder = true,
                _ => {}
            }
        }
        let e = self.eye_pos();
        self.head_in_water = world.get_id(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32) == id::WATER;
    }

    fn in_cobweb(&self, world: &World) -> bool {
        self.aabb().cells().any(|(x, y, z)| world.get_id(x, y, z) == id::COBWEB)
    }

    /// Один шаг физики фиксированной длины `dt`.
    pub fn step(&mut self, world: &World, input: MoveInput, dt: f64) {
        self.prev_pos = self.pos;
        // Пока чанк под игроком не загружен — стоим на месте.
        if !world.is_loaded_at(self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
            self.vel = DVec3::ZERO;
            return;
        }
        self.sample_env(world);

        // Присед: нельзя встать, если над головой блок.
        let want_sneak = input.sneak && !self.flying;
        if !want_sneak && self.sneaking {
            let stand = Aabb::from_feet(self.pos, WIDTH, HEIGHT);
            if !physics::collides(world, &stand) {
                self.sneaking = false;
            }
        } else {
            self.sneaking = want_sneak;
        }
        self.sprinting = input.sprint && !self.sneaking && input.dir.length_squared() > 0.01 && (self.hunger > 6.0 || self.creative());

        let liquid = self.in_water || self.in_lava;
        let speed = if self.flying {
            if input.sprint { FLY * 2.0 } else { FLY }
        } else if self.sneaking {
            SNEAK
        } else if liquid {
            SWIM
        } else if self.sprinting {
            SPRINT
        } else {
            WALK
        };
        let speed = if self.in_cobweb(world) { speed * 0.25 } else { speed };

        // Горизонтальное ускорение к целевой скорости.
        let target = input.dir.as_dvec3() * speed;
        let accel = if self.flying {
            10.0
        } else if self.on_ground {
            14.0
        } else if liquid {
            6.0
        } else {
            2.5
        };
        let k = (accel * dt).min(1.0);
        self.vel.x += (target.x - self.vel.x) * k;
        self.vel.z += (target.z - self.vel.z) * k;

        // Вертикаль.
        if self.flying {
            let vy = if input.jump { FLY } else if input.sneak { -FLY } else { 0.0 };
            self.vel.y += (vy - self.vel.y) * (10.0 * dt).min(1.0);
        } else if liquid {
            let g = if self.in_lava { 4.0 } else { 9.0 };
            self.vel.y -= g * dt;
            self.vel.y *= 1.0 - (2.5 * dt).min(0.9);
            if input.jump {
                self.vel.y = self.vel.y.max(3.2);
            }
            // Выпрыгнуть на берег: у кромки воды упёрлись в блок — подскок.
            self.vel.y = self.vel.y.clamp(-4.0, 4.0);
        } else if self.on_ladder {
            self.vel.y = (self.vel.y - GRAVITY * dt).max(-2.5);
            if input.jump || input.dir.length_squared() > 0.01 && self.horizontal_blocked(world) {
                self.vel.y = 2.6;
            } else if self.sneaking {
                self.vel.y = 0.0;
            }
        } else {
            self.vel.y = (self.vel.y - GRAVITY * dt).max(-TERMINAL);
            if input.jump && self.on_ground {
                self.vel.y = JUMP_VEL;
                self.add_exhaustion(if self.sprinting { 0.2 } else { 0.05 });
            }
        }

        // Перемещение с коллизиями.
        let mut delta = self.vel * dt;
        let mut b = self.aabb();

        // Присед у края: не даём сойти с опоры.
        if self.sneaking && self.on_ground && !self.flying {
            let probe = |dx: f64, dz: f64| {
                let moved = b.offset(DVec3::new(dx, -0.6, dz));
                physics::collides(world, &moved)
            };
            let step = 0.05;
            while delta.x != 0.0 && !probe(delta.x, 0.0) {
                delta.x = if delta.x.abs() < step { 0.0 } else { delta.x - step * delta.x.signum() };
            }
            while delta.z != 0.0 && !probe(0.0, delta.z) {
                delta.z = if delta.z.abs() < step { 0.0 } else { delta.z - step * delta.z.signum() };
            }
            while delta.x != 0.0 && delta.z != 0.0 && !probe(delta.x, delta.z) {
                delta.x = if delta.x.abs() < step { 0.0 } else { delta.x - step * delta.x.signum() };
                delta.z = if delta.z.abs() < step { 0.0 } else { delta.z - step * delta.z.signum() };
            }
        }

        let (moved, hit) = physics::move_box(world, &mut b, delta);
        let was_on_ground = self.on_ground;
        self.on_ground = hit[1] && delta.y < 0.0;
        if hit[0] {
            self.vel.x = 0.0;
        }
        if hit[1] {
            self.vel.y = 0.0;
        }
        if hit[2] {
            self.vel.z = 0.0;
        }
        // Выход из воды на берег: в воде упёрлись горизонтально — подталкиваем вверх.
        if liquid && (hit[0] || hit[2]) && input.jump {
            self.vel.y = 4.5;
        }
        self.pos = DVec3::new((b.min.x + b.max.x) * 0.5, b.min.y, (b.min.z + b.max.z) * 0.5);
        let horiz = (moved.x * moved.x + moved.z * moved.z).sqrt();
        if self.on_ground {
            self.walk_dist += horiz;
        }
        if self.sprinting {
            self.add_exhaustion(0.1 * horiz as f32);
        }

        // Урон от падения.
        if self.flying || liquid || self.on_ladder || self.in_cobweb(world) {
            self.fall_start = None;
        } else if !self.on_ground {
            // Учитываем и позицию до шага — иначе теряется часть высоты.
            let y = self.pos.y.max(self.prev_pos.y);
            self.fall_start = Some(self.fall_start.map_or(y, |s| s.max(y)));
        } else if !was_on_ground || self.fall_start.is_some() {
            if let Some(start) = self.fall_start.take() {
                let dist = start - self.pos.y;
                if dist > 3.5 {
                    self.damage((dist - 3.0).floor() as f32);
                }
            }
        }
        if self.on_ground && self.flying && !self.creative() {
            self.flying = false;
        }
    }

    fn horizontal_blocked(&self, world: &World) -> bool {
        let b = self.aabb();
        let probe = b.offset(DVec3::new(self.vel.x.signum() * 0.1, 0.0, self.vel.z.signum() * 0.1));
        physics::collides(world, &probe)
    }

    /// Медленные процессы: голод, регенерация, утопление, лава.
    pub fn tick_status(&mut self, world: &World, dt: f32) {
        self.hurt_timer = (self.hurt_timer - dt).max(0.0);
        if self.creative() {
            self.health = MAX_HEALTH;
            self.hunger = MAX_HUNGER;
            self.air = 10.0;
            return;
        }
        // Пассивная усталость.
        self.exhaustion += dt * 0.01;
        while self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            self.hunger = (self.hunger - 1.0).max(0.0);
        }
        self.regen_timer += dt;
        if self.regen_timer >= 4.0 {
            self.regen_timer = 0.0;
            if self.hunger >= 18.0 && self.health < MAX_HEALTH {
                self.heal(1.0);
                self.exhaustion += 1.5;
            } else if self.hunger <= 0.0 && self.health > 1.0 {
                self.damage(1.0);
            }
        }
        // Дыхание под водой.
        if self.head_in_water {
            self.air -= dt;
            if self.air <= 0.0 {
                self.air = 1.0;
                self.damage(2.0);
            }
        } else {
            self.air = (self.air + dt * 3.0).min(10.0);
        }
        if self.in_lava {
            self.damage(dt * 8.0);
        }
        // Кактус.
        let b = self.aabb();
        let grown = Aabb { min: b.min - DVec3::splat(0.05), max: b.max + DVec3::splat(0.05) };
        if grown.cells().any(|(x, y, z)| world.get_id(x, y, z) == id::CACTUS) && self.hurt_timer <= 0.0 {
            self.damage(1.0);
        }
    }

    pub fn eat(&mut self, hunger: f32) {
        self.hunger = (self.hunger + hunger).min(MAX_HUNGER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::{id, make};
    use crate::world::chunk::{Chunk, ChunkData, ChunkPos};

    /// Мир 3×3 чанка: каменный пол на y=10, опционально бассейн с водой.
    fn flat_world(pool: bool) -> World {
        let mut w = World::new(1, None, false);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut d = ChunkData::empty();
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 0..=10 {
                            d.set(x, y, z, make(id::STONE, 0));
                        }
                    }
                }
                if pool && cx == 0 && cz == 0 {
                    for z in 4..12 {
                        for x in 4..12 {
                            for y in 6..=10 {
                                d.set(x, y, z, make(id::WATER, 0));
                            }
                        }
                    }
                }
                let p = ChunkPos::new(cx, cz);
                w.chunks.insert(p, Chunk::new(p, d));
            }
        }
        w
    }

    fn run(p: &mut Player, w: &World, input: MoveInput, secs: f64) {
        let steps = (secs * 60.0) as usize;
        for _ in 0..steps {
            p.step(w, input, 1.0 / 60.0);
        }
    }

    #[test]
    fn lands_on_floor() {
        let w = flat_world(false);
        let mut p = Player::new(DVec3::new(-8.5, 20.0, -8.5), GameMode::Survival);
        run(&mut p, &w, MoveInput::default(), 3.0);
        assert!(p.on_ground);
        assert!((p.pos.y - 11.0).abs() < 1e-6, "y = {}", p.pos.y);
    }

    #[test]
    fn sneak_does_not_fall_off_edge() {
        let mut w = flat_world(false);
        // Яма шириной в несколько блоков перед игроком.
        for x in -12..-6 {
            for z in -12..-4 {
                w.set(x, 10, z, 0);
                w.set(x, 9, z, 0);
            }
        }
        let mut p = Player::new(DVec3::new(-4.5, 11.0, -8.5), GameMode::Survival);
        run(&mut p, &w, MoveInput::default(), 0.5);
        let input = MoveInput { dir: Vec3::new(-1.0, 0.0, 0.0), sneak: true, ..Default::default() };
        run(&mut p, &w, input, 4.0);
        assert!(p.pos.y > 10.99, "упал в яму: y = {}", p.pos.y);
        assert!(p.pos.x < -5.5, "не дошёл до края: x = {}", p.pos.x);
        // Без приседа — падает.
        let input = MoveInput { dir: Vec3::new(-1.0, 0.0, 0.0), ..Default::default() };
        run(&mut p, &w, input, 2.0);
        assert!(p.pos.y < 10.5, "должен был упасть: y = {}", p.pos.y);
    }

    #[test]
    fn swimming_floats_up_with_jump() {
        let w = flat_world(true);
        let mut p = Player::new(DVec3::new(8.0, 7.0, 8.0), GameMode::Survival);
        run(&mut p, &w, MoveInput::default(), 1.0);
        assert!(p.in_water);
        let y0 = p.pos.y;
        run(&mut p, &w, MoveInput { jump: true, ..Default::default() }, 1.0);
        assert!(p.pos.y > y0 + 1.0, "не всплыл: {} -> {}", y0, p.pos.y);
        // Без прыжка в воде тонет медленно (скорость ограничена).
        let y1 = p.pos.y;
        run(&mut p, &w, MoveInput::default(), 0.5);
        assert!(y1 - p.pos.y < 2.5);
    }

    #[test]
    fn fall_damage_from_height() {
        let w = flat_world(false);
        let mut p = Player::new(DVec3::new(-8.5, 21.0, -8.5), GameMode::Survival);
        run(&mut p, &w, MoveInput::default(), 3.0);
        assert_eq!(p.health, MAX_HEALTH - 7.0);
    }
}
