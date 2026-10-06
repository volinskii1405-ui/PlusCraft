//! Игровое состояние: мир, камера, обновление и подготовка кадра.

use glam::{DVec3, Vec3};
use winit::keyboard::KeyCode;

use crate::camera::Camera;
use crate::input::InputState;
use crate::renderer::{Globals, Renderer};
use crate::settings::{Action, Settings};
use crate::ui::draw::{UiBatch, WHITE};
use crate::world::biome;
use crate::world::chunk::ChunkPos;
use crate::world::gen::SEA_LEVEL;
use crate::world::World;

pub struct Game {
    pub world: World,
    pub camera: Camera,
    pub time: f64,
    pub show_debug: bool,
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

impl Game {
    pub fn new(seed: u64, settings: &Settings) -> Self {
        let world = World::new(seed, None, settings.graphics.smooth_lighting);
        let spawn = find_spawn(&world);
        let mut camera = Camera::new(spawn + DVec3::new(0.0, 20.0, 0.0));
        camera.fov_deg = settings.graphics.fov as f32;
        camera.pitch = -0.3;
        log::info!("Новый мир: seed {seed}, точка появления {spawn:.1}");
        Self {
            world,
            camera,
            time: 0.0,
            show_debug: true,
            fps: 0.0,
            fps_acc: 0.0,
            fps_frames: 0,
            frame_ms: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32, input: &InputState, settings: &Settings, grabbed: bool) {
        self.time += dt as f64;
        self.fps_acc += dt;
        self.fps_frames += 1;
        if self.fps_acc >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_acc;
            self.frame_ms = self.fps_acc * 1000.0 / self.fps_frames as f32;
            self.fps_acc = 0.0;
            self.fps_frames = 0;
        }
        let c = &settings.controls;
        if input.pressed(c, Action::Debug) {
            self.show_debug = !self.show_debug;
        }
        if grabbed {
            let k = 0.0025 * c.mouse_sensitivity as f32;
            let dy = if c.invert_y { -input.mouse_delta.1 } else { input.mouse_delta.1 };
            self.camera.rotate(input.mouse_delta.0 as f32 * k, dy as f32 * k);
        }
        // Свободный полёт (этап 2).
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
        if input.down(c, Action::Jump) {
            dir.y += 1.0;
        }
        if input.down(c, Action::Sneak) {
            dir.y -= 1.0;
        }
        let speed = if input.down(c, Action::Sprint) { 40.0 } else { 10.0 };
        if dir.length_squared() > 0.0 {
            self.camera.pos += (dir.normalize() * speed * dt).as_dvec3();
        }
        if input.key_pressed(KeyCode::F4) {
            self.world.remesh_all();
        }

        let p = self.camera.pos;
        let center = ChunkPos::of_block(p.x.floor() as i32, p.z.floor() as i32);
        self.world.update(center, settings.graphics.render_distance as i32);
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
        let horizon = srgb([0.72, 0.84, 0.98]);
        Globals {
            cam_pos: [p.x as f32, p.y as f32, p.z as f32, self.time as f32],
            fog_color: [horizon[0], horizon[1], horizon[2], rd * 0.55],
            params: [rd * 0.95, 1.0, 0.0, 0.015],
            sun_dir: [sun.x, sun.y, sun.z, 0.3],
            sky_top: srgb([0.35, 0.55, 0.95]),
            sky_horizon: horizon,
            block_light: [1.0, 0.8, 0.55, settings.graphics.brightness as f32],
            sky_light: [1.0, 1.0, 1.0, 0.0],
            ..Default::default()
        }
    }

    pub fn draw_hud(&self, ui: &mut UiBatch, renderer: &Renderer) {
        let (w, h) = (ui.width, ui.height);
        let s = ui.scale;
        ui.rect(w / 2.0 - 9.0 * s, h / 2.0 - 1.0 * s, 18.0 * s, 2.0 * s, [255, 255, 255, 200]);
        ui.rect(w / 2.0 - 1.0 * s, h / 2.0 - 9.0 * s, 2.0 * s, 18.0 * s, [255, 255, 255, 200]);
        if !self.show_debug {
            return;
        }
        let p = self.camera.pos;
        let (bx, by, bz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let cp = ChunkPos::of_block(bx, bz);
        let stats = renderer.stats;
        let (gen_q, mesh_q) = self.world.pending_jobs();
        let (sky, blk) = self.world.light(bx, by, bz);
        let lines = [
            format!("PlusCraft 0.2 — {:.0} FPS ({:.1} мс)", self.fps, self.frame_ms),
            format!("XYZ: {:.2} / {:.2} / {:.2}", p.x, p.y, p.z),
            format!("Блок: {bx} {by} {bz}   Чанк: {} {} [{} {}]", cp.x, cp.z, bx & 15, bz & 15),
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
            format!("Seed: {}", self.world.seed),
            format!("GPU: {}", renderer.device_name()),
        ];
        let mut y = 6.0 * s;
        for l in &lines {
            let tw = ui.text_width(l, 0.85);
            ui.rect(4.0 * s, y - 1.0 * s, tw + 6.0 * s, ui.line_height(0.85), [0, 0, 0, 110]);
            ui.text(7.0 * s, y, l, 0.85, WHITE);
            y += ui.line_height(0.85);
        }
    }
}
