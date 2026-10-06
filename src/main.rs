//! PlusCraft — воксельная песочница на Rust + Vulkan.
//!
//! Этап 1: окно, Vulkan, swapchain, один чанк и свободная камера.

mod assets;
mod camera;
mod item;
mod paths;
mod renderer;
mod settings;
mod ui;
mod world;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use glam::{DVec3, Vec3};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use assets::font::FontAtlas;
use camera::Camera;
use renderer::{FrameInput, Globals, Renderer, TextureLayers};
use settings::Settings;
use world::block::{id, make};
use world::chunk::{ChunkData, ChunkPos, CHUNK_W};
use world::neighborhood::Neighborhood;
use world::noise::Simplex;

struct Options {
    exit_after: Option<f32>,
}

fn parse_args() -> Result<Option<Options>> {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = Options { exit_after: None };
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
            "--help" | "-h" => {
                println!("pluscraft [--gen-assets] [--exit-after СЕКУНДЫ]");
                return Ok(None);
            }
            other => log::warn!("неизвестный аргумент: {other}"),
        }
        i += 1;
    }
    Ok(Some(opts))
}

/// Демонстрационный чанк этапа 1: холмы, пруд, дерево, факел.
fn demo_chunk(seed: u64) -> ChunkData {
    let noise = Simplex::new(seed);
    let mut c = ChunkData::empty();
    for z in 0..CHUNK_W {
        for x in 0..CHUNK_W {
            let h = (62.0 + noise.fbm2(x as f32 * 0.07, z as f32 * 0.07, 3, 2.0, 0.5) * 6.0) as usize;
            for y in 0..=h {
                let b = if y == 0 {
                    id::BEDROCK
                } else if y < h - 3 {
                    if (x + y * 3 + z * 7) % 41 == 0 { id::COAL_ORE } else { id::STONE }
                } else if y < h {
                    id::DIRT
                } else if h < 61 {
                    id::SAND
                } else {
                    id::GRASS
                };
                c.set(x, y, z, make(b, 0));
            }
            for y in h + 1..=60 {
                c.set(x, y, z, make(id::WATER, 0));
            }
            if h > 61 && (x * 7 + z * 13) % 11 == 0 {
                c.set(x, h + 1, z, make(id::TALL_GRASS, 0));
            }
        }
    }
    // Дерево в центре.
    let (tx, tz) = (8, 8);
    let ground = (0..256).rev().find(|&y| c.get(tx, y, tz) != 0).unwrap_or(64);
    for y in ground + 1..ground + 6 {
        c.set(tx, y, tz, make(id::OAK_LOG, 0));
    }
    for dy in 3..7i32 {
        let r = if dy >= 5 { 1 } else { 2 };
        for dz in -r..=r {
            for dx in -r..=r {
                let (x, y, z) = (tx as i32 + dx, ground as i32 + dy, tz as i32 + dz);
                if c.get(x as usize, y as usize, z as usize) == 0 {
                    c.set(x as usize, y as usize, z as usize, make(id::OAK_LEAVES, 0));
                }
            }
        }
    }
    let g2 = (0..256).rev().find(|&y| c.get(3, y, 12) != 0).unwrap_or(64);
    c.set(3, g2 + 1, 12, make(id::TORCH, 0));
    c.set(12, g2 + 1, 3, make(id::GLASS, 0));
    c.set(13, g2 + 1, 3, make(id::RESONITE_ORE, 0));
    c
}

struct State {
    // Рендерер объявлен раньше окна: поля уничтожаются по порядку, а Vulkan
    // surface должен умереть до окна.
    renderer: Renderer,
    window: Arc<Window>,
    font: FontAtlas,
    camera: Camera,
    keys: std::collections::HashSet<KeyCode>,
    grabbed: bool,
    last_frame: Instant,
    start: Instant,
    fps_timer: f32,
    fps_frames: u32,
    fps: f32,
    exit_after: Option<f32>,
    settings: Settings,
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
        let mut renderer = Renderer::new(&window, settings.graphics.vsync, &block_tex, &ui_tex)?;
        log::info!("Рендерер готов: {}", renderer.device_name());

        // Один чанк для этапа 1.
        let data = Arc::new(demo_chunk(42));
        let nb = Neighborhood {
            center: ChunkPos::new(0, 0),
            chunks: [[None, None, None], [None, Some(data), None], [None, None, None]],
        };
        let t = Instant::now();
        let mesh = world::mesher::build(&nb, true);
        log::info!(
            "Меш чанка: {} непрозр. + {} прозр. вершин за {:.1} мс",
            mesh.opaque.len(),
            mesh.translucent.len(),
            t.elapsed().as_secs_f32() * 1000.0
        );
        renderer.upload_chunk_mesh(ChunkPos::new(0, 0), &mesh.opaque, &mesh.translucent, mesh.min_y, mesh.max_y)?;

        let mut camera = Camera::new(DVec3::new(-6.0, 72.0, 22.0));
        camera.yaw = -0.6;
        camera.pitch = -0.35;
        camera.fov_deg = settings.graphics.fov as f32;

        Ok(Self {
            window,
            renderer,
            font,
            camera,
            keys: Default::default(),
            grabbed: false,
            last_frame: Instant::now(),
            start: Instant::now(),
            fps_timer: 0.0,
            fps_frames: 0,
            fps: 0.0,
            exit_after: opts.exit_after,
            settings,
        })
    }

    fn set_grab(&mut self, grab: bool) {
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

    fn update(&mut self, dt: f32) {
        let mut dir = Vec3::ZERO;
        let k = |c: KeyCode| self.keys.contains(&c);
        if k(KeyCode::KeyW) {
            dir += self.camera.forward_flat();
        }
        if k(KeyCode::KeyS) {
            dir -= self.camera.forward_flat();
        }
        if k(KeyCode::KeyD) {
            dir += self.camera.right_flat();
        }
        if k(KeyCode::KeyA) {
            dir -= self.camera.right_flat();
        }
        if k(KeyCode::Space) {
            dir.y += 1.0;
        }
        if k(KeyCode::ShiftLeft) {
            dir.y -= 1.0;
        }
        let speed = if k(KeyCode::ControlLeft) { 25.0 } else { 8.0 };
        if dir.length_squared() > 0.0 {
            self.camera.pos += (dir.normalize() * speed * dt).as_dvec3();
        }
    }

    fn render(&mut self) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.fps_timer += dt;
        self.fps_frames += 1;
        if self.fps_timer >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_timer;
            self.fps_timer = 0.0;
            self.fps_frames = 0;
        }
        self.update(dt);

        let t = self.start.elapsed().as_secs_f32();
        let (w, h) = self.renderer.extent();
        let mut ui = ui::draw::UiBatch::new(&self.font, w as f32, h as f32, 1.0);
        let p = self.camera.pos;
        ui.text_shadow(8.0, 8.0, &format!("PlusCraft — этап 1   FPS: {:.0}", self.fps), 1.0, ui::draw::WHITE);
        ui.text_shadow(8.0, 30.0, &format!("XYZ: {:.1} / {:.1} / {:.1}", p.x, p.y, p.z), 1.0, ui::draw::WHITE);
        ui.text_shadow(8.0, 52.0, &format!("GPU: {}", self.renderer.device_name()), 1.0, [200, 200, 200, 255]);
        ui.text_shadow(
            8.0,
            74.0,
            "WASD — полёт, Пробел/Shift — вверх/вниз, Ctrl — быстро, ЛКМ — захват мыши, Esc — отпустить",
            0.8,
            [220, 220, 160, 255],
        );
        // Прицел.
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        ui.rect(cx - 9.0, cy - 1.0, 18.0, 2.0, [255, 255, 255, 200]);
        ui.rect(cx - 1.0, cy - 9.0, 2.0, 18.0, [255, 255, 255, 200]);

        let srgb = |c: [f32; 3]| [c[0].powf(2.2), c[1].powf(2.2), c[2].powf(2.2), 1.0];
        let sun = Vec3::new(0.4, 0.75, -0.5).normalize();
        let horizon = srgb([0.72, 0.84, 0.98]);
        let globals = Globals {
            cam_pos: [p.x as f32, p.y as f32, p.z as f32, t],
            fog_color: [horizon[0], horizon[1], horizon[2], 60.0],
            params: [120.0, 1.0, 0.0, 0.02],
            sun_dir: [sun.x, sun.y, sun.z, 0.3],
            sky_top: srgb([0.35, 0.55, 0.95]),
            sky_horizon: horizon,
            block_light: [1.0, 0.82, 0.6, 0.3],
            sky_light: [1.0, 1.0, 1.0, 0.0],
            ..Default::default()
        };
        let input = FrameInput {
            camera_pos: self.camera.pos,
            view: self.camera.view_rotation(),
            fov_y: self.camera.fov_deg.to_radians(),
            globals,
            max_distance: 200.0,
            entities: &[],
            entities_translucent: &[],
            lines: &[],
            ui: &ui.verts,
            draw_world: true,
        };
        self.renderer.render(&input)?;
        Ok(())
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
                            state.keys.insert(code);
                            if code == KeyCode::Escape {
                                state.set_grab(false);
                            }
                        }
                        ElementState::Released => {
                            state.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                if !state.grabbed {
                    state.set_grab(true);
                }
            }
            WindowEvent::Focused(false) => {
                state.keys.clear();
                state.set_grab(false);
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = state.render() {
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
                let c = &state.settings.controls;
                let k = 0.0025 * c.mouse_sensitivity as f32;
                let dy = if c.invert_y { -delta.1 } else { delta.1 };
                state.camera.rotate(delta.0 as f32 * k, dy as f32 * k);
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
    // Явно уничтожаем рендерер до выхода, чтобы Vulkan-объекты освободились
    // раньше окна.
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
        std::thread::sleep(Duration::from_millis(10));
        std::process::exit(1);
    }
}
