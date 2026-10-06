//! PlusCraft — воксельная песочница на Rust + Vulkan.

mod assets;
mod camera;
mod game;
mod input;
mod item;
mod paths;
mod renderer;
mod save;
mod settings;
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

struct Options {
    exit_after: Option<f32>,
    seed: Option<u64>,
}

fn parse_args() -> Result<Option<Options>> {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = Options { exit_after: None, seed: None };
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
            "--help" | "-h" => {
                println!("pluscraft [--gen-assets] [--seed SEED] [--exit-after СЕКУНДЫ]");
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
        let game = Game::new(seed, &settings);

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

        if self.input.key_pressed(KeyCode::Escape) {
            self.set_grab(false);
        }
        if self.input.mouse_pressed(0) && !self.grabbed {
            self.set_grab(true);
        }

        let (w, h) = self.renderer.extent();
        let mut ui = ui::draw::UiBatch::new(&self.font, w as f32, h as f32, self.settings.graphics.ui_scale as f32);
        let Some(game) = self.game.as_mut() else { return Ok(()) };
        game.update(dt, &self.input, &self.settings, self.grabbed);
        game.upload_meshes(&mut self.renderer);
        game.draw_hud(&mut ui, &self.renderer);

        let input = FrameInput {
            camera_pos: game.camera.pos,
            view: game.camera.view_rotation(),
            fov_y: game.camera.fov_deg.to_radians(),
            globals: game.globals(&self.settings),
            max_distance: self.settings.graphics.render_distance as f32 * 16.0,
            entities: &[],
            entities_translucent: &[],
            lines: &[],
            ui: &ui.verts,
            draw_world: true,
        };
        self.renderer.render(&input)?;
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
    let mut app = App { opts, state: None, error: None };
    event_loop.run_app(&mut app)?;
    // Явно уничтожаем состояние до выхода из цикла: Vulkan-объекты
    // освобождаются раньше окна, мир сохраняется.
    drop(app.state.take());
    if let Some(e) = app.error {
        return Err(e);
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
