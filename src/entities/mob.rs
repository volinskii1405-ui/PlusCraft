//! Мобы: виды, ИИ (блуждание, преследование по A*, атака, бегство),
//! физика, модели из кубоидов с анимацией ходьбы.

use glam::{DVec3, IVec3, Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::assets::textures::Tex;
use crate::inventory::ItemStack;
use crate::item::{self, id as it};
use crate::physics::{self, Aabb};
use crate::renderer::vertex::EntityVertex;
use crate::world::block::id;
use crate::world::noise::Rng;
use crate::world::World;

use super::mesh::{self, pack_light, FaceTex, UNLIT};
use super::path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobKind {
    /// Луговик — мирный, пасётся на равнинах; шерсть и баранина.
    Grazer,
    /// Кабан — мирный, леса и тайга; мясо и кожа.
    Boar,
    /// Ползун — враждебный, бродит в темноте и ночью, горит на солнце.
    Crawler,
    /// Пещерный паук — враждебный, быстрый, только под землёй.
    Spider,
    /// Мракоед — появляется лишь в абсолютной темноте и гасит факелы.
    Gloom,
}

impl MobKind {
    pub fn name(self) -> &'static str {
        match self {
            MobKind::Grazer => "Луговик",
            MobKind::Boar => "Кабан",
            MobKind::Crawler => "Ползун",
            MobKind::Spider => "Пещерный паук",
            MobKind::Gloom => "Мракоед",
        }
    }
    pub fn hostile(self) -> bool {
        matches!(self, MobKind::Crawler | MobKind::Spider | MobKind::Gloom)
    }
    /// (ширина, высота) хитбокса.
    pub fn size(self) -> (f64, f64) {
        match self {
            MobKind::Grazer => (0.9, 1.3),
            MobKind::Boar => (0.9, 0.9),
            MobKind::Crawler => (0.6, 1.9),
            MobKind::Spider => (1.2, 0.8),
            MobKind::Gloom => (0.7, 1.6),
        }
    }
    pub fn max_health(self) -> f32 {
        match self {
            MobKind::Grazer => 8.0,
            MobKind::Boar => 10.0,
            MobKind::Crawler => 20.0,
            MobKind::Spider => 14.0,
            MobKind::Gloom => 16.0,
        }
    }
    pub fn speed(self) -> f64 {
        match self {
            MobKind::Grazer => 1.6,
            MobKind::Boar => 2.0,
            MobKind::Crawler => 3.0,
            MobKind::Spider => 4.2,
            MobKind::Gloom => 2.6,
        }
    }
    pub fn damage(self) -> f32 {
        match self {
            MobKind::Crawler => 3.0,
            MobKind::Spider => 2.0,
            MobKind::Gloom => 4.0,
            _ => 0.0,
        }
    }
    fn path_height(self) -> i32 {
        match self {
            MobKind::Boar | MobKind::Spider => 1,
            _ => 2,
        }
    }
    pub fn drops(self, rng: &mut Rng) -> Vec<ItemStack> {
        let mut v = Vec::new();
        let mut add = |item: item::ItemId, lo: i32, hi: i32, rng: &mut Rng| {
            let n = rng.range(lo, hi + 1);
            if n > 0 {
                v.push(ItemStack::new(item, n as u8));
            }
        };
        match self {
            MobKind::Grazer => {
                add(it::RAW_MUTTON, 1, 2, rng);
                add(id::WOOL as item::ItemId, 0, 2, rng);
            }
            MobKind::Boar => {
                add(it::RAW_MEAT, 1, 3, rng);
                add(it::LEATHER, 0, 2, rng);
            }
            MobKind::Crawler => {
                add(it::BONE, 0, 2, rng);
                if rng.chance(0.08) {
                    add(it::RAW_IRON, 1, 1, rng);
                }
            }
            MobKind::Spider => add(it::STRING, 0, 2, rng),
            MobKind::Gloom => {
                add(it::COAL, 0, 2, rng);
                if rng.chance(0.35) {
                    add(it::RESONITE_SHARD, 1, 1, rng);
                }
            }
        }
        v
    }
}

/// Мутация моба по глубине (уникальная механика): чем глубже появился моб,
/// тем он сильнее и необычнее.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Mutation {
    /// 0 — обычный, 1..3 — уровни мутации.
    pub level: u8,
}

impl Mutation {
    pub fn for_depth(y: f64, rng: &mut Rng) -> Self {
        let level = if y < 12.0 {
            3
        } else if y < 26.0 {
            2
        } else if y < 42.0 {
            1
        } else {
            0
        };
        // Небольшой разброс: иногда на уровень меньше.
        let level = if level > 0 && rng.chance(0.25) { level - 1 } else { level };
        Self { level }
    }
    pub fn health_mul(self) -> f32 {
        1.0 + 0.5 * self.level as f32
    }
    pub fn damage_mul(self) -> f32 {
        1.0 + 0.35 * self.level as f32
    }
    pub fn speed_mul(self) -> f64 {
        1.0 + 0.1 * self.level as f64
    }
    pub fn scale(self) -> f32 {
        1.0 + 0.12 * self.level as f32
    }
    /// Мутанты 3-го уровня регенерируют.
    pub fn regen(self) -> f32 {
        if self.level >= 3 {
            0.5
        } else {
            0.0
        }
    }
    pub fn title(self) -> &'static str {
        match self.level {
            0 => "",
            1 => "Мутировавший ",
            2 => "Искажённый ",
            _ => "Глубинный ",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum AiState {
    #[default]
    Idle,
    Wander,
    Chase,
    Flee,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mob {
    pub kind: MobKind,
    pub pos: DVec3,
    #[serde(skip)]
    pub prev_pos: DVec3,
    pub vel: DVec3,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub mutation: Mutation,
    #[serde(skip)]
    pub on_ground: bool,
    #[serde(skip)]
    pub state: AiState,
    #[serde(skip)]
    path: Vec<IVec3>,
    #[serde(skip)]
    path_timer: f32,
    #[serde(skip)]
    think_timer: f32,
    #[serde(skip)]
    wander_target: Option<DVec3>,
    #[serde(skip)]
    pub attack_cooldown: f32,
    #[serde(skip)]
    pub hurt_timer: f32,
    /// Горит (на солнце) — оранжевый оттенок.
    #[serde(skip)]
    pub burn_timer: f32,
    #[serde(skip)]
    flee_timer: f32,
    #[serde(skip)]
    pub walk_phase: f32,
    pub age: f32,
    #[serde(skip)]
    pub dead_timer: f32,
    /// Притянут резонансным импульсом: идёт к точке.
    #[serde(skip)]
    pub lure: Option<DVec3>,
    #[serde(skip)]
    lure_timer: f32,
}

/// Прямая видимость между точками (на высоте ~0.6 блока над ногами).
fn line_of_sight(world: &World, a: DVec3, b: DVec3) -> bool {
    let a = a + DVec3::new(0.0, 0.6, 0.0);
    let b = b + DVec3::new(0.0, 0.6, 0.0);
    let d = b - a;
    let n = (d.length() / 0.25).ceil().max(1.0) as i32;
    (1..n).all(|i| {
        let p = a + d * (i as f64 / n as f64);
        !world.is_solid(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
    })
}

/// Событие от моба для игры (урон игроку, погашенный факел).
pub enum MobEvent {
    HitPlayer { damage: f32, from: DVec3 },
    SnuffTorch(IVec3),
}

impl Mob {
    pub fn new(kind: MobKind, pos: DVec3, mutation: Mutation, rng: &mut Rng) -> Self {
        let max_health = kind.max_health() * mutation.health_mul();
        Self {
            kind,
            pos,
            prev_pos: pos,
            vel: DVec3::ZERO,
            yaw: rng.f32() * std::f32::consts::TAU,
            health: max_health,
            max_health,
            mutation,
            on_ground: false,
            state: AiState::Idle,
            path: Vec::new(),
            path_timer: 0.0,
            think_timer: rng.f32(),
            wander_target: None,
            attack_cooldown: 0.0,
            hurt_timer: 0.0,
            burn_timer: 0.0,
            flee_timer: 0.0,
            walk_phase: 0.0,
            age: 0.0,
            dead_timer: 0.0,
            lure: None,
            lure_timer: 0.0,
        }
    }

    pub fn display_name(&self) -> String {
        format!("{}{}", self.mutation.title(), self.kind.name())
    }

    pub fn aabb(&self) -> Aabb {
        let (w, h) = self.kind.size();
        let s = self.mutation.scale() as f64;
        Aabb::from_feet(self.pos, w * s.min(1.3), h * s.min(1.3))
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    /// Получение урона с отбрасыванием от точки `from`.
    pub fn hurt(&mut self, amount: f32, from: DVec3) {
        if self.is_dead() {
            return;
        }
        self.health -= amount;
        self.hurt_timer = 0.35;
        let mut d = self.pos - from;
        d.y = 0.0;
        let d = d.normalize_or_zero();
        self.vel += d * 7.0 + DVec3::new(0.0, 5.0, 0.0);
        if !self.kind.hostile() {
            self.state = AiState::Flee;
            self.flee_timer = 5.0;
        } else {
            self.state = AiState::Chase;
        }
    }

    fn feet_cell(&self) -> IVec3 {
        IVec3::new(self.pos.x.floor() as i32, (self.pos.y + 0.01).floor() as i32, self.pos.z.floor() as i32)
    }

    /// ИИ: решения о цели и пути (вызывается ~10 раз в секунду).
    pub fn think(&mut self, world: &World, player: DVec3, player_alive: bool, dt: f32, rng: &mut Rng) {
        self.path_timer -= dt;
        let to_player = player - self.pos;
        let dist = to_player.length();
        if self.lure_timer > 0.0 {
            self.lure_timer -= dt;
            if self.lure_timer <= 0.0 {
                self.lure = None;
            }
        }

        if self.kind.hostile() {
            let sense = 18.0 + 4.0 * self.mutation.level as f64;
            if player_alive && dist < sense {
                self.state = AiState::Chase;
            } else if self.state == AiState::Chase {
                self.state = AiState::Wander;
                self.path.clear();
            }
        } else if self.state == AiState::Flee {
            self.flee_timer -= dt;
            if self.flee_timer <= 0.0 {
                self.state = AiState::Idle;
            }
        }

        match self.state {
            AiState::Chase => {
                if self.path_timer <= 0.0 {
                    self.path_timer = 0.8 + rng.f32() * 0.4;
                    let goal = IVec3::new(player.x.floor() as i32, (player.y + 0.01).floor() as i32, player.z.floor() as i32);
                    self.path = path::find_path(world, self.feet_cell(), goal, self.kind.path_height(), 500).unwrap_or_default();
                }
            }
            AiState::Flee => {
                if self.path_timer <= 0.0 {
                    self.path_timer = 1.0;
                    let away = (self.pos - player).normalize_or_zero() * 10.0;
                    let goal = (self.pos + DVec3::new(away.x, 0.0, away.z)).floor().as_ivec3();
                    self.path = path::find_path(world, self.feet_cell(), goal, self.kind.path_height(), 300).unwrap_or_default();
                }
            }
            AiState::Idle | AiState::Wander => {
                self.think_timer -= dt;
                if let Some(target) = self.lure {
                    if self.path_timer <= 0.0 {
                        self.path_timer = 1.5;
                        self.state = AiState::Wander;
                        self.path = path::find_path(world, self.feet_cell(), target.floor().as_ivec3(), self.kind.path_height(), 500)
                            .unwrap_or_default();
                    }
                } else if self.think_timer <= 0.0 {
                    self.think_timer = 3.0 + rng.f32() * 5.0;
                    if rng.chance(0.6) {
                        self.state = AiState::Wander;
                        let goal = self.feet_cell() + IVec3::new(rng.range(-8, 9), 0, rng.range(-8, 9));
                        self.path = path::find_path(world, self.feet_cell(), goal, self.kind.path_height(), 250).unwrap_or_default();
                        self.wander_target = None;
                    } else {
                        self.state = AiState::Idle;
                        self.path.clear();
                    }
                }
            }
        }
    }

    /// Притягивает моба к точке (резонансный импульс).
    pub fn lure_to(&mut self, p: DVec3, time: f32) {
        self.lure = Some(p);
        self.lure_timer = time;
        self.path_timer = 0.0;
    }

    /// Шаг физики и следования пути.
    pub fn step(&mut self, world: &World, player: DVec3, player_alive: bool, dt: f64, events: &mut Vec<MobEvent>) {
        self.prev_pos = self.pos;
        self.age += dt as f32;
        self.attack_cooldown = (self.attack_cooldown - dt as f32).max(0.0);
        self.hurt_timer = (self.hurt_timer - dt as f32).max(0.0);
        self.burn_timer = (self.burn_timer - dt as f32).max(0.0);
        if self.is_dead() {
            self.dead_timer += dt as f32;
            self.vel.x *= 0.8;
            self.vel.z *= 0.8;
        }
        if !world.is_loaded_at(self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
            return;
        }
        if self.health < self.max_health && !self.is_dead() {
            self.health = (self.health + self.mutation.regen() * dt as f32).min(self.max_health);
        }

        // Желаемое направление: к следующей точке пути или прямо к игроку вблизи.
        let mut want = DVec3::ZERO;
        let speed = self.kind.speed()
            * self.mutation.speed_mul()
            * match self.state {
                AiState::Flee => 2.2,
                AiState::Chase => 1.0,
                _ => 0.6,
            };
        let to_player = player - self.pos;
        let close = to_player.length() < 2.2 && (to_player.y.abs() < 1.5) && line_of_sight(world, self.pos, player);
        if !self.is_dead() {
            if self.state == AiState::Chase && close {
                want = DVec3::new(to_player.x, 0.0, to_player.z).normalize_or_zero();
            } else if let Some(&next) = self.path.first() {
                let target = DVec3::new(next.x as f64 + 0.5, next.y as f64, next.z as f64 + 0.5);
                let mut d = target - self.pos;
                d.y = 0.0;
                if d.length() < 0.35 {
                    self.path.remove(0);
                } else {
                    want = d.normalize();
                }
            } else if self.state == AiState::Wander {
                self.state = AiState::Idle;
            }
        }

        let in_water = world.get_id(self.pos.x.floor() as i32, (self.pos.y + 0.3).floor() as i32, self.pos.z.floor() as i32) == id::WATER;
        let k = if self.on_ground { 10.0 } else { 2.0 } * dt;
        self.vel.x += (want.x * speed - self.vel.x) * k.min(1.0);
        self.vel.z += (want.z * speed - self.vel.z) * k.min(1.0);

        let floats = self.kind == MobKind::Gloom;
        if floats {
            // Мракоед парит над землёй.
            let ground = (0..4).find(|d| world.is_solid(self.pos.x.floor() as i32, self.pos.y.floor() as i32 - d - 1, self.pos.z.floor() as i32));
            let target_vy = match ground {
                Some(d) if d < 1 => 1.5,
                Some(_) => 0.0,
                None => -1.5,
            };
            self.vel.y += (target_vy + (self.age * 2.0).sin() as f64 * 0.3 - self.vel.y) * (4.0 * dt).min(1.0);
        } else if in_water {
            self.vel.y = (self.vel.y + 12.0 * dt).min(2.0);
        } else {
            self.vel.y = (self.vel.y - 30.0 * dt).max(-50.0);
        }

        let mut b = self.aabb();
        let (moved, hit) = physics::move_box(world, &mut b, self.vel * dt);
        self.on_ground = hit[1] && self.vel.y < 0.0;
        if hit[1] {
            self.vel.y = 0.0;
        }
        // Упёрлись в стену — прыжок (если сверху свободно).
        if (hit[0] || hit[2]) && self.on_ground && want.length_squared() > 0.0 {
            self.vel.y = 8.2;
        }
        if hit[0] {
            self.vel.x = 0.0;
        }
        if hit[2] {
            self.vel.z = 0.0;
        }
        self.pos = DVec3::new((b.min.x + b.max.x) * 0.5, b.min.y, (b.min.z + b.max.z) * 0.5);

        let horiz = (moved.x * moved.x + moved.z * moved.z).sqrt();
        self.walk_phase += (horiz * 6.0) as f32;
        if want.length_squared() > 0.0 {
            let target_yaw = (-want.x).atan2(-want.z) as f32;
            let mut d = target_yaw - self.yaw;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            self.yaw += d * (10.0 * dt as f32).min(1.0);
        }

        // Атака.
        if self.kind.hostile() && !self.is_dead() && player_alive && self.attack_cooldown <= 0.0 {
            let pb = Aabb::from_feet(player, 0.6, 1.8);
            let reach = Aabb { min: b.min - DVec3::splat(0.35), max: b.max + DVec3::splat(0.35) };
            if reach.intersects(&pb) {
                self.attack_cooldown = if self.kind == MobKind::Spider { 0.8 } else { 1.0 };
                events.push(MobEvent::HitPlayer { damage: self.kind.damage() * self.mutation.damage_mul(), from: self.pos });
            }
        }
        // Мракоед гасит факелы рядом.
        if self.kind == MobKind::Gloom && !self.is_dead() && (self.age * 10.0) as i64 % 20 == 0 {
            let c = self.feet_cell();
            for dz in -3..=3 {
                for dy in -2..=3 {
                    for dx in -3..=3 {
                        let p = c + IVec3::new(dx, dy, dz);
                        if world.get_id(p.x, p.y, p.z) == id::TORCH {
                            events.push(MobEvent::SnuffTorch(p));
                        }
                    }
                }
            }
        }
    }

    /// Ползуны горят на прямом солнце.
    pub fn burns_in_daylight(&self) -> bool {
        self.kind == MobKind::Crawler
    }

    /// Пересечение луча с хитбоксом (для атаки игрока).
    pub fn ray_hit(&self, origin: DVec3, dir: Vec3, max: f64) -> Option<f64> {
        let b = self.aabb();
        let d = dir.as_dvec3();
        let mut tmin: f64 = 0.0;
        let mut tmax: f64 = max;
        for a in 0..3 {
            if d[a].abs() < 1e-9 {
                if origin[a] < b.min[a] || origin[a] > b.max[a] {
                    return None;
                }
            } else {
                let inv = 1.0 / d[a];
                let mut t0 = (b.min[a] - origin[a]) * inv;
                let mut t1 = (b.max[a] - origin[a]) * inv;
                if t0 > t1 {
                    std::mem::swap(&mut t0, &mut t1);
                }
                tmin = tmin.max(t0);
                tmax = tmax.min(t1);
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some(tmin)
    }

    /// Геометрия модели.
    pub fn build_mesh(&self, world: &World, cam: DVec3, alpha: f64, out: &mut Vec<EntityVertex>) {
        let pos = self.prev_pos.lerp(self.pos, alpha);
        let rel = (pos - cam).as_vec3();
        let (sky, blk) = world.light(pos.x.floor() as i32, (pos.y + 0.5).floor() as i32, pos.z.floor() as i32);
        let light = pack_light(sky, blk);
        let s = self.mutation.scale();
        let mut base = Mat4::from_translation(rel) * Mat4::from_rotation_y(self.yaw) * Mat4::from_scale(Vec3::splat(s));
        if self.is_dead() {
            // Заваливается на бок.
            base *= Mat4::from_rotation_z((self.dead_timer * 4.0).min(1.5));
        }
        let hurt = self.hurt_timer > 0.0 || self.is_dead();
        let burning = self.burn_timer > 0.0 && (self.age * 8.0).sin() > -0.3;
        let tint = |c: [u8; 3]| -> [u8; 4] {
            if hurt {
                [255, (c[1] as f32 * 0.4) as u8, (c[2] as f32 * 0.4) as u8, 255]
            } else if burning {
                [255, (c[1] as f32 * 0.6 + 90.0) as u8, (c[2] as f32 * 0.3) as u8, 255]
            } else {
                [c[0], c[1], c[2], 255]
            }
        };
        // Мутанты получают фиолетовый оттенок и светящиеся глаза.
        let mutate = |c: [u8; 3]| -> [u8; 3] {
            let l = self.mutation.level as f32 * 0.18;
            [
                (c[0] as f32 * (1.0 - l) + 150.0 * l) as u8,
                (c[1] as f32 * (1.0 - l) + 60.0 * l) as u8,
                (c[2] as f32 * (1.0 - l) + 200.0 * l) as u8,
            ]
        };
        let fur = FaceTex::full(Tex::MobFur as u16);
        let skin = FaceTex::full(Tex::MobSkin as u16);
        let eye = FaceTex::full(Tex::MobEyes as u16);
        let swing = self.walk_phase.sin() * 0.7;

        let mut part = |m: Mat4, min: Vec3, max: Vec3, tex: FaceTex, color: [u8; 4], lit: u16| {
            mesh::push_box(out, &(base * m), min, max, &[tex; 6], color, lit);
        };
        let leg = |x: f32, top: f32, z: f32, phase: f32| -> Mat4 {
            Mat4::from_translation(Vec3::new(x, top, z)) * Mat4::from_rotation_x(swing * phase)
        };
        let eye_light = if self.mutation.level >= 2 || self.kind == MobKind::Gloom || self.kind == MobKind::Spider { UNLIT } else { light };

        match self.kind {
            MobKind::Grazer => {
                let wool = tint(mutate([235, 232, 225]));
                let face = tint(mutate([70, 60, 55]));
                part(Mat4::IDENTITY, Vec3::new(-0.45, 0.55, -0.6), Vec3::new(0.45, 1.15, 0.6), fur, wool, light);
                part(Mat4::IDENTITY, Vec3::new(-0.22, 0.8, -0.95), Vec3::new(0.22, 1.25, -0.55), skin, face, light);
                part(Mat4::IDENTITY, Vec3::new(-0.16, 1.05, -0.96), Vec3::new(-0.06, 1.12, -0.94), eye, [20, 20, 20, 255], eye_light);
                part(Mat4::IDENTITY, Vec3::new(0.06, 1.05, -0.96), Vec3::new(0.16, 1.12, -0.94), eye, [20, 20, 20, 255], eye_light);
                for (x, z, ph) in [(-0.28, -0.4, 1.0), (0.28, -0.4, -1.0), (-0.28, 0.4, -1.0), (0.28, 0.4, 1.0)] {
                    part(leg(x, 0.6, z, ph), Vec3::new(-0.1, -0.6, -0.1), Vec3::new(0.1, 0.0, 0.1), skin, face, light);
                }
            }
            MobKind::Boar => {
                let hide = tint(mutate([120, 80, 55]));
                let dark = tint(mutate([80, 50, 35]));
                part(Mat4::IDENTITY, Vec3::new(-0.4, 0.35, -0.6), Vec3::new(0.4, 0.85, 0.6), fur, hide, light);
                part(Mat4::IDENTITY, Vec3::new(-0.28, 0.4, -0.95), Vec3::new(0.28, 0.85, -0.6), skin, hide, light);
                part(Mat4::IDENTITY, Vec3::new(-0.14, 0.42, -1.08), Vec3::new(0.14, 0.62, -0.95), skin, [230, 160, 150, 255], light);
                part(Mat4::IDENTITY, Vec3::new(-0.22, 0.45, -1.02), Vec3::new(-0.17, 0.6, -0.97), skin, [240, 240, 230, 255], light);
                part(Mat4::IDENTITY, Vec3::new(0.17, 0.45, -1.02), Vec3::new(0.22, 0.6, -0.97), skin, [240, 240, 230, 255], light);
                part(Mat4::IDENTITY, Vec3::new(-0.2, 0.7, -0.96), Vec3::new(-0.1, 0.77, -0.94), eye, [20, 20, 20, 255], eye_light);
                part(Mat4::IDENTITY, Vec3::new(0.1, 0.7, -0.96), Vec3::new(0.2, 0.77, -0.94), eye, [20, 20, 20, 255], eye_light);
                for (x, z, ph) in [(-0.25, -0.4, 1.0), (0.25, -0.4, -1.0), (-0.25, 0.4, -1.0), (0.25, 0.4, 1.0)] {
                    part(leg(x, 0.4, z, ph), Vec3::new(-0.09, -0.4, -0.09), Vec3::new(0.09, 0.0, 0.09), skin, dark, light);
                }
            }
            MobKind::Crawler => {
                let body = tint(mutate([90, 120, 85]));
                let cloth = tint(mutate([60, 70, 95]));
                part(Mat4::IDENTITY, Vec3::new(-0.25, 0.75, -0.13), Vec3::new(0.25, 1.45, 0.13), skin, cloth, light);
                part(Mat4::IDENTITY, Vec3::new(-0.25, 1.45, -0.25), Vec3::new(0.25, 1.95, 0.25), skin, body, light);
                part(Mat4::IDENTITY, Vec3::new(-0.17, 1.68, -0.26), Vec3::new(-0.06, 1.75, -0.25), eye, [255, 60, 40, 255], UNLIT);
                part(Mat4::IDENTITY, Vec3::new(0.06, 1.68, -0.26), Vec3::new(0.17, 1.75, -0.25), eye, [255, 60, 40, 255], UNLIT);
                // Руки вытянуты вперёд.
                for x in [-0.37f32, 0.37] {
                    let m = Mat4::from_translation(Vec3::new(x, 1.38, 0.0)) * Mat4::from_rotation_x(-1.45 + swing * 0.2);
                    part(m, Vec3::new(-0.11, -0.7, -0.11), Vec3::new(0.11, 0.0, 0.11), skin, body, light);
                }
                for (x, ph) in [(-0.12f32, 1.0), (0.12, -1.0)] {
                    part(leg(x, 0.75, 0.0, ph), Vec3::new(-0.12, -0.75, -0.12), Vec3::new(0.12, 0.0, 0.12), skin, cloth, light);
                }
            }
            MobKind::Spider => {
                let body = tint(mutate([45, 40, 42]));
                part(Mat4::IDENTITY, Vec3::new(-0.3, 0.25, -0.2), Vec3::new(0.3, 0.6, 0.65), fur, body, light);
                part(Mat4::IDENTITY, Vec3::new(-0.22, 0.28, -0.55), Vec3::new(0.22, 0.58, -0.2), fur, body, light);
                for x in [-0.15f32, -0.05, 0.05, 0.15] {
                    part(Mat4::IDENTITY, Vec3::new(x - 0.03, 0.45, -0.56), Vec3::new(x + 0.03, 0.51, -0.55), eye, [255, 30, 30, 255], UNLIT);
                }
                for i in 0..4 {
                    let z = -0.1 + i as f32 * 0.2;
                    let ph = if i % 2 == 0 { 1.0 } else { -1.0 };
                    for side in [-1.0f32, 1.0] {
                        let m = Mat4::from_translation(Vec3::new(0.3 * side, 0.45, z))
                            * Mat4::from_rotation_y(swing * 0.4 * ph)
                            * Mat4::from_rotation_z(side * 0.6);
                        let (a, b) = if side > 0.0 { (0.0, 0.8) } else { (-0.8, 0.0) };
                        part(m, Vec3::new(a, -0.04, -0.04), Vec3::new(b, 0.04, 0.04), skin, body, light);
                    }
                }
            }
            MobKind::Gloom => {
                let body = tint([30, 20, 45]);
                let glow = [170, 90, 255, 255];
                let bob = (self.age * 3.0).sin() * 0.05;
                let m = Mat4::from_translation(Vec3::new(0.0, bob, 0.0));
                part(m, Vec3::new(-0.3, 0.3, -0.3), Vec3::new(0.3, 1.3, 0.3), fur, body, light);
                part(m, Vec3::new(-0.22, 1.3, -0.22), Vec3::new(0.22, 1.7, 0.22), fur, body, light);
                part(m, Vec3::new(-0.15, 1.45, -0.23), Vec3::new(-0.04, 1.55, -0.22), eye, glow, UNLIT);
                part(m, Vec3::new(0.04, 1.45, -0.23), Vec3::new(0.15, 1.55, -0.22), eye, glow, UNLIT);
                // «Хвост» из тающих кубиков.
                for i in 0..3 {
                    let y = 0.25 - i as f32 * 0.12;
                    let w = 0.2 - i as f32 * 0.05;
                    part(m, Vec3::new(-w, y - 0.1, -w), Vec3::new(w, y, w), fur, body, light);
                }
            }
        }
    }
}
