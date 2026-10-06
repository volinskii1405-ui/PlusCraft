//! Игровое состояние: мир, игрок, сущности; фиксированный шаг физики,
//! добыча и установка блоков, подготовка кадра.

use glam::{DVec3, IVec3, Mat4, Quat, Vec3};
use winit::keyboard::KeyCode;

use crate::camera::Camera;
use crate::entities::mesh::{self, pack_light};
use crate::entities::{self, Entities};
use crate::input::InputState;
use crate::inventory::{ItemStack, HOTBAR};
use crate::item::{self, ItemKind};
use crate::physics::{self, Aabb, RayHit};
use crate::player::{self, GameMode, MoveInput, Player};
use crate::renderer::vertex::EntityVertex;
use crate::renderer::{Globals, Renderer};
use crate::settings::{Action, Settings};
use crate::ui::draw::{UiBatch, WHITE};
use crate::ui::hud;
use crate::world::biome;
use crate::world::block::{self, block_id, block_meta, def, id, Drop, Shape, ToolKind};
use crate::world::chunk::ChunkPos;
use crate::world::gen::SEA_LEVEL;
use crate::world::noise::Rng;
use crate::world::World;

/// Шаг физики, с.
pub const PHYSICS_DT: f64 = 1.0 / 60.0;
const REACH_SURVIVAL: f32 = 4.8;
const REACH_CREATIVE: f32 = 6.0;

struct Mining {
    pos: IVec3,
    progress: f32,
}

/// Геометрия сущностей кадра.
#[derive(Default)]
pub struct FrameGeometry {
    pub entities: Vec<EntityVertex>,
    pub translucent: Vec<EntityVertex>,
    pub lines: Vec<EntityVertex>,
    pub overlay: Vec<EntityVertex>,
}

pub struct Game {
    pub world: World,
    pub player: Player,
    pub entities: Entities,
    pub camera: Camera,
    pub time: f64,
    pub show_debug: bool,
    accumulator: f64,
    mining: Option<Mining>,
    use_cooldown: f32,
    attack_cooldown: f32,
    pub target: Option<RayHit>,
    last_jump_press: f64,
    eye_smooth: f64,
    hand_swing: f32,
    rng: Rng,
    /// Всплывающие сообщения (текст, оставшееся время).
    pub messages: Vec<(String, f32)>,
    fps: f32,
    fps_acc: f32,
    fps_frames: u32,
    frame_ms: f32,
}

/// Ищет точку суши рядом с началом координат.
pub fn find_spawn(world: &World) -> DVec3 {
    for r in 0..64 {
        for i in -r..=r {
            for (x, z) in [(i * 8, -r * 8), (i * 8, r * 8), (-r * 8, i * 8), (r * 8, i * 8)] {
                let c = world.gen.column(x, z);
                if c.height > SEA_LEVEL + 1 && c.height < 110 && !world.gen.caves.is_cave(x, c.height, z, c.height) {
                    return DVec3::new(x as f64 + 0.5, c.height as f64 + 1.0, z as f64 + 0.5);
                }
            }
        }
    }
    DVec3::new(0.5, 100.0, 0.5)
}

pub fn srgb(c: [f32; 3]) -> [f32; 4] {
    [c[0].powf(2.2), c[1].powf(2.2), c[2].powf(2.2), 1.0]
}

/// Скорость инструмента по тиру (0 — рука).
fn tool_speed(tier: u8) -> f32 {
    [1.0, 2.0, 4.0, 5.0, 6.0, 8.0][tier.min(5) as usize]
}

/// Время разрушения блока и можно ли его добыть данным предметом.
pub fn break_time(block: u16, held: Option<ItemStack>) -> (f32, bool) {
    let d = def(block_id(block));
    if d.hardness < 0.0 {
        return (f32::INFINITY, false);
    }
    let (kind, tier) = match held.and_then(|s| item::def(s.item)).map(|d| d.kind) {
        Some(ItemKind::Tool { kind, tier, .. }) => (kind, tier),
        _ => (ToolKind::None, 0),
    };
    let correct = d.tool != ToolKind::None && d.tool == kind;
    let can_harvest = d.min_tier == 0 || (correct && tier >= d.min_tier);
    let mut speed = if correct { tool_speed(tier) } else { 1.0 };
    if block_id(block) == id::COBWEB && kind == ToolKind::Sword {
        speed = 15.0;
    }
    if d.hardness == 0.0 {
        return (0.05, can_harvest);
    }
    let t = d.hardness * if can_harvest { 1.5 } else { 5.0 } / speed;
    (t, can_harvest)
}

impl Game {
    pub fn new(seed: u64, settings: &Settings, mode: GameMode) -> Self {
        let world = World::new(seed, None, settings.graphics.smooth_lighting);
        let spawn = find_spawn(&world);
        let mut player = Player::new(spawn, mode);
        if mode == GameMode::Creative {
            for (i, it) in [
                item::id::COBBLE,
                block::id::PLANKS as u16,
                block::id::GLASS as u16,
                block::id::TORCH as u16,
                block::id::SUPPORT as u16,
                block::id::STONE_BRICKS as u16,
                block::id::RESONITE_LAMP as u16,
                block::id::LADDER as u16,
                block::id::OAK_LOG as u16,
            ]
            .iter()
            .enumerate()
            {
                player.inventory.slots[i] = Some(ItemStack::new(*it, 64));
            }
        }
        let mut camera = Camera::new(spawn);
        camera.fov_deg = settings.graphics.fov as f32;
        log::info!("Новый мир: seed {seed}, точка появления {spawn:.1}");
        Self {
            world,
            player,
            entities: Entities::default(),
            camera,
            time: 0.0,
            show_debug: false,
            accumulator: 0.0,
            mining: None,
            use_cooldown: 0.0,
            attack_cooldown: 0.0,
            target: None,
            last_jump_press: -10.0,
            eye_smooth: player::EYE,
            hand_swing: 0.0,
            rng: Rng::new(seed ^ 0xABCDEF),
            messages: Vec::new(),
            fps: 0.0,
            fps_acc: 0.0,
            fps_frames: 0,
            frame_ms: 0.0,
        }
    }

    pub fn message(&mut self, text: impl Into<String>) {
        self.messages.push((text.into(), 4.0));
        if self.messages.len() > 6 {
            self.messages.remove(0);
        }
    }

    fn reach(&self) -> f32 {
        if self.player.creative() {
            REACH_CREATIVE
        } else {
            REACH_SURVIVAL
        }
    }

    /// Обновление кадра. `active` — мышь захвачена и игровое управление активно.
    pub fn update(&mut self, dt: f32, input: &InputState, settings: &Settings, active: bool) {
        self.time += dt as f64;
        self.fps_acc += dt;
        self.fps_frames += 1;
        if self.fps_acc >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_acc;
            self.frame_ms = self.fps_acc * 1000.0 / self.fps_frames as f32;
            self.fps_acc = 0.0;
            self.fps_frames = 0;
        }
        for m in &mut self.messages {
            m.1 -= dt;
        }
        self.messages.retain(|m| m.1 > 0.0);

        let c = &settings.controls;
        if active {
            if input.pressed(c, Action::Debug) {
                self.show_debug = !self.show_debug;
            }
            let k = 0.0025 * c.mouse_sensitivity as f32;
            let dy = if c.invert_y { -input.mouse_delta.1 } else { input.mouse_delta.1 };
            self.camera.rotate(input.mouse_delta.0 as f32 * k, dy as f32 * k);
            self.player.yaw = self.camera.yaw;
            self.player.pitch = self.camera.pitch;

            // Хотбар: цифры и колесо.
            let digits = [
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Digit5,
                KeyCode::Digit6,
                KeyCode::Digit7,
                KeyCode::Digit8,
                KeyCode::Digit9,
            ];
            for (i, k) in digits.iter().enumerate() {
                if input.key_pressed(*k) {
                    self.player.inventory.selected = i;
                }
            }
            if input.scroll != 0.0 {
                let s = self.player.inventory.selected as i32 - input.scroll.signum() as i32;
                self.player.inventory.selected = s.rem_euclid(HOTBAR as i32) as usize;
            }
            if input.key_pressed(KeyCode::F6) {
                self.player.mode = if self.player.creative() { GameMode::Survival } else { GameMode::Creative };
                self.player.flying = false;
                let name = if self.player.creative() { "Креатив" } else { "Выживание" };
                self.message(format!("Режим игры: {name}"));
            }
            if input.pressed(c, Action::ToggleFly) && self.player.creative() {
                self.player.flying = !self.player.flying;
            }
            if input.pressed(c, Action::Jump) {
                if self.player.creative() && self.time - self.last_jump_press < 0.3 {
                    self.player.flying = !self.player.flying;
                }
                self.last_jump_press = self.time;
            }
        }

        // --- Физика с фиксированным шагом ---
        let mut mv = MoveInput::default();
        if active && !self.player.is_dead() {
            let mut dir = Vec3::ZERO;
            if input.down(c, Action::Forward) {
                dir += self.camera.forward_flat();
            }
            if input.down(c, Action::Back) {
                dir -= self.camera.forward_flat();
            }
            if input.down(c, Action::Right) {
                dir += self.camera.right_flat();
            }
            if input.down(c, Action::Left) {
                dir -= self.camera.right_flat();
            }
            mv.dir = dir.normalize_or_zero();
            mv.jump = input.down(c, Action::Jump);
            mv.sneak = input.down(c, Action::Sneak);
            mv.sprint = input.down(c, Action::Sprint) && input.down(c, Action::Forward);
        }
        self.accumulator += dt as f64;
        let mut steps = 0;
        while self.accumulator >= PHYSICS_DT && steps < 15 {
            self.player.step(&self.world, mv, PHYSICS_DT);
            self.entities.step_items(&self.world, PHYSICS_DT);
            self.accumulator -= PHYSICS_DT;
            steps += 1;
        }
        if steps == 15 {
            self.accumulator = 0.0;
        }
        self.player.tick_status(&self.world, dt);
        self.pickup_items();

        // Камера: интерполяция между шагами физики, плавный присед.
        let alpha = self.accumulator / PHYSICS_DT;
        let eye_target = self.player.eye_height();
        self.eye_smooth += (eye_target - self.eye_smooth) * (dt as f64 * 12.0).min(1.0);
        let pos = self.player.prev_pos.lerp(self.player.pos, alpha);
        self.camera.pos = pos + DVec3::new(0.0, self.eye_smooth, 0.0);
        let fov_boost = if self.player.sprinting { 1.08 } else { 1.0 };
        let target_fov = settings.graphics.fov as f32 * fov_boost;
        self.camera.fov_deg += (target_fov - self.camera.fov_deg) * (dt * 8.0).min(1.0);

        // --- Взаимодействие с блоками ---
        let eye = self.player.eye_pos();
        let fwd = self.camera.forward();
        self.target = if self.player.is_dead() {
            None
        } else {
            physics::raycast(&self.world, eye, fwd, self.reach(), physics::pickable)
        };
        self.use_cooldown = (self.use_cooldown - dt).max(0.0);
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        self.hand_swing = (self.hand_swing - dt * 4.0).max(0.0);
        if active && !self.player.is_dead() {
            self.handle_attack(dt, input, settings);
            self.handle_use(input, settings);
            if input.pressed(c, Action::PickBlock) {
                self.pick_block();
            }
            if input.pressed(c, Action::Drop) {
                let all = input.key_down(KeyCode::ControlLeft);
                self.drop_selected(all);
            }
        } else {
            self.mining = None;
        }

        let p = self.player.pos;
        let center = ChunkPos::of_block(p.x.floor() as i32, p.z.floor() as i32);
        self.world.update(center, settings.graphics.render_distance as i32);
    }

    fn handle_attack(&mut self, dt: f32, input: &InputState, settings: &Settings) {
        let c = &settings.controls;
        let held = input.down(c, Action::Attack);
        if input.pressed(c, Action::Attack) {
            self.hand_swing = 1.0;
        }
        let Some(hit) = self.target else {
            self.mining = None;
            return;
        };
        if !held {
            self.mining = None;
            return;
        }
        if self.player.creative() {
            if input.pressed(c, Action::Attack) || self.attack_cooldown <= 0.0 {
                self.break_block(hit.pos, false);
                self.attack_cooldown = 0.2;
            }
            return;
        }
        let v = self.world.get(hit.pos.x, hit.pos.y, hit.pos.z);
        let (time, _) = break_time(v, self.player.inventory.selected_stack());
        if !time.is_finite() {
            self.mining = None;
            return;
        }
        match &mut self.mining {
            Some(m) if m.pos == hit.pos => m.progress += dt / time.max(0.001),
            _ => self.mining = Some(Mining { pos: hit.pos, progress: dt / time.max(0.001) }),
        }
        if self.hand_swing <= 0.0 {
            self.hand_swing = 1.0;
        }
        if self.mining.as_ref().map(|m| m.progress >= 1.0).unwrap_or(false) {
            self.mining = None;
            self.break_block(hit.pos, true);
            self.player.add_exhaustion(0.005);
        }
    }

    /// Разрушает блок; в выживании — выпадение предметов и износ инструмента.
    pub fn break_block(&mut self, pos: IVec3, survival: bool) {
        let v = self.world.get(pos.x, pos.y, pos.z);
        let bid = block_id(v);
        if bid == id::AIR || def(bid).hardness < 0.0 {
            return;
        }
        if !self.world.set(pos.x, pos.y, pos.z, 0) {
            return;
        }
        if survival {
            let held = self.player.inventory.selected_stack();
            let (_, can_harvest) = break_time(v, held);
            if can_harvest {
                for stack in self.drops_for(v) {
                    self.spawn_drop(pos, stack);
                }
            }
            if let Some(ItemKind::Tool { .. }) = held.and_then(|s| item::def(s.item)).map(|d| d.kind) {
                if def(bid).hardness > 0.0 && self.player.inventory.damage_selected(1) {
                    self.message("Инструмент сломался!");
                }
            }
        }
        self.after_block_removed(pos);
    }

    /// Что выпадает из блока.
    fn drops_for(&mut self, v: u16) -> Vec<ItemStack> {
        let bid = block_id(v);
        match def(bid).drop {
            Drop::SelfBlock => {
                if item::def(bid as u16).is_some() {
                    vec![ItemStack::new(bid as u16, 1)]
                } else {
                    vec![]
                }
            }
            Drop::Nothing => vec![],
            Drop::Item(it, lo, hi) => {
                let n = self.rng.range(lo as i32, hi as i32 + 1);
                if n > 0 {
                    vec![ItemStack::new(it, n as u8)]
                } else {
                    vec![]
                }
            }
            Drop::Chance(it, pct) => {
                if self.rng.f32() * 100.0 < pct as f32 {
                    vec![ItemStack::new(it, 1)]
                } else if bid == id::GRAVEL {
                    vec![ItemStack::new(id::GRAVEL as u16, 1)]
                } else {
                    vec![]
                }
            }
        }
    }

    pub fn spawn_drop(&mut self, pos: IVec3, stack: ItemStack) {
        let p = DVec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.3, pos.z as f64 + 0.5);
        let vel = DVec3::new((self.rng.f32() - 0.5) as f64 * 2.0, 3.0, (self.rng.f32() - 0.5) as f64 * 2.0);
        self.entities.spawn_item(stack, p, vel);
    }

    /// Последствия удаления блока: растения без опоры, падение песка.
    fn after_block_removed(&mut self, pos: IVec3) {
        // Блок сверху, которому нужна опора (трава, цветы, факел, рельсы).
        let above = self.world.get(pos.x, pos.y + 1, pos.z);
        let ad = def(block_id(above));
        if matches!(ad.shape, Shape::Cross | Shape::Torch | Shape::Rail) {
            self.world.set(pos.x, pos.y + 1, pos.z, 0);
            if !self.player.creative() {
                for s in self.drops_for(above) {
                    self.spawn_drop(pos + IVec3::Y, s);
                }
            }
        }
        // Сыпучие блоки сверху падают.
        let mut y = pos.y + 1;
        while def(self.world.get_id(pos.x, y, pos.z)).gravity {
            let v = self.world.get(pos.x, y, pos.z);
            let mut ny = y;
            while ny - 1 >= 0 && block::def(self.world.get_id(pos.x, ny - 1, pos.z)).replaceable {
                ny -= 1;
            }
            if ny == y {
                break;
            }
            self.world.set(pos.x, y, pos.z, 0);
            self.world.set(pos.x, ny, pos.z, v);
            y += 1;
        }
    }

    fn handle_use(&mut self, input: &InputState, settings: &Settings) {
        let c = &settings.controls;
        let pressed = input.pressed(c, Action::Use);
        let held = input.down(c, Action::Use);
        if !(pressed || (held && self.use_cooldown <= 0.0)) {
            return;
        }
        self.use_cooldown = 0.22;
        let stack = self.player.inventory.selected_stack();
        let kind = stack.and_then(|s| item::def(s.item)).map(|d| d.kind);

        // Еда — съесть.
        if let Some(ItemKind::Food { hunger }) = kind {
            if pressed && (self.player.hunger < player::MAX_HUNGER || self.player.creative()) {
                self.player.eat(hunger);
                if !self.player.creative() {
                    self.player.inventory.consume_selected();
                }
                self.hand_swing = 1.0;
                return;
            }
        }

        let Some(hit) = self.target else { return };
        self.try_place(hit);
    }

    /// Ставит блок из выбранного слота у грани `hit`. true — если поставлен.
    pub fn try_place(&mut self, hit: RayHit) -> bool {
        let stack = self.player.inventory.selected_stack();
        let kind = stack.and_then(|s| item::def(s.item)).map(|d| d.kind);
        let Some(ItemKind::Block(b)) = kind else { return false };
        let target = hit.pos + hit.normal;
        let cur = self.world.get(target.x, target.y, target.z);
        // Растение/вода на месте попадания — заменяем его самого.
        let hit_v = self.world.get(hit.pos.x, hit.pos.y, hit.pos.z);
        let (target, cur) = if def(block_id(hit_v)).replaceable && block_id(hit_v) != id::AIR {
            (hit.pos, hit_v)
        } else {
            (target, cur)
        };
        if !def(block_id(cur)).replaceable {
            return false;
        }
        let bd = def(b);
        if bd.solid {
            let cell = Aabb::block(target.x, target.y, target.z);
            if cell.intersects(&self.player.aabb()) {
                return false;
            }
        }
        // Метаданные: направление лицевой стороны, ориентация рельс, лестниц.
        let meta = match b {
            id::FURNACE | id::CHEST | id::CRAFTING_TABLE => facing_towards_player(self.camera.forward()),
            id::RAIL => {
                let f = self.camera.forward();
                if f.x.abs() > f.z.abs() { 1 } else { 0 }
            }
            id::LADDER => {
                if hit.normal.y != 0 {
                    return false;
                }
                // Стена — с противоположной стороны от нормали.
                match (hit.normal.x, hit.normal.z) {
                    (1, _) => 3,
                    (-1, _) => 2,
                    (_, 1) => 5,
                    _ => 4,
                }
            }
            id::TORCH => 255,
            _ => 0,
        };
        // Растениям и факелам нужна опора снизу.
        if matches!(bd.shape, Shape::Cross | Shape::Torch | Shape::Rail) && !self.world.is_solid(target.x, target.y - 1, target.z) {
            return false;
        }
        if self.world.set(target.x, target.y, target.z, block::make(b, meta)) {
            if !self.player.creative() {
                self.player.inventory.consume_selected();
            }
            self.hand_swing = 1.0;
            // Сыпучий блок над пустотой падает сразу.
            if bd.gravity {
                self.after_block_removed(target - IVec3::Y);
            }
            return true;
        }
        false
    }

    fn pick_block(&mut self) {
        let Some(hit) = self.target else { return };
        let bid = self.world.get_id(hit.pos.x, hit.pos.y, hit.pos.z);
        let item_id = match bid {
            id::FURNACE_LIT => id::FURNACE as u16,
            id::BURNT_TORCH => id::TORCH as u16,
            b => b as u16,
        };
        if item::def(item_id).is_none() {
            return;
        }
        let creative = self.player.creative();
        let inv = &mut self.player.inventory;
        if let Some(i) = (0..HOTBAR).find(|&i| inv.slots[i].map(|s| s.item) == Some(item_id)) {
            inv.selected = i;
        } else if creative {
            let sel = inv.selected;
            inv.slots[sel] = Some(ItemStack::new(item_id, 64));
        }
    }

    fn drop_selected(&mut self, all: bool) {
        let sel = self.player.inventory.selected;
        let Some(mut s) = self.player.inventory.slots[sel] else { return };
        let n = if all { s.count } else { 1 };
        s.count -= n;
        let thrown = ItemStack { count: n, ..s };
        self.player.inventory.slots[sel] = if s.count == 0 { None } else { Some(s) };
        self.throw_stack(thrown);
    }

    pub fn throw_stack(&mut self, stack: ItemStack) {
        let f = self.camera.forward().as_dvec3();
        let mut it = entities::ItemEntity::new(stack, self.player.eye_pos() - DVec3::new(0.0, 0.3, 0.0), f * 6.0 + DVec3::new(0.0, 1.5, 0.0));
        it.pickup_delay = 1.5;
        self.entities.items.push(it);
    }

    fn pickup_items(&mut self) {
        if self.player.is_dead() {
            return;
        }
        let center = self.player.pos + DVec3::new(0.0, 0.9, 0.0);
        for it in &mut self.entities.items {
            if it.pickup_delay > 0.0 {
                continue;
            }
            let d = it.pos.distance(center);
            if d < 1.8 {
                // Притягиваем, а на расстоянии < 0.9 — подбираем.
                if d < 0.9 {
                    match self.player.inventory.add(it.stack) {
                        None => it.stack.count = 0,
                        Some(rest) => it.stack = rest,
                    }
                } else {
                    let dir = (center - it.pos).normalize();
                    it.vel = it.vel.lerp(dir * 6.0, 0.3);
                }
            }
        }
        self.entities.items.retain(|i| i.stack.count > 0);
    }

    /// Смерть: выпадение инвентаря и возрождение.
    pub fn respawn(&mut self) {
        let pos = self.player.pos;
        for i in 0..self.player.inventory.slots.len() {
            if let Some(s) = self.player.inventory.slots[i].take() {
                let v = DVec3::new((self.rng.f32() - 0.5) as f64 * 4.0, 3.0, (self.rng.f32() - 0.5) as f64 * 4.0);
                self.entities.spawn_item(s, pos + DVec3::new(0.0, 1.0, 0.0), v);
            }
        }
        self.player.health = player::MAX_HEALTH;
        self.player.hunger = player::MAX_HUNGER;
        self.player.air = 10.0;
        self.player.vel = DVec3::ZERO;
        self.player.pos = self.player.spawn;
        self.player.prev_pos = self.player.spawn;
        self.player.unstuck(&self.world);
    }

    /// Загружает готовые меши в рендерер (с ограничением на кадр).
    pub fn upload_meshes(&mut self, renderer: &mut Renderer) {
        for p in self.world.unloaded.drain(..) {
            renderer.remove_chunk_mesh(p);
        }
        let budget = 24;
        let n = self.world.ready_meshes.len().min(budget);
        for (p, m) in self.world.ready_meshes.drain(..n) {
            if !self.world.chunks.contains_key(&p) {
                continue;
            }
            if let Err(e) = renderer.upload_chunk_mesh(p, &m.opaque, &m.translucent, m.min_y, m.max_y) {
                log::error!("загрузка меша {p:?}: {e:#}");
            }
        }
    }

    pub fn globals(&self, settings: &Settings) -> Globals {
        let p = self.camera.pos;
        let rd = settings.graphics.render_distance as f32 * 16.0;
        let sun = Vec3::new(0.35, 0.8, -0.45).normalize();
        let underwater = self.world.get_id(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32) == id::WATER;
        let horizon = srgb([0.72, 0.84, 0.98]);
        let (fog_color, fog_start, fog_end) = if underwater {
            (srgb([0.1, 0.25, 0.55]), 0.0, 24.0)
        } else {
            (horizon, rd * 0.55, rd * 0.95)
        };
        Globals {
            cam_pos: [p.x as f32, p.y as f32, p.z as f32, self.time as f32],
            fog_color: [fog_color[0], fog_color[1], fog_color[2], fog_start],
            params: [fog_end, 1.0, if underwater { 1.0 } else { 0.0 }, 0.015],
            sun_dir: [sun.x, sun.y, sun.z, 0.3],
            sky_top: srgb([0.35, 0.55, 0.95]),
            sky_horizon: horizon,
            block_light: [1.0, 0.8, 0.55, settings.graphics.brightness as f32],
            sky_light: [1.0, 1.0, 1.0, 0.0],
            ..Default::default()
        }
    }

    /// Геометрия сущностей, рамки выбора, трещин и предмета в руке.
    pub fn build_geometry(&self, settings: &Settings) -> FrameGeometry {
        let mut g = FrameGeometry::default();
        let cam = self.camera.pos;
        self.entities.build_item_mesh(&self.world, cam, self.time as f32, &mut g.entities);

        if let Some(hit) = self.target {
            let min = (hit.pos.as_dvec3() - cam).as_vec3() - Vec3::splat(0.002);
            let max = min + Vec3::splat(1.004);
            mesh::push_wire_box(&mut g.lines, min, max, [10, 10, 10, 200]);
            if let Some(m) = &self.mining {
                if m.pos == hit.pos && m.progress > 0.0 {
                    let stage = ((m.progress * 10.0) as u16).min(9);
                    let (sky, blk) = self.world.light(hit.pos.x + hit.normal.x, hit.pos.y + hit.normal.y, hit.pos.z + hit.normal.z);
                    let layer = crate::assets::textures::Tex::Destroy0 as u16 + stage;
                    mesh::push_overlay_cube(&mut g.translucent, min - Vec3::splat(0.001), max + Vec3::splat(0.001), layer, pack_light(sky, blk));
                }
            }
        }

        // Предмет в руке (рисуется поверх мира после очистки глубины).
        if !self.player.is_dead() {
            let p = self.player.pos;
            let (sky, blk) = self.world.light(p.x.floor() as i32, (p.y + 1.0).floor() as i32, p.z.floor() as i32);
            let light = pack_light(sky, blk);
            let swing = (self.hand_swing * std::f32::consts::PI).sin();
            let bob = if settings.graphics.view_bobbing && self.player.on_ground {
                (self.player.walk_dist as f32 * 2.2).sin() * 0.03
            } else {
                0.0
            };
            let rot = Quat::from_rotation_y(self.camera.yaw) * Quat::from_rotation_x(self.camera.pitch);
            let local = Vec3::new(0.42, -0.38 + bob - swing * 0.15, -0.65 + swing * 0.1);
            let model = Mat4::from_rotation_translation(rot, Vec3::ZERO)
                * Mat4::from_translation(local)
                * Mat4::from_rotation_y(-0.6 - swing * 0.5)
                * Mat4::from_rotation_x(-swing * 0.8);
            match self.player.inventory.selected_stack() {
                Some(st) => entities::push_item_model(&mut g.overlay, &model, st.item, light, 0.32),
                None => {
                    // Рука — вытянутый кубоид цвета кожи.
                    let faces = [mesh::FaceTex::none(); 6];
                    let m = model * Mat4::from_rotation_x(0.3);
                    mesh::push_box(&mut g.overlay, &m, Vec3::new(-0.07, -0.35, -0.07), Vec3::new(0.07, 0.1, 0.07), &faces, [224, 172, 140, 255], light);
                }
            }
        }
        g
    }

    pub fn draw_hud(&self, ui: &mut UiBatch, renderer: &Renderer) {
        hud::draw_crosshair(ui);
        hud::draw_hotbar(ui, &self.player);
        hud::draw_status(ui, &self.player);

        // Сообщения.
        let s = ui.scale;
        let mut y = ui.height - 140.0 * s;
        for (text, t) in self.messages.iter().rev() {
            let a = (t.min(1.0) * 255.0) as u8;
            ui.text_shadow(10.0 * s, y, text, 0.9, [255, 255, 255, a]);
            y -= ui.line_height(0.9);
        }

        // Эффект урона — красная вспышка.
        if self.player.hurt_timer > 0.0 {
            let a = (self.player.hurt_timer / 0.4 * 90.0) as u8;
            ui.rect(0.0, 0.0, ui.width, ui.height, [200, 0, 0, a]);
        }
        if self.player.head_in_water {
            ui.rect(0.0, 0.0, ui.width, ui.height, [20, 60, 160, 70]);
        }

        if self.show_debug {
            self.draw_debug(ui, renderer);
        }
    }

    fn draw_debug(&self, ui: &mut UiBatch, renderer: &Renderer) {
        let s = ui.scale;
        let p = self.player.pos;
        let (bx, by, bz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let cp = ChunkPos::of_block(bx, bz);
        let stats = renderer.stats;
        let (gen_q, mesh_q) = self.world.pending_jobs();
        let (sky, blk) = self.world.light(bx, by, bz);
        let f = self.camera.forward();
        let facing = if f.x.abs() > f.z.abs() {
            if f.x > 0.0 { "восток (+X)" } else { "запад (-X)" }
        } else if f.z > 0.0 {
            "юг (+Z)"
        } else {
            "север (-Z)"
        };
        let mut lines = vec![
            format!("PlusCraft 0.3 — {:.0} FPS ({:.1} мс)", self.fps, self.frame_ms),
            format!("XYZ: {:.2} / {:.2} / {:.2}", p.x, p.y, p.z),
            format!("Блок: {bx} {by} {bz}   Чанк: {} {} [{} {}]", cp.x, cp.z, bx & 15, bz & 15),
            format!("Взгляд: {facing}  (рыск {:.0}°, тангаж {:.0}°)", self.camera.yaw.to_degrees(), self.camera.pitch.to_degrees()),
            format!("Биом: {}", biome::get(self.world.biome_at(bx, bz)).name),
            format!("Свет: небо {sky}, блоки {blk}"),
            format!(
                "Чанки: {} загружено, {} мешей, видно {}, квадов {}",
                self.world.chunks.len(),
                stats.chunks_total,
                stats.chunks_drawn,
                stats.quads_drawn
            ),
            format!("Задачи: генерация {gen_q}, меши {mesh_q}, потоков {}", self.world.worker_count()),
            format!("Сущности: предметов {}", self.entities.items.len()),
            format!("Seed: {}", self.world.seed),
            format!("GPU: {}", renderer.device_name()),
        ];
        if let Some(hit) = self.target {
            let v = self.world.get(hit.pos.x, hit.pos.y, hit.pos.z);
            lines.push(format!(
                "Цель: {} [{} {} {}] meta {}",
                def(block_id(v)).name,
                hit.pos.x,
                hit.pos.y,
                hit.pos.z,
                block_meta(v)
            ));
        }
        let mut y = 6.0 * s;
        for l in &lines {
            let tw = ui.text_width(l, 0.8);
            ui.rect(4.0 * s, y - 1.0 * s, tw + 6.0 * s, ui.line_height(0.8), [0, 0, 0, 120]);
            ui.text(7.0 * s, y, l, 0.8, WHITE);
            y += ui.line_height(0.8);
        }
    }
}

/// Направление лицевой стороны блока — к игроку (2..5 = +X, -X, +Z, -Z).
fn facing_towards_player(forward: Vec3) -> u8 {
    if forward.x.abs() > forward.z.abs() {
        if forward.x > 0.0 { 3 } else { 2 }
    } else if forward.z > 0.0 {
        5
    } else {
        4
    }
}
