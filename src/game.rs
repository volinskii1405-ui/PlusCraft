//! Игровое состояние: мир, игрок, сущности; фиксированный шаг физики,
//! добыча и установка блоков, подготовка кадра.

use glam::{DVec3, IVec3, Mat4, Quat, Vec3};
use winit::keyboard::KeyCode;

use crate::blockentity::BlockEntities;
use crate::camera::Camera;
use crate::crafting::Recipes;
use crate::entities::mesh::{self, pack_light};
use crate::entities::mob::{MobEvent, MobKind};
use crate::entities::spawn::{self, SpawnRules};
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
use crate::ui::console::Console;
use crate::ui::hud;
use crate::ui::screens::{self, ContainerUi, Screen};
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
    /// Время суток 0..1 (0 — рассвет, 0.25 — полдень).
    pub day_time: f64,
    /// Номер дня с начала мира.
    pub day_count: u32,
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
    pub console: Console,
    pub recipes: Recipes,
    pub block_entities: BlockEntities,
    /// Открытый экран инвентаря/контейнера.
    pub container: Option<ContainerUi>,
    ai_timer: f32,
    spawn_timer: f32,
    last_alpha: f64,
    /// Моб под прицелом (индекс в entities.mobs).
    pub target_mob: Option<usize>,
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
            day_time: 0.02,
            day_count: 0,
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
            console: Console::default(),
            recipes: Recipes::load(),
            block_entities: BlockEntities::default(),
            container: None,
            ai_timer: 0.0,
            spawn_timer: 0.0,
            last_alpha: 0.0,
            target_mob: None,
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

    /// Выполняет консольную команду и возвращает текст ответа.
    pub fn run_command(&mut self, cmd: &str) -> String {
        let cmd = cmd.trim().trim_start_matches('/');
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let Some(&name) = parts.first() else { return String::new() };
        match name {
            "help" | "помощь" => "Команды: /time day|night|noon|midnight|0..1, /gamemode s|c, /tp x y z, /give предмет [n], /setblock x y z блок, /fill x1 y1 z1 x2 y2 z2 блок, /summon моб [мутация], /heal, /kill, /seed".into(),
            "time" => {
                let t = match parts.get(1).copied() {
                    Some("day") | Some("день") => 0.05,
                    Some("noon") | Some("полдень") => 0.25,
                    Some("sunset") | Some("закат") => 0.48,
                    Some("night") | Some("ночь") => 0.6,
                    Some("midnight") | Some("полночь") => 0.75,
                    Some(v) => match v.parse::<f64>() {
                        Ok(x) => x.rem_euclid(1.0),
                        Err(_) => return format!("Неизвестное время: {v}"),
                    },
                    None => return format!("Сейчас {} (день {})", crate::sky::clock(self.day_time), self.day_count + 1),
                };
                self.day_time = t;
                format!("Время установлено: {}", crate::sky::clock(t))
            }
            "gamemode" | "gm" => {
                let mode = match parts.get(1).copied() {
                    Some("c") | Some("1") | Some("creative") | Some("креатив") => GameMode::Creative,
                    Some("s") | Some("0") | Some("survival") | Some("выживание") => GameMode::Survival,
                    _ => return "Использование: /gamemode s|c".into(),
                };
                self.player.mode = mode;
                self.player.flying = false;
                format!("Режим игры: {}", if mode == GameMode::Creative { "креатив" } else { "выживание" })
            }
            "tp" => {
                let v: Vec<f64> = parts[1..].iter().filter_map(|p| p.parse().ok()).collect();
                if v.len() != 3 {
                    return "Использование: /tp x y z".into();
                }
                self.player.pos = DVec3::new(v[0], v[1], v[2]);
                self.player.prev_pos = self.player.pos;
                self.player.vel = DVec3::ZERO;
                format!("Телепорт в {:.1} {:.1} {:.1}", v[0], v[1], v[2])
            }
            "give" => {
                let Some(key) = parts.get(1) else { return "Использование: /give предмет [количество]".into() };
                let n: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1).clamp(1, 64 * 36);
                let Some(id) = item::by_key(key) else { return format!("Нет предмета «{key}»") };
                let mut left = n;
                while left > 0 {
                    let k = left.min(item::max_stack(id) as u32);
                    left -= k;
                    if let Some(rest) = self.player.inventory.add(ItemStack::new(id, k as u8)) {
                        let p = self.player.pos.floor().as_ivec3();
                        self.spawn_drop(p, rest);
                    }
                }
                format!("Выдано: {} × {}", n, item::name(id))
            }
            "heal" => {
                self.player.health = player::MAX_HEALTH;
                self.player.hunger = player::MAX_HUNGER;
                "Здоровье восстановлено".into()
            }
            "kill" => {
                self.player.health = 0.0;
                "Вы погибли".into()
            }
            "seed" => format!("Seed: {}", self.world.seed),
            "summon" => {
                let kind = match parts.get(1).copied() {
                    Some("grazer") => MobKind::Grazer,
                    Some("boar") => MobKind::Boar,
                    Some("crawler") => MobKind::Crawler,
                    Some("spider") => MobKind::Spider,
                    Some("gloom") => MobKind::Gloom,
                    _ => return "Использование: /summon grazer|boar|crawler|spider|gloom [мутация 0..3] [x y z]".into(),
                };
                let level = parts.get(2).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0).min(3);
                let f = self.camera.forward_flat().as_dvec3();
                let coords: Vec<f64> = parts.iter().skip(3).take(3).filter_map(|p| p.parse().ok()).collect();
                let p = if coords.len() == 3 {
                    DVec3::new(coords[0], coords[1], coords[2])
                } else {
                    self.player.pos + f * 4.0 + DVec3::new(0.0, 0.5, 0.0)
                };
                let m = crate::entities::mob::Mob::new(kind, p, crate::entities::mob::Mutation { level }, &mut self.rng);
                let name = m.display_name();
                self.entities.mobs.push(m);
                format!("Призван: {name}")
            }
            "fill" => {
                let v: Vec<i32> = parts.iter().skip(1).take(6).filter_map(|p| p.parse().ok()).collect();
                let (Some(key), true) = (parts.get(7), v.len() == 6) else { return "Использование: /fill x1 y1 z1 x2 y2 z2 блок".into() };
                let b = if *key == "air" { Some(id::AIR) } else { block::by_key(key) };
                let Some(b) = b else { return format!("Нет блока «{key}»") };
                let (x0, x1) = (v[0].min(v[3]), v[0].max(v[3]));
                let (y0, y1) = (v[1].min(v[4]), v[1].max(v[4]));
                let (z0, z1) = (v[2].min(v[5]), v[2].max(v[5]));
                let vol = (x1 - x0 + 1) as i64 * (y1 - y0 + 1) as i64 * (z1 - z0 + 1) as i64;
                if vol > 65536 {
                    return format!("Слишком большой объём: {vol} (макс. 65536)");
                }
                let mut n = 0;
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        for x in x0..=x1 {
                            if self.world.set(x, y, z, block::make(b, 0)) {
                                n += 1;
                            }
                        }
                    }
                }
                format!("Заполнено блоков: {n}")
            }
            "ui" => {
                // Отладка: ставит контейнер рядом с игроком и открывает его.
                let p = self.player.pos.floor().as_ivec3() + IVec3::new(0, 0, -2);
                let (b, sc) = match parts.get(1).copied() {
                    Some("crafting") => (id::CRAFTING_TABLE, Screen::Crafting(p)),
                    Some("furnace") => (id::FURNACE, Screen::Furnace(p)),
                    Some("chest") => (id::CHEST, Screen::Chest(p)),
                    _ => return "Использование: /ui crafting|furnace|chest".into(),
                };
                self.world.set(p.x, p.y, p.z, block::make(b, 4));
                screens::open(self, sc);
                "Открыто".into()
            }
            "setblock" => {
                let v: Vec<i32> = parts.iter().skip(1).take(3).filter_map(|p| p.parse().ok()).collect();
                let (Some(key), true) = (parts.get(4), v.len() == 3) else { return "Использование: /setblock x y z блок".into() };
                let Some(b) = block::by_key(key) else { return format!("Нет блока «{key}»") };
                let meta = if b == id::TORCH { 255 } else { 0 };
                if self.world.set(v[0], v[1], v[2], block::make(b, meta)) {
                    format!("Блок {} установлен", def(b).name)
                } else {
                    "Чанк не загружен".into()
                }
            }
            other => format!("Неизвестная команда: /{other} (см. /help)"),
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
        // Консоль команд перехватывает клавиатуру.
        if active && !self.console.open && input.pressed(&settings.controls, Action::Chat) {
            self.console.open_with("/");
        } else if let Some(cmd) = self.console.update(input) {
            let reply = self.run_command(&cmd);
            self.message(reply);
        }
        let active = active && !self.console.open;
        if input.pressed(&settings.controls, Action::Inventory)
            && self.container.is_none()
            && !self.console.open
            && !self.player.is_dead()
        {
            screens::open(self, Screen::Inventory);
        }
        self.time += dt as f64;
        self.fps_acc += dt;
        self.fps_frames += 1;
        if self.fps_acc >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_acc;
            self.frame_ms = self.fps_acc * 1000.0 / self.fps_frames as f32;
            self.fps_acc = 0.0;
            self.fps_frames = 0;
        }
        let day_len = (settings.gameplay.day_length_minutes * 60.0).max(10.0);
        self.day_time += dt as f64 / day_len;
        if self.day_time >= 1.0 {
            self.day_time -= 1.0;
            self.day_count += 1;
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
        let mut mob_events = Vec::new();
        // Враждебные мобы не охотятся на игрока в креативе.
        let player_alive = !self.player.is_dead() && !self.player.creative();
        while self.accumulator >= PHYSICS_DT && steps < 15 {
            self.player.step(&self.world, mv, PHYSICS_DT);
            self.entities.step_items(&self.world, PHYSICS_DT);
            let pp = self.player.pos;
            for m in &mut self.entities.mobs {
                m.step(&self.world, pp, player_alive, PHYSICS_DT, &mut mob_events);
            }
            self.accumulator -= PHYSICS_DT;
            steps += 1;
        }
        if steps == 15 {
            self.accumulator = 0.0;
        }
        self.player.tick_status(&self.world, dt);
        self.pickup_items();
        self.tick_furnaces(dt);
        self.update_mobs(dt, settings, mob_events);

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
        self.target_mob = self.ray_mob(self.reach() as f64).and_then(|(i, t)| {
            if t < self.target.map(|h| h.dist as f64).unwrap_or(f64::MAX) {
                Some(i)
            } else {
                None
            }
        });
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
            // Моб ближе блока — бьём моба.
            let block_dist = self.target.map(|h| h.dist as f64).unwrap_or(f64::MAX);
            if let Some((idx, t)) = self.ray_mob(self.reach() as f64) {
                if t < block_dist && self.attack_cooldown <= 0.0 {
                    self.attack_mob(idx);
                    self.attack_cooldown = 0.45;
                    self.mining = None;
                    return;
                }
            }
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
        // Содержимое сундука/печи выпадает (лут шахт генерируется сейчас).
        if bid == id::CHEST && block_meta(v) & crate::world::mines::CHEST_LOOT_FLAG != 0 {
            let seed = self.world.seed;
            self.block_entities.chest_mut(pos, || crate::blockentity::Chest::mine_loot(seed, pos));
        }
        if !self.world.set(pos.x, pos.y, pos.z, 0) {
            return;
        }
        for st in self.block_entities.remove(pos) {
            self.spawn_drop(pos, st);
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
        // Взаимодействие с верстаком, печью, сундуком (с Shift — ставим блок).
        if pressed && !input.down(c, Action::Sneak) {
            let screen = match self.world.get_id(hit.pos.x, hit.pos.y, hit.pos.z) {
                id::CRAFTING_TABLE => Some(Screen::Crafting(hit.pos)),
                id::FURNACE | id::FURNACE_LIT => Some(Screen::Furnace(hit.pos)),
                id::CHEST => Some(Screen::Chest(hit.pos)),
                _ => None,
            };
            if let Some(sc) = screen {
                screens::open(self, sc);
                return;
            }
        }
        self.try_place(hit);
    }

    /// ИИ, спавн, деспавн, смерть мобов и их воздействие на игрока.
    fn update_mobs(&mut self, dt: f32, settings: &Settings, events: Vec<MobEvent>) {
        let daylight = crate::sky::sky_state(self.day_time).daylight;
        let pp = self.player.pos;
        let alive = !self.player.is_dead() && !self.player.creative();
        for e in events {
            match e {
                MobEvent::HitPlayer { damage, from } => {
                    let dmg = damage * settings.gameplay.difficulty.damage_mul() as f32;
                    if dmg > 0.0 && !self.player.creative() {
                        self.player.damage(dmg);
                        let mut d = self.player.pos - from;
                        d.y = 0.0;
                        self.player.vel += d.normalize_or_zero() * 6.0 + DVec3::new(0.0, 4.5, 0.0);
                    }
                }
                MobEvent::SnuffTorch(p) => {
                    if self.world.get_id(p.x, p.y, p.z) == id::TORCH {
                        self.world.set(p.x, p.y, p.z, block::make(id::BURNT_TORCH, 0));
                    }
                }
            }
        }
        // Мирная сложность — враждебных нет.
        if settings.gameplay.difficulty == crate::settings::Difficulty::Peaceful {
            self.entities.mobs.retain(|m| !m.kind.hostile());
        }
        self.ai_timer += dt;
        if self.ai_timer >= 0.1 {
            let step = self.ai_timer;
            self.ai_timer = 0.0;
            for m in &mut self.entities.mobs {
                m.think(&self.world, pp, alive, step, &mut self.rng);
                // Ползуны горят на солнце.
                if m.burns_in_daylight() && daylight > 0.7 && !m.is_dead() {
                    let (sky, _) = self.world.light(m.pos.x.floor() as i32, (m.pos.y + 1.6).floor() as i32, m.pos.z.floor() as i32);
                    if sky >= 15 {
                        m.health -= 1.0 * step * 2.0;
                        m.burn_timer = 0.3;
                    }
                }
            }
            // Мобы не слипаются.
            let n = self.entities.mobs.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    let (a, b) = self.entities.mobs.split_at_mut(j);
                    let (x, y) = (&mut a[i], &mut b[0]);
                    let mut d = x.pos - y.pos;
                    d.y = 0.0;
                    let l = d.length();
                    if l < 0.7 && l > 1e-4 {
                        let push = d / l * (0.7 - l) * 0.5;
                        x.vel += push * 4.0;
                        y.vel -= push * 4.0;
                    }
                }
            }
        }
        self.spawn_timer += dt;
        if self.spawn_timer >= 0.5 {
            self.spawn_timer = 0.0;
            let rules = SpawnRules {
                daylight: daylight as f32,
                difficulty: settings.gameplay.difficulty,
                mob_difficulty: settings.gameplay.mob_difficulty,
            };
            spawn::try_spawn(&self.world, &mut self.entities.mobs, pp, &rules, &mut self.rng);
        }
        spawn::despawn(&mut self.entities.mobs, pp, dt, &mut self.rng);
        // Погибшие: дроп после анимации падения.
        let mut drops = Vec::new();
        self.entities.mobs.retain(|m| {
            if m.is_dead() && m.dead_timer > 0.8 {
                drops.push((m.kind, m.pos));
                false
            } else {
                true
            }
        });
        for (kind, pos) in drops {
            for st in kind.drops(&mut self.rng) {
                self.entities.spawn_item(st, pos + DVec3::new(0.0, 0.5, 0.0), DVec3::new(0.0, 3.0, 0.0));
            }
        }
    }

    /// Ближайший моб на луче взгляда (дистанция).
    fn ray_mob(&self, max: f64) -> Option<(usize, f64)> {
        let eye = self.player.eye_pos();
        let dir = self.camera.forward();
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.entities.mobs.iter().enumerate() {
            if m.is_dead() {
                continue;
            }
            if let Some(t) = m.ray_hit(eye, dir, max) {
                if best.map(|b| t < b.1).unwrap_or(true) {
                    best = Some((i, t));
                }
            }
        }
        best
    }

    /// Удар по мобу оружием из руки.
    fn attack_mob(&mut self, idx: usize) {
        let held = self.player.inventory.selected_stack();
        let (dmg, is_tool) = match held.and_then(|s| item::def(s.item)).map(|d| d.kind) {
            Some(ItemKind::Tool { damage, .. }) => (damage, true),
            _ => (1.0, false),
        };
        let crit = !self.player.on_ground && self.player.vel.y < 0.0;
        let dmg = if crit { dmg * 1.5 } else { dmg };
        let from = self.player.pos;
        if let Some(m) = self.entities.mobs.get_mut(idx) {
            m.hurt(dmg, from);
        }
        if is_tool && !self.player.creative() && self.player.inventory.damage_selected(1) {
            self.message("Инструмент сломался!");
        }
        self.player.add_exhaustion(0.1);
    }

    /// Плавка во всех печах; горящая печь светится (смена блока).
    fn tick_furnaces(&mut self, dt: f32) {
        let mut changed = Vec::new();
        for (&(x, y, z), e) in self.block_entities.map.iter_mut() {
            if let crate::blockentity::BlockEntity::Furnace(f) = e {
                if f.tick(&self.recipes, dt) {
                    changed.push((IVec3::new(x, y, z), f.is_burning()));
                }
            }
        }
        for (p, burning) in changed {
            let v = self.world.get(p.x, p.y, p.z);
            let b = block_id(v);
            if b == id::FURNACE || b == id::FURNACE_LIT {
                let nb = if burning { id::FURNACE_LIT } else { id::FURNACE };
                self.world.set(p.x, p.y, p.z, block::make(nb, block_meta(v)));
            }
        }
    }

    /// Нужен ли сейчас курсор мыши (открыт экран).
    pub fn wants_cursor(&self) -> bool {
        self.container.is_some()
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
        let sky = crate::sky::sky_state(self.day_time);
        let underwater = self.world.get_id(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32) == id::WATER;
        let in_lava = self.world.get_id(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32) == id::LAVA;
        let horizon = srgb(sky.sky_horizon);
        // Туман приглушённее неба на закате, чтобы рельеф не «заливало» розовым.
        let calm = crate::sky::sky_state(0.25).sky_horizon;
        let fog_srgb = [
            sky.sky_horizon[0] * 0.6 + calm[0] * 0.4 * sky.daylight,
            sky.sky_horizon[1] * 0.6 + calm[1] * 0.4 * sky.daylight,
            sky.sky_horizon[2] * 0.6 + calm[2] * 0.4 * sky.daylight,
        ];
        let fog_lin = srgb(fog_srgb);
        // Туман в глубине пещер темнеет: смешиваем с чёрным по небесному свету у камеры.
        let (cam_sky, _) = self.world.light(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let cave = 1.0 - (cam_sky as f32 / 15.0);
        let fog_base = [fog_lin[0] * (1.0 - cave * 0.85), fog_lin[1] * (1.0 - cave * 0.85), fog_lin[2] * (1.0 - cave * 0.85), 1.0];
        let (fog_color, fog_start, fog_end) = if in_lava {
            (srgb([0.9, 0.3, 0.05]), 0.0, 2.5)
        } else if underwater {
            let k = sky.daylight;
            (srgb([0.08 * k, 0.22 * k, 0.5 * k]), 0.0, 22.0)
        } else {
            (fog_base, rd * 0.55, rd * 0.95)
        };
        let sl = srgb(sky.sky_light);
        Globals {
            cam_pos: [p.x as f32, p.y as f32, p.z as f32, self.time as f32],
            fog_color: [fog_color[0], fog_color[1], fog_color[2], fog_start],
            params: [fog_end, sky.daylight, if underwater { 1.0 } else { 0.0 }, 0.012],
            sun_dir: [sky.sun_dir.x, sky.sun_dir.y, sky.sun_dir.z, self.day_time as f32],
            sky_top: srgb(sky.sky_top),
            sky_horizon: horizon,
            block_light: [1.0, 0.78, 0.5, settings.graphics.brightness as f32],
            sky_light: [sl[0], sl[1], sl[2], 0.0],
            ..Default::default()
        }
    }

    /// Геометрия сущностей, рамки выбора, трещин и предмета в руке.
    pub fn build_geometry(&self, settings: &Settings) -> FrameGeometry {
        let mut g = FrameGeometry::default();
        let cam = self.camera.pos;
        self.entities.build_item_mesh(&self.world, cam, self.time as f32, &mut g.entities);
        let alpha = (self.accumulator / PHYSICS_DT).clamp(0.0, 1.0);
        for m in &self.entities.mobs {
            if m.pos.distance(cam) < 96.0 {
                m.build_mesh(&self.world, cam, alpha, &mut g.entities);
            }
        }

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
                Some(st) => {
                    let is_block = matches!(item::def(st.item).map(|d| d.kind), Some(ItemKind::Block(b)) if def(b).shape == Shape::Cube);
                    entities::push_item_model(&mut g.overlay, &model, st.item, light, if is_block { 0.3 } else { 0.2 });
                }
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
        if let Some(m) = self.target_mob.and_then(|i| self.entities.mobs.get(i)) {
            let s = ui.scale;
            let text = format!("{}  {:.0}/{:.0}", m.display_name(), m.health.max(0.0), m.max_health);
            ui.text_centered(ui.width / 2.0, ui.height / 2.0 + 16.0 * s, &text, 0.85, if m.kind.hostile() { [255, 170, 160, 255] } else { [200, 255, 200, 255] });
            let w = 80.0 * s;
            let frac = (m.health / m.max_health).clamp(0.0, 1.0);
            ui.rect(ui.width / 2.0 - w / 2.0, ui.height / 2.0 + 36.0 * s, w, 4.0 * s, [0, 0, 0, 180]);
            ui.rect(ui.width / 2.0 - w / 2.0, ui.height / 2.0 + 36.0 * s, w * frac, 4.0 * s, [220, 40, 40, 255]);
        }

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

        self.console.draw(ui, self.time);
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
            format!("Свет: небо {sky}, блоки {blk}   Время: {} (день {})", crate::sky::clock(self.day_time), self.day_count + 1),
            format!(
                "Чанки: {} загружено, {} мешей, видно {}, квадов {}",
                self.world.chunks.len(),
                stats.chunks_total,
                stats.chunks_drawn,
                stats.quads_drawn
            ),
            format!("Задачи: генерация {gen_q}, меши {mesh_q}, потоков {}", self.world.worker_count()),
            format!(
                "Сущности: предметов {}, мобов {} (враждебных {})",
                self.entities.items.len(),
                self.entities.mobs.len(),
                self.entities.mobs.iter().filter(|m| m.kind.hostile()).count()
            ),
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
