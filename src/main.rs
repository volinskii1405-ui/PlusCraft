//! PlusCraft — воксельная песочница на Rust + Vulkan.

mod assets;
mod blockentity;
mod camera;
mod crafting;
mod entities;
mod game;
mod input;
mod inventory;
mod item;
mod paths;
mod physics;
mod player;
mod renderer;
mod save;
mod selftest;
mod settings;
mod sky;
mod ui;
mod world;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use assets::font::FontAtlas;
use game::Game;
use input::InputState;
use renderer::{FrameInput, Renderer, TextureLayers};
use settings::{Input, Settings};

pub struct Options {
    pub exit_after: Option<f32>,
    pub seed: Option<u64>,
    /// Камера для тестовых скриншотов: x, y, z, yaw°, pitch°.
    pub camera: Option<[f64; 5]>,
    pub selftest: bool,
    /// Консольные команды, выполняемые при старте (для тестов).
    pub commands: Vec<String>,
}

fn parse_args() -> Result<Option<Options>> {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = Options { exit_after: None, seed: None, camera: None, selftest: false, commands: Vec::new() };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--gen-assets" => {
                let path = paths::assets_dir().join("textures").join("blocks.png");
                assets::write_block_atlas(&path)?;
                println!("Атлас текстур записан: {}", path.display());
                if !paths::settings_file().exists() {
                    Settings::default().save()?;
                    println!("Настройки по умолчанию записаны: {}", paths::settings_file().display());
                }
                return Ok(None);
            }
            "--exit-after" => {
                i += 1;
                opts.exit_after = args.get(i).and_then(|s| s.parse().ok());
            }
            "--seed" => {
                i += 1;
                opts.seed = args.get(i).map(|s| seed_from_str(s));
            }
            "--camera" => {
                i += 1;
                let v: Vec<f64> = args.get(i).map(|s| s.split(',').filter_map(|p| p.parse().ok()).collect()).unwrap_or_default();
                if v.len() == 5 {
                    opts.camera = Some([v[0], v[1], v[2], v[3], v[4]]);
                }
            }
            "--selftest" => opts.selftest = true,
            "--cmd" => {
                i += 1;
                if let Some(c) = args.get(i) {
                    opts.commands.push(c.clone());
                }
            }
            "--help" | "-h" => {
                println!("pluscraft [--gen-assets] [--seed SEED] [--exit-after СЕКУНДЫ] [--selftest]");
                return Ok(None);
            }
            other => log::warn!("неизвестный аргумент: {other}"),
        }
        i += 1;
    }
    Ok(Some(opts))
}

/// Seed из строки: число — как есть, иначе хэш строки.
pub fn seed_from_str(s: &str) -> u64 {
    let s = s.trim();
    if let Ok(n) = s.parse::<i64>() {
        return n as u64;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn random_seed() -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345);
    let mut s = t;
    world::noise::splitmix(&mut s) % 1_000_000_000
}

struct State {
    // Рендерер объявлен раньше окна: поля уничтожаются по порядку, а Vulkan
    // surface должен умереть до окна.
    renderer: Renderer,
    window: Arc<Window>,
    font: FontAtlas,
    settings: Settings,
    input: InputState,
    game: Option<Game>,
    grabbed: bool,
    last_frame: Instant,
    start: Instant,
    exit_after: Option<f32>,
    selftest: Option<selftest::SelfTest>,
    /// Стартовые команды (--cmd) — выполняются, когда мир вокруг загрузится.
    pending_commands: Vec<String>,
}

impl State {
    fn new(event_loop: &ActiveEventLoop, opts: &Options) -> Result<Self> {
        let settings = Settings::load();
        let g = &settings.graphics;
        let mut attrs = Window::default_attributes()
            .with_title("PlusCraft")
            .with_inner_size(LogicalSize::new(g.window_width as f64, g.window_height as f64));
        if g.fullscreen {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }
        let window = Arc::new(event_loop.create_window(attrs).context("создание окна")?);

        let block_tex = assets::load_block_textures();
        let font = FontAtlas::load()?;
        let ui_tex = TextureLayers { size: assets::font::ATLAS_SIZE, layers: vec![font.pixels.clone()] };
        let renderer = Renderer::new(&window, settings.graphics.vsync, &block_tex, &ui_tex)?;
        log::info!("Рендерер готов: {}", renderer.device_name());

        let seed = opts.seed.unwrap_or_else(|| {
            if settings.gameplay.default_seed.trim().is_empty() {
                random_seed()
            } else {
                seed_from_str(&settings.gameplay.default_seed)
            }
        });
        let mut game = Game::new(seed, &settings, player::GameMode::Survival);
        if let Some(c) = opts.camera {
            game.player.pos = glam::DVec3::new(c[0], c[1], c[2]);
            game.player.prev_pos = game.player.pos;
            game.player.flying = true;
            game.player.mode = player::GameMode::Creative;
            game.camera.yaw = (c[3] as f32).to_radians();
            game.camera.pitch = (c[4] as f32).to_radians();
        }

        Ok(Self {
            renderer,
            window,
            font,
            settings,
            input: InputState::default(),
            game: Some(game),
            grabbed: false,
            last_frame: Instant::now(),
            start: Instant::now(),
            exit_after: opts.exit_after,
            selftest: opts.selftest.then(selftest::SelfTest::new),
            pending_commands: opts.commands.clone(),
        })
    }

    fn set_grab(&mut self, grab: bool) {
        if grab == self.grabbed {
            return;
        }
        if grab {
            let ok = self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined));
            if let Err(e) = ok {
                log::warn!("не удалось захватить курсор: {e}");
            }
            self.window.set_cursor_visible(false);
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            self.window.set_cursor_visible(true);
        }
        self.grabbed = grab;
    }

    fn frame(&mut self) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.25);
        self.last_frame = now;

        let console_open = self.game.as_ref().map(|g| g.console.open).unwrap_or(false);
        let screen_open = self.game.as_ref().map(|g| g.wants_cursor()).unwrap_or(false);
        if self.input.key_pressed(KeyCode::Escape) && !console_open && !screen_open {
            self.set_grab(false);
        }
        let dead = self.game.as_ref().map(|g| g.player.is_dead()).unwrap_or(false);
        if screen_open && self.grabbed {
            self.set_grab(false);
        }
        if self.input.mouse_pressed(0) && !self.grabbed && !dead && !screen_open {
            self.set_grab(true);
            // Клик, захвативший мышь, не должен ломать блок.
            self.input.consume_mouse(0);
        }

        let (w, h) = self.renderer.extent();
        let mut ui = ui::draw::UiBatch::new(&self.font, w as f32, h as f32, self.settings.graphics.ui_scale as f32);
        let Some(game) = self.game.as_mut() else { return Ok(()) };
        let dead = game.player.is_dead();
        game.update(dt, &self.input, &self.settings, self.grabbed && !dead && self.selftest.is_none());
        if let Some(t) = self.selftest.as_mut() {
            t.update(game, dt);
        }
        if !self.pending_commands.is_empty() && game.world.chunks.len() >= 25 {
            for c in std::mem::take(&mut self.pending_commands) {
                let r = game.run_command(&c);
                log::info!("{c}: {r}");
            }
        }
        game.upload_meshes(&mut self.renderer);
        game.draw_hud(&mut ui, &self.renderer);
        let geo = game.build_geometry(&self.settings);
        let mut want_grab_after_screen = false;
        if game.container.is_some() {
            let inv_key = self.input.pressed(&self.settings.controls, settings::Action::Inventory);
            if !ui::screens::update_and_draw(game, &mut ui, &self.input, inv_key) {
                want_grab_after_screen = true;
            }
        }
        let mut want_grab = None;
        if want_grab_after_screen {
            want_grab = Some(true);
        }
        if dead {
            want_grab = Some(false);
            ui.rect(0.0, 0.0, ui.width, ui.height, [120, 0, 0, 110]);
            let s = ui.scale;
            ui.text_centered(ui.width / 2.0, ui.height * 0.3, "Вы погибли!", 2.0, [255, 230, 230, 255]);
            let (bw, bh) = (260.0 * s, 40.0 * s);
            let (bx, by) = ((ui.width - bw) / 2.0, ui.height * 0.45);
            if ui::widgets::button(&mut ui, &self.input, bx, by, bw, bh, "Возродиться") {
                game.respawn();
                want_grab = Some(true);
            }
        }

        let input = FrameInput {
            camera_pos: game.camera.pos,
            view: game.camera.view_rotation(),
            fov_y: game.camera.fov_deg.to_radians(),
            globals: game.globals(&self.settings),
            max_distance: self.settings.graphics.render_distance as f32 * 16.0,
            entities: &geo.entities,
            entities_translucent: &geo.translucent,
            lines: &geo.lines,
            overlay: &geo.overlay,
            ui: &ui.verts,
            draw_world: true,
        };
        self.renderer.render(&input)?;
        drop(ui);
        if let Some(g) = want_grab {
            self.set_grab(g);
        }
        self.input.end_frame();

        // Ограничение FPS.
        let limit = self.settings.graphics.fps_limit;
        if limit > 0 {
            let target = Duration::from_secs_f64(1.0 / limit as f64);
            let spent = now.elapsed();
            if spent < target {
                std::thread::sleep(target - spent);
            }
        }
        Ok(())
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(g) = self.game.as_mut() {
            g.world.save_all();
        }
    }
}

struct App {
    opts: Options,
    state: Option<State>,
    error: Option<anyhow::Error>,
    selftest_ok: Option<bool>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match State::new(event_loop, &self.opts) {
            Ok(s) => self.state = Some(s),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.renderer.resize(size.width, size.height),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if !event.repeat {
                                state.input.on_press(Input::Key(code));
                            }
                        }
                        ElementState::Released => state.input.on_release(Input::Key(code)),
                    }
                }
                if event.state == ElementState::Pressed {
                    if let Some(text) = &event.text {
                        state.input.typed.extend(text.chars().filter(|c| !c.is_control()));
                    }
                }
            }
            WindowEvent::MouseInput { state: s, button, .. } => {
                let b = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Back => 3,
                    MouseButton::Forward => 4,
                    MouseButton::Other(n) => (n.min(250) + 5) as u8,
                };
                match s {
                    ElementState::Pressed => state.input.on_press(Input::Mouse(b)),
                    ElementState::Released => state.input.on_release(Input::Mouse(b)),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                state.input.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 40.0) as f32,
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.input.mouse_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::Focused(false) => {
                state.input.clear_all();
                state.set_grab(false);
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = state.frame() {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if let Some(t) = &state.selftest {
                    if t.done {
                        self.selftest_ok = Some(t.all_passed());
                        event_loop.exit();
                        return;
                    }
                }
                if let Some(limit) = state.exit_after {
                    if state.start.elapsed().as_secs_f32() > limit {
                        log::info!("--exit-after: выход");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        let Some(state) = self.state.as_mut() else { return };
        if let DeviceEvent::MouseMotion { delta } = event {
            if state.grabbed {
                state.input.mouse_delta.0 += delta.0;
                state.input.mouse_delta.1 += delta.1;
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }
}

fn run() -> Result<()> {
    let Some(opts) = parse_args()? else { return Ok(()) };
    let event_loop = EventLoop::new().context("создание цикла событий")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { opts, state: None, error: None, selftest_ok: None };
    event_loop.run_app(&mut app)?;
    // Явно уничтожаем состояние до выхода из цикла: Vulkan-объекты
    // освобождаются раньше окна, мир сохраняется.
    drop(app.state.take());
    if let Some(e) = app.error {
        return Err(e);
    }
    if app.selftest_ok == Some(false) {
        anyhow::bail!("самопроверка не пройдена");
    }
    Ok(())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run() {
        log::error!("Фатальная ошибка: {e:#}");
        eprintln!("PlusCraft завершился с ошибкой: {e:#}");
        std::process::exit(1);
    }
}
