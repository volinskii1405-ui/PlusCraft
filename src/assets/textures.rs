//! Процедурный генератор текстур 16×16 (пиксель-арт).
//!
//! Текстуры генерируются детерминированно и сохраняются в
//! `assets/textures/blocks.png` (сетка 16 тайлов в ширину). При старте игра
//! грузит PNG — его можно перерисовать вручную; если файла нет, тайлы
//! генерируются заново в памяти.

pub const TILE: usize = 16;

macro_rules! tiles {
    ($($name:ident),* $(,)?) => {
        #[allow(non_camel_case_types, dead_code)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u16)]
        pub enum Tex { $($name),* }
        pub const TILE_NAMES: &[&str] = &[$(stringify!($name)),*];
        pub const ALL_TILES: &[Tex] = &[$(Tex::$name),*];
    };
}

tiles! {
    // --- блоки ---
    Stone, Cobble, MossyCobble, StoneBricks, Dirt, GrassTop, GrassSide, SnowGrassSide, Snow,
    Sand, SandstoneSide, SandstoneTop, Gravel, Bedrock, Clay,
    OakLogSide, OakLogTop, SpruceLogSide, SpruceLogTop, Planks, SprucePlanks, OakLeaves, SpruceLeaves,
    Glass, Ice, Water, Lava, CactusSide, CactusTop,
    CoalOre, CopperOre, IronOre, GoldOre, DiamondOre, ResoniteOre,
    Torch, BurntTorch, CraftingTop, CraftingSide, CraftingFront,
    FurnaceFront, FurnaceFrontLit, FurnaceSide, FurnaceTop,
    ChestSide, ChestFront, ChestTop, Rail, Support, SupportTop,
    TallGrass, FlowerRed, FlowerYellow, DeadBush, Cobweb, Wool,
    CopperBlock, IronBlock, GoldBlock, DiamondBlock, ResoniteLamp, Obsidian, Ladder,
    DeepStone, DeepCoalOre, DeepIronOre, DeepGoldOre, DeepDiamondOre,
    Destroy0, Destroy1, Destroy2, Destroy3, Destroy4, Destroy5, Destroy6, Destroy7, Destroy8, Destroy9,
    // --- предметы ---
    Stick, Coal, Charcoal, RawCopper, CopperIngot, RawIron, IronIngot, RawGold, GoldIngot,
    Diamond, ResoniteShard, Leather, RawMeat, CookedMeat, Apple, Bone, StringItem, Gel,
    Bucket, WaterBucket, Resonator, Flint, Bread, Wheat, Feather, RawMutton, CookedMutton,
    WoodPickaxe, StonePickaxe, CopperPickaxe, IronPickaxe, DiamondPickaxe,
    WoodAxe, StoneAxe, CopperAxe, IronAxe, DiamondAxe,
    WoodShovel, StoneShovel, CopperShovel, IronShovel, DiamondShovel,
    WoodSword, StoneSword, CopperSword, IronSword, DiamondSword,
    // --- мобы ---
    MobSkin, MobFur, MobEyes,
    // --- иконки интерфейса ---
    IconHeart, IconHeartHalf, IconHeartEmpty, IconFood, IconFoodHalf, IconFoodEmpty, IconBubble,
}

pub fn tile_count() -> usize {
    TILE_NAMES.len()
}

type Rgba = [u8; 4];

/// Детерминированный хэш для пикселей.
fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    h
}

fn rnd(x: i32, y: i32, seed: u32) -> f32 {
    (hash(x, y, seed) & 0xFFFF) as f32 / 65535.0
}

/// Тайлящийся value-noise с периодом 16.
fn vnoise(x: f32, y: f32, cell: f32, seed: u32) -> f32 {
    let n = (TILE as f32 / cell) as i32;
    let fx = x / cell;
    let fy = y / cell;
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let s = |a: f32| a * a * (3.0 - 2.0 * a);
    let g = |ix: i32, iy: i32| rnd(ix.rem_euclid(n), iy.rem_euclid(n), seed);
    let a = g(x0, y0) + (g(x0 + 1, y0) - g(x0, y0)) * s(tx);
    let b = g(x0, y0 + 1) + (g(x0 + 1, y0 + 1) - g(x0, y0 + 1)) * s(tx);
    a + (b - a) * s(ty)
}

fn shade(c: Rgba, k: f32) -> Rgba {
    [
        (c[0] as f32 * k).clamp(0.0, 255.0) as u8,
        (c[1] as f32 * k).clamp(0.0, 255.0) as u8,
        (c[2] as f32 * k).clamp(0.0, 255.0) as u8,
        c[3],
    ]
}

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
        (a[3] as f32 + (b[3] as f32 - a[3] as f32) * t) as u8,
    ]
}

const CLEAR: Rgba = [0, 0, 0, 0];

struct Canvas {
    px: Vec<Rgba>,
}

impl Canvas {
    fn new() -> Self {
        Self { px: vec![CLEAR; TILE * TILE] }
    }
    fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if (0..TILE as i32).contains(&x) && (0..TILE as i32).contains(&y) {
            self.px[y as usize * TILE + x as usize] = c;
        }
    }
    fn get(&self, x: i32, y: i32) -> Rgba {
        self.px[(y.rem_euclid(16)) as usize * TILE + (x.rem_euclid(16)) as usize]
    }
    fn fill<F: FnMut(i32, i32) -> Rgba>(&mut self, mut f: F) {
        for y in 0..16 {
            for x in 0..16 {
                let c = f(x, y);
                self.set(x, y, c);
            }
        }
    }
    /// Рисует ASCII-маску: символ -> цвет по палитре, '.' — пропуск.
    fn mask(&mut self, rows: &[&str], palette: &[(char, Rgba)]) {
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if let Some((_, c)) = palette.iter().find(|(k, _)| *k == ch) {
                    self.set(x as i32, y as i32, *c);
                }
            }
        }
    }
    fn into_bytes(self) -> Vec<u8> {
        self.px.into_iter().flatten().collect()
    }
}

fn noisy(base: Rgba, amount: f32, seed: u32) -> Canvas {
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let n = rnd(x, y, seed) * 2.0 - 1.0;
        let v = vnoise(x as f32, y as f32, 4.0, seed ^ 0x55) * 2.0 - 1.0;
        shade(base, 1.0 + n * amount + v * amount * 0.8)
    });
    c
}

fn stone_base(seed: u32, base: Rgba) -> Canvas {
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let v = vnoise(x as f32, y as f32, 4.0, seed) * 0.6 + vnoise(x as f32, y as f32, 2.0, seed + 7) * 0.4;
        let n = rnd(x, y, seed + 3) * 0.08;
        let k = 0.82 + v * 0.3 + n;
        let mut col = shade(base, k);
        if v < 0.28 {
            col = shade(col, 0.85);
        }
        col
    });
    c
}

fn ore(seed: u32, base_stone: Rgba, ore: Rgba, highlight: Rgba, clusters: usize) -> Canvas {
    let mut c = stone_base(seed ^ 0x1234, base_stone);
    let spots = [(3, 3), (10, 2), (6, 8), (12, 10), (2, 12), (9, 13), (13, 5)];
    for (i, &(sx, sy)) in spots.iter().take(clusters).enumerate() {
        let jx = (hash(i as i32, 0, seed) % 3) as i32 - 1;
        let jy = (hash(i as i32, 1, seed) % 3) as i32 - 1;
        let (cx, cy) = (sx + jx, sy + jy);
        let shape = [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0), (0, -1), (2, 1), (1, 2)];
        let n = 4 + (hash(i as i32, 2, seed) % 4) as usize;
        for (k, &(dx, dy)) in shape.iter().take(n).enumerate() {
            let col = if k == 0 { highlight } else if k % 3 == 2 { shade(ore, 0.75) } else { ore };
            c.set(cx + dx, cy + dy, col);
        }
    }
    c
}

fn planks(base: Rgba, seed: u32) -> Canvas {
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let board = y / 4;
        let offset = (hash(board, 0, seed) % 16) as i32;
        let grain = vnoise((x + offset) as f32 * 0.5, y as f32 * 4.0, 2.0, seed + board as u32) * 0.18;
        let mut col = shade(base, 0.9 + grain + rnd(x, y, seed) * 0.06);
        if y % 4 == 3 {
            col = shade(base, 0.62);
        }
        let seam = (offset + 5) % 16;
        if x == seam && y % 4 != 3 && board % 2 == 0 {
            col = shade(base, 0.7);
        }
        if x == (seam + 8) % 16 && y % 4 != 3 && board % 2 == 1 {
            col = shade(base, 0.7);
        }
        col
    });
    c
}

fn log_side(bark: Rgba, seed: u32) -> Canvas {
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let stripe = vnoise(x as f32 * 3.0, y as f32 * 0.4, 4.0, seed);
        let k = 0.75 + stripe * 0.35 + rnd(x, y, seed + 1) * 0.08;
        let mut col = shade(bark, k);
        if (x + (hash(0, y / 5, seed) % 3) as i32) % 5 == 0 {
            col = shade(col, 0.75);
        }
        col
    });
    c
}

fn log_top(bark: Rgba, inner: Rgba, seed: u32) -> Canvas {
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let dx = x as f32 - 7.5;
        let dy = y as f32 - 7.5;
        let d = (dx * dx + dy * dy).sqrt();
        if x == 0 || y == 0 || x == 15 || y == 15 {
            return shade(bark, 0.8 + rnd(x, y, seed) * 0.15);
        }
        let ring = ((d * 1.1) as i32) % 2 == 0;
        shade(inner, if ring { 0.95 } else { 0.8 } + rnd(x, y, seed) * 0.05)
    });
    c
}

fn leaves(base: Rgba, seed: u32, holes: f32) -> Canvas {
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let r = rnd(x, y, seed);
        if r < holes {
            return CLEAR;
        }
        let v = vnoise(x as f32, y as f32, 4.0, seed + 9);
        let mut col = shade(base, 0.7 + v * 0.45 + rnd(x, y, seed + 2) * 0.1);
        if rnd(x, y, seed + 5) > 0.92 {
            col = shade(col, 1.25);
        }
        col
    });
    c
}

fn metal_block(base: Rgba, seed: u32) -> Canvas {
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        let inner_edge = x == 1 || y == 1;
        let mut col = shade(base, 0.95 + rnd(x, y, seed) * 0.08);
        if edge {
            col = shade(base, 0.7);
        } else if inner_edge {
            col = shade(base, 1.2);
        } else if (x + y) % 11 == 0 && x > 3 && y > 3 {
            col = shade(base, 1.3);
        }
        col
    });
    c
}

fn cobble(base: Rgba, seed: u32) -> Canvas {
    // Ячейки Вороного (тайлящиеся) — «булыжники».
    let pts: Vec<(f32, f32, f32)> = (0..9)
        .map(|i| {
            (
                rnd(i, 0, seed) * 16.0,
                rnd(i, 1, seed) * 16.0,
                0.8 + rnd(i, 2, seed) * 0.35,
            )
        })
        .collect();
    let mut c = Canvas::new();
    c.fill(|x, y| {
        let mut d1 = f32::MAX;
        let mut d2 = f32::MAX;
        let mut k = 1.0;
        for &(px, py, pk) in &pts {
            for ox in [-16.0, 0.0, 16.0] {
                for oy in [-16.0, 0.0, 16.0] {
                    let dx = x as f32 + 0.5 - (px + ox);
                    let dy = y as f32 + 0.5 - (py + oy);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                        k = pk;
                    } else if d < d2 {
                        d2 = d;
                    }
                }
            }
        }
        if d2 - d1 < 1.0 {
            shade(base, 0.55)
        } else {
            shade(base, k + rnd(x, y, seed + 3) * 0.08 - (d1 / 12.0))
        }
    });
    c
}

fn crack(stage: u32) -> Canvas {
    // Трещины: случайные ломаные из центра, их число растёт со стадией.
    let mut c = Canvas::new();
    let lines = 2 + stage as i32;
    for l in 0..lines {
        let mut x = 7.5f32;
        let mut y = 7.5f32;
        let angle = rnd(l, 0, 777) * std::f32::consts::TAU;
        let len = 3.0 + stage as f32 * 0.7 + rnd(l, 1, 777) * 3.0;
        let (mut dx, mut dy) = (angle.cos(), angle.sin());
        let mut t = 0.0;
        while t < len {
            c.set(x as i32, y as i32, [20, 20, 20, 200]);
            x += dx;
            y += dy;
            t += 1.0;
            let jitter = (rnd(l, t as i32, 999) - 0.5) * 0.9;
            let (s, co) = jitter.sin_cos();
            let ndx = dx * co - dy * s;
            let ndy = dx * s + dy * co;
            dx = ndx;
            dy = ndy;
        }
    }
    c
}

/// Цвета материалов инструментов по тирам.
fn tier_colors(tier: usize) -> (Rgba, Rgba) {
    match tier {
        0 => ([168, 135, 84, 255], [110, 85, 50, 255]),   // дерево
        1 => ([140, 140, 140, 255], [90, 90, 90, 255]),   // камень
        2 => ([214, 125, 80, 255], [150, 80, 50, 255]),   // медь
        3 => ([225, 225, 225, 255], [150, 150, 160, 255]), // железо
        _ => ([110, 240, 225, 255], [40, 160, 150, 255]), // алмаз
    }
}

const STICK: Rgba = [137, 103, 57, 255];
const STICK_D: Rgba = [92, 66, 35, 255];

fn tool(kind: usize, tier: usize) -> Canvas {
    let (h, d) = tier_colors(tier);
    let mut c = Canvas::new();
    let pal = [('H', h), ('D', d), ('S', STICK), ('s', STICK_D), ('W', shade(h, 1.25))];
    let rows: &[&str] = match kind {
        // кирка
        0 => &[
            "................",
            "...DHHHHHHD.....",
            "..DHWWWWWHHD....",
            ".DHD....SsDHD...",
            ".HD....Ss..DH...",
            ".D....Ss....D...",
            "......Ss........",
            ".....Ss.........",
            "....Ss..........",
            "...Ss...........",
            "..Ss............",
            ".Ss.............",
            "Ss..............",
            "s...............",
            "................",
            "................",
        ],
        // топор
        1 => &[
            "................",
            ".....DHHD.......",
            "....DHWWHD......",
            "....HWWHHSs.....",
            "....DHHHSs......",
            ".....DDSs.......",
            "......Ss........",
            ".....Ss.........",
            "....Ss..........",
            "...Ss...........",
            "..Ss............",
            ".Ss.............",
            "Ss..............",
            "s...............",
            "................",
            "................",
        ],
        // лопата
        2 => &[
            "................",
            "..........DHD...",
            ".........DHWHD..",
            ".........HWWHD..",
            "........DHHHD...",
            ".........DSD....",
            "........Ss......",
            ".......Ss.......",
            "......Ss........",
            ".....Ss.........",
            "....Ss..........",
            "...Ss...........",
            "..Ss............",
            ".Ss.............",
            "Ss..............",
            "................",
        ],
        // меч
        _ => &[
            "................",
            "............DHD.",
            "...........DWHD.",
            "..........DWHD..",
            ".........DWHD...",
            "........DWHD....",
            ".......DWHD.....",
            "..D...DWHD......",
            "..DD.DWHD.......",
            "...DDWHD........",
            "....DsD.........",
            "...SsDD.........",
            "..Ss..D.........",
            ".Ss.............",
            "s...............",
            "................",
        ],
    };
    c.mask(rows, &pal);
    c
}

fn item_blob(rows: &[&str], pal: &[(char, Rgba)]) -> Canvas {
    let mut c = Canvas::new();
    c.mask(rows, pal);
    c
}

const INGOT: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "......DDDDDDD...",
    ".....DWWWWWHD...",
    "....DWHHHHHDD...",
    "...DWHHHHHDHD...",
    "..DHHHHHHDHD....",
    "..DDDDDDDHD.....",
    "..DHHHHHHD......",
    "..DDDDDDDD......",
    "................",
    "................",
    "................",
];

const RAW_ORE: [&str; 16] = [
    "................",
    "................",
    "................",
    "......DD........",
    "....DDHHD.......",
    "...DHWHHHDD.....",
    "...DHHHHWHHD....",
    "..DHHHDHHHHD....",
    "..DHWHHHHDHHD...",
    "..DHHHHHHHHHD...",
    "...DHHDHHWHD....",
    "....DDHHHHD.....",
    "......DDDD......",
    "................",
    "................",
    "................",
];

const GEM: [&str; 16] = [
    "................",
    "................",
    "................",
    ".....DDDDDD.....",
    "....DWWHHHHD....",
    "...DWHHHHHHHD...",
    "..DHHHHHHHHHHD..",
    "..DDDDDDDDDDDD..",
    "...DHHHHHHHHD...",
    "....DHHHHHHD....",
    ".....DHHHHD.....",
    "......DHHD......",
    ".......DD.......",
    "................",
    "................",
    "................",
];

fn palette(h: Rgba, d: Rgba) -> [(char, Rgba); 3] {
    [('H', h), ('D', d), ('W', shade(h, 1.3))]
}

pub fn generate(t: Tex) -> Vec<u8> {
    let stone: Rgba = [128, 128, 128, 255];
    let deep: Rgba = [78, 78, 88, 255];
    let dirt: Rgba = [134, 96, 67, 255];
    let grass: Rgba = [96, 160, 56, 255];
    let sand: Rgba = [219, 206, 160, 255];
    let oak: Rgba = [104, 82, 50, 255];
    let oak_in: Rgba = [176, 142, 88, 255];
    let spruce: Rgba = [66, 48, 30, 255];
    let spruce_in: Rgba = [130, 100, 62, 255];
    let plank: Rgba = [168, 134, 82, 255];
    let spruce_plank: Rgba = [118, 88, 54, 255];
    let seed = t as u32 * 7919 + 13;

    let c: Canvas = match t {
        Tex::Stone => stone_base(seed, stone),
        Tex::DeepStone => stone_base(seed, deep),
        Tex::Cobble => cobble(stone, seed),
        Tex::MossyCobble => {
            let mut c = cobble(stone, seed);
            for y in 0..16 {
                for x in 0..16 {
                    if vnoise(x as f32, y as f32, 4.0, seed + 1) > 0.55 {
                        let p = c.get(x, y);
                        c.set(x, y, mix(p, [70, 120, 50, 255], 0.65));
                    }
                }
            }
            c
        }
        Tex::StoneBricks => {
            let mut c = stone_base(seed, stone);
            for y in 0..16 {
                for x in 0..16 {
                    let row = y / 4;
                    let off = if row % 2 == 0 { 0 } else { 4 };
                    if y % 4 == 3 || (x + off) % 8 == 7 {
                        c.set(x, y, shade(stone, 0.55));
                    } else if y % 4 == 0 {
                        let p = c.get(x, y);
                        c.set(x, y, shade(p, 1.1));
                    }
                }
            }
            c
        }
        Tex::Dirt => {
            let mut c = noisy(dirt, 0.08, seed);
            for i in 0..14 {
                let x = (hash(i, 0, seed) % 16) as i32;
                let y = (hash(i, 1, seed) % 16) as i32;
                c.set(x, y, shade(dirt, if i % 2 == 0 { 0.7 } else { 1.2 }));
            }
            c
        }
        Tex::GrassTop => {
            let mut c = noisy(grass, 0.1, seed);
            for i in 0..20 {
                let x = (hash(i, 0, seed) % 16) as i32;
                let y = (hash(i, 1, seed) % 16) as i32;
                c.set(x, y, shade(grass, if i % 2 == 0 { 0.78 } else { 1.18 }));
            }
            c
        }
        Tex::GrassSide | Tex::SnowGrassSide => {
            let mut c = Canvas::new();
            let d = generate(Tex::Dirt);
            let top = if t == Tex::GrassSide { grass } else { [240, 245, 250, 255] };
            c.fill(|x, y| {
                let i = (y as usize * 16 + x as usize) * 4;
                let fringe = 3 + (hash(x, 0, seed) % 3) as i32;
                if y < fringe {
                    shade(top, 0.85 + rnd(x, y, seed) * 0.25)
                } else if y == fringe && hash(x, 9, seed) % 2 == 0 {
                    shade(top, 0.7)
                } else {
                    [d[i], d[i + 1], d[i + 2], 255]
                }
            });
            c
        }
        Tex::Snow => noisy([238, 244, 250, 255], 0.03, seed),
        Tex::Sand => noisy(sand, 0.05, seed),
        Tex::SandstoneTop => noisy([216, 200, 150, 255], 0.04, seed),
        Tex::SandstoneSide => {
            let mut c = noisy([214, 198, 148, 255], 0.04, seed);
            for x in 0..16 {
                c.set(x, 0, [190, 172, 120, 255]);
                c.set(x, 3, [228, 214, 168, 255]);
                c.set(x, 11, [196, 178, 128, 255]);
                c.set(x, 15, [180, 162, 110, 255]);
            }
            c
        }
        Tex::Gravel => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let v = vnoise(x as f32, y as f32, 2.0, seed);
                let r = rnd(x / 2, y / 2, seed);
                let base: Rgba = if r < 0.3 { [150, 140, 135, 255] } else if r < 0.6 { [110, 105, 100, 255] } else { [128, 122, 120, 255] };
                shade(base, 0.8 + v * 0.35)
            });
            c
        }
        Tex::Bedrock => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let v = vnoise(x as f32, y as f32, 2.0, seed);
                if v > 0.5 { [90, 90, 90, 255] } else { shade([45, 45, 45, 255], 0.7 + rnd(x, y, seed) * 0.6) }
            });
            c
        }
        Tex::Clay => noisy([158, 164, 178, 255], 0.04, seed),
        Tex::Obsidian => {
            let mut c = noisy([22, 16, 34, 255], 0.2, seed);
            for i in 0..8 {
                c.set((hash(i, 0, seed) % 16) as i32, (hash(i, 1, seed) % 16) as i32, [70, 50, 110, 255]);
            }
            c
        }
        Tex::OakLogSide => log_side(oak, seed),
        Tex::OakLogTop => log_top(oak, oak_in, seed),
        Tex::SpruceLogSide => log_side(spruce, seed),
        Tex::SpruceLogTop => log_top(spruce, spruce_in, seed),
        Tex::Planks => planks(plank, seed),
        Tex::SprucePlanks => planks(spruce_plank, seed),
        Tex::OakLeaves => leaves([70, 140, 45, 255], seed, 0.22),
        Tex::SpruceLeaves => leaves([45, 95, 60, 255], seed, 0.18),
        Tex::Glass => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                if x == 0 || y == 0 || x == 15 || y == 15 {
                    [220, 235, 240, 255]
                } else if (x == y + 3 || x == y + 4) && x < 10 && y > 1 {
                    [255, 255, 255, 140]
                } else if x == y - 6 && y > 9 {
                    [255, 255, 255, 120]
                } else {
                    [200, 225, 235, 30]
                }
            });
            c
        }
        Tex::Ice => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let v = vnoise(x as f32, y as f32, 8.0, seed);
                let mut col = shade([160, 200, 245, 175], 0.92 + v * 0.12);
                // Редкие светлые штрихи-трещины, не касающиеся краёв тайла.
                if (x == y + 2 && (3..9).contains(&x)) || (x + y == 20 && (9..14).contains(&x)) {
                    col = [225, 240, 255, 210];
                }
                col
            });
            c
        }
        Tex::Water => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let v = vnoise(x as f32, y as f32, 4.0, seed);
                let w = vnoise(x as f32 * 2.0, y as f32, 8.0, seed + 5);
                let mut col: Rgba = shade([46, 98, 205, 175], 0.85 + v * 0.25);
                if w > 0.68 {
                    col = [90, 150, 235, 185];
                }
                col
            });
            c
        }
        Tex::Lava => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let v = vnoise(x as f32, y as f32, 4.0, seed) * 0.6 + vnoise(x as f32, y as f32, 2.0, seed + 1) * 0.4;
                mix([200, 50, 10, 255], [255, 210, 60, 255], (v - 0.25) * 1.6)
            });
            c
        }
        Tex::CactusSide => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let base: Rgba = [70, 130, 50, 255];
                let mut col = shade(base, 0.85 + rnd(x, y, seed) * 0.15);
                if x % 4 == 0 {
                    col = shade(base, 0.65);
                }
                if x % 4 == 2 && y % 5 == 1 {
                    col = [230, 230, 190, 255];
                }
                col
            });
            c
        }
        Tex::CactusTop => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
                if d > 7.0 { [60, 115, 45, 255] } else if (d as i32) % 3 == 0 { [90, 155, 65, 255] } else { [75, 140, 55, 255] }
            });
            c
        }
        Tex::CoalOre => ore(seed, stone, [35, 35, 35, 255], [70, 70, 70, 255], 6),
        Tex::CopperOre => ore(seed, stone, [200, 110, 70, 255], [110, 200, 150, 255], 6),
        Tex::IronOre => ore(seed, stone, [214, 172, 140, 255], [240, 210, 190, 255], 5),
        Tex::GoldOre => ore(seed, stone, [245, 210, 50, 255], [255, 250, 180, 255], 5),
        Tex::DiamondOre => ore(seed, stone, [80, 225, 215, 255], [220, 255, 255, 255], 4),
        Tex::DeepCoalOre => ore(seed, deep, [25, 25, 25, 255], [60, 60, 60, 255], 6),
        Tex::DeepIronOre => ore(seed, deep, [214, 172, 140, 255], [240, 210, 190, 255], 5),
        Tex::DeepGoldOre => ore(seed, deep, [245, 210, 50, 255], [255, 250, 180, 255], 5),
        Tex::DeepDiamondOre => ore(seed, deep, [80, 225, 215, 255], [220, 255, 255, 255], 5),
        Tex::ResoniteOre => {
            let mut c = stone_base(seed, deep);
            // Кристаллы-«иглы».
            let crystals = [(3, 11, 5), (7, 13, 7), (11, 12, 4), (5, 6, 3), (12, 6, 4)];
            for &(x, y, h) in &crystals {
                for i in 0..h {
                    let col = if i == h - 1 { [240, 220, 255, 255] } else if i % 2 == 0 { [170, 90, 255, 255] } else { [120, 200, 255, 255] };
                    c.set(x, y - i, col);
                    if i < h - 2 {
                        c.set(x + 1, y - i, shade(col, 0.7));
                    }
                }
            }
            c
        }
        Tex::ResoniteLamp => {
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let edge = x == 0 || y == 0 || x == 15 || y == 15;
                let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
                if edge {
                    [60, 40, 90, 255]
                } else {
                    mix([250, 235, 255, 255], [140, 80, 230, 255], d / 8.0 + rnd(x, y, seed) * 0.15)
                }
            });
            c
        }
        Tex::Torch | Tex::BurntTorch => {
            let lit = t == Tex::Torch;
            item_blob(
                &[
                    "................",
                    "................",
                    "................",
                    ".......YY.......",
                    "......YWWY......",
                    "......OWWO......",
                    ".......OO.......",
                    ".......Ss.......",
                    ".......Ss.......",
                    ".......Ss.......",
                    ".......Ss.......",
                    ".......Ss.......",
                    ".......Ss.......",
                    ".......Ss.......",
                    ".......Ss.......",
                    ".......Ss.......",
                ],
                &[
                    ('Y', if lit { [255, 200, 60, 255] } else { [60, 55, 50, 255] }),
                    ('W', if lit { [255, 250, 200, 255] } else { [90, 85, 80, 255] }),
                    ('O', if lit { [240, 120, 30, 255] } else { [40, 36, 32, 255] }),
                    ('S', if lit { STICK } else { [80, 62, 40, 255] }),
                    ('s', STICK_D),
                ],
            )
        }
        Tex::CraftingTop => {
            let mut c = planks(plank, seed);
            for i in 0..16 {
                c.set(i, 0, shade(plank, 0.55));
                c.set(i, 15, shade(plank, 0.55));
                c.set(0, i, shade(plank, 0.55));
                c.set(15, i, shade(plank, 0.55));
                if (3..13).contains(&i) {
                    c.set(i, 5, shade(plank, 0.6));
                    c.set(i, 10, shade(plank, 0.6));
                    c.set(5, i, shade(plank, 0.6));
                    c.set(10, i, shade(plank, 0.6));
                }
            }
            c
        }
        Tex::CraftingSide | Tex::CraftingFront => {
            let mut c = planks(plank, seed);
            for i in 0..16 {
                c.set(i, 0, shade(plank, 0.55));
                c.set(i, 1, shade(plank, 0.75));
            }
            if t == Tex::CraftingFront {
                // пила и молоток
                for i in 3..12 {
                    c.set(i, 5, [170, 170, 175, 255]);
                    c.set(i, 6, [130, 130, 135, 255]);
                }
                c.set(2, 5, STICK);
                c.set(2, 6, STICK);
                for i in 8..14 {
                    c.set(11, i, STICK);
                }
                for i in 9..14 {
                    c.set(i, 8, [100, 100, 105, 255]);
                }
            } else {
                for i in 4..13 {
                    c.set(4, i, STICK_D);
                    c.set(11, i, STICK_D);
                }
            }
            c
        }
        Tex::FurnaceSide | Tex::FurnaceTop => {
            let mut c = stone_base(seed, [120, 120, 120, 255]);
            for i in 0..16 {
                c.set(i, 0, [80, 80, 80, 255]);
                c.set(i, 15, [80, 80, 80, 255]);
                c.set(0, i, [90, 90, 90, 255]);
                c.set(15, i, [90, 90, 90, 255]);
            }
            c
        }
        Tex::FurnaceFront | Tex::FurnaceFrontLit => {
            let mut c = generate_canvas(Tex::FurnaceSide);
            let lit = t == Tex::FurnaceFrontLit;
            for y in 8..14 {
                for x in 4..12 {
                    let col = if lit {
                        mix([255, 220, 80, 255], [200, 50, 10, 255], (13 - y) as f32 / 6.0 + rnd(x, y, seed) * 0.3)
                    } else {
                        [25, 25, 25, 255]
                    };
                    c.set(x, y, col);
                }
            }
            for x in 3..13 {
                c.set(x, 7, [70, 70, 70, 255]);
                c.set(x, 14, [70, 70, 70, 255]);
            }
            for x in 4..12 {
                c.set(x, 3, [60, 60, 60, 255]);
            }
            c
        }
        Tex::ChestSide | Tex::ChestFront | Tex::ChestTop => {
            let wood: Rgba = [160, 110, 50, 255];
            let mut c = planks(wood, seed);
            for i in 0..16 {
                c.set(i, 0, shade(wood, 0.5));
                c.set(i, 15, shade(wood, 0.5));
                c.set(0, i, shade(wood, 0.5));
                c.set(15, i, shade(wood, 0.5));
                if t != Tex::ChestTop {
                    c.set(i, 5, shade(wood, 0.45));
                }
            }
            if t == Tex::ChestFront {
                for y in 4..8 {
                    for x in 7..9 {
                        c.set(x, y, [200, 200, 205, 255]);
                    }
                }
                c.set(7, 7, [60, 60, 60, 255]);
            }
            c
        }
        Tex::Rail => {
            let mut c = Canvas::new();
            for y in 0..16 {
                if y % 4 == 1 || y % 4 == 2 {
                    for x in 1..15 {
                        c.set(x, y, shade([120, 90, 55, 255], 0.9 + rnd(x, y, seed) * 0.2));
                    }
                }
                for x in [3, 12] {
                    c.set(x, y, [170, 170, 178, 255]);
                    c.set(x + 1, y, [110, 110, 118, 255]);
                }
            }
            c
        }
        Tex::Support | Tex::SupportTop => {
            let wood: Rgba = [96, 70, 40, 255];
            let mut c = Canvas::new();
            c.fill(|x, y| {
                let frame = x <= 2 || x >= 13 || y <= 1 || y >= 14;
                let diag = (x - y).abs() <= 1 || (x + y - 15).abs() <= 1;
                if frame {
                    shade(wood, 0.9 + rnd(x, y, seed) * 0.2 - if x == 2 || x == 13 { 0.2 } else { 0.0 })
                } else if diag && t == Tex::Support {
                    shade(wood, 1.1 + rnd(x, y, seed) * 0.1)
                } else if t == Tex::SupportTop {
                    shade(wood, 0.75 + rnd(x, y, seed) * 0.1)
                } else {
                    CLEAR
                }
            });
            c
        }
        Tex::Ladder => {
            let mut c = Canvas::new();
            for y in 0..16 {
                c.set(2, y, STICK);
                c.set(3, y, STICK_D);
                c.set(12, y, STICK);
                c.set(13, y, STICK_D);
                if y % 4 == 2 {
                    for x in 2..14 {
                        c.set(x, y, STICK);
                    }
                }
            }
            c
        }
        Tex::TallGrass => {
            let mut c = Canvas::new();
            for i in 0..9 {
                let x = 1 + i * 2 - (i % 2);
                let h = 6 + (hash(i, 0, seed) % 8) as i32;
                let lean = (hash(i, 1, seed) % 3) as i32 - 1;
                for k in 0..h {
                    let xx = x + if k > h / 2 { lean } else { 0 };
                    c.set(xx, 15 - k, shade(grass, 0.7 + k as f32 / h as f32 * 0.5));
                }
            }
            c
        }
        Tex::FlowerRed | Tex::FlowerYellow => {
            let petal: Rgba = if t == Tex::FlowerRed { [220, 30, 40, 255] } else { [250, 220, 40, 255] };
            item_blob(
                &[
                    "................",
                    "................",
                    "................",
                    "................",
                    "......P.P.......",
                    ".....PPCPP......",
                    "......PPP.......",
                    ".......G........",
                    ".......G..L.....",
                    "...L...G.LL.....",
                    "...LL..GL.......",
                    "....L..G........",
                    ".......G........",
                    ".......G........",
                    ".......G........",
                    ".......G........",
                ],
                &[('P', petal), ('C', [255, 240, 120, 255]), ('G', [60, 120, 40, 255]), ('L', [80, 150, 50, 255])],
            )
        }
        Tex::DeadBush => item_blob(
            &[
                "................",
                "................",
                "...B......B.....",
                "....B....B...B..",
                "..B..B..B...B...",
                "...B..B.B..B....",
                "....B..BB.B.....",
                ".....B.BBB......",
                "......BBB.......",
                ".......B........",
                ".......B........",
                ".......B........",
                "......BB........",
                ".......B........",
                ".......B........",
                ".......B........",
            ],
            &[('B', [130, 95, 50, 255])],
        ),
        Tex::Cobweb => {
            let mut c = Canvas::new();
            let w: Rgba = [235, 235, 240, 200];
            for i in 0..16 {
                c.set(i, i, w);
                c.set(15 - i, i, w);
                c.set(7, i, w);
                c.set(i, 8, w);
            }
            for r in [3, 6] {
                for a in 0..32 {
                    let ang = a as f32 / 32.0 * std::f32::consts::TAU;
                    c.set((7.5 + ang.cos() * r as f32) as i32, (7.5 + ang.sin() * r as f32) as i32, w);
                }
            }
            c
        }
        Tex::Wool => noisy([235, 235, 232, 255], 0.05, seed),
        Tex::CopperBlock => metal_block([205, 115, 75, 255], seed),
        Tex::IronBlock => metal_block([215, 215, 220, 255], seed),
        Tex::GoldBlock => metal_block([245, 205, 60, 255], seed),
        Tex::DiamondBlock => metal_block([100, 230, 220, 255], seed),
        Tex::Destroy0 => crack(0),
        Tex::Destroy1 => crack(1),
        Tex::Destroy2 => crack(2),
        Tex::Destroy3 => crack(3),
        Tex::Destroy4 => crack(4),
        Tex::Destroy5 => crack(5),
        Tex::Destroy6 => crack(6),
        Tex::Destroy7 => crack(7),
        Tex::Destroy8 => crack(8),
        Tex::Destroy9 => crack(9),
        // --- предметы ---
        Tex::Stick => item_blob(
            &[
                "................",
                "................",
                "................",
                "...........Ss...",
                "..........Ss....",
                ".........Ss.....",
                "........Ss......",
                ".......Ss.......",
                "......Ss........",
                ".....Ss.........",
                "....Ss..........",
                "...Ss...........",
                "..Ss............",
                "................",
                "................",
                "................",
            ],
            &[('S', STICK), ('s', STICK_D)],
        ),
        Tex::Coal => item_blob(&RAW_ORE, &palette([45, 45, 48, 255], [20, 20, 22, 255])),
        Tex::Charcoal => item_blob(&RAW_ORE, &palette([70, 58, 45, 255], [30, 24, 18, 255])),
        Tex::RawCopper => item_blob(&RAW_ORE, &palette([200, 110, 70, 255], [120, 60, 35, 255])),
        Tex::RawIron => item_blob(&RAW_ORE, &palette([205, 165, 135, 255], [130, 95, 75, 255])),
        Tex::RawGold => item_blob(&RAW_ORE, &palette([240, 200, 60, 255], [160, 120, 20, 255])),
        Tex::Flint => item_blob(&RAW_ORE, &palette([70, 70, 75, 255], [35, 35, 38, 255])),
        Tex::CopperIngot => item_blob(&INGOT, &palette([215, 125, 80, 255], [130, 70, 40, 255])),
        Tex::IronIngot => item_blob(&INGOT, &palette([220, 220, 225, 255], [130, 130, 140, 255])),
        Tex::GoldIngot => item_blob(&INGOT, &palette([250, 215, 70, 255], [170, 125, 20, 255])),
        Tex::Diamond => item_blob(&GEM, &palette([100, 235, 225, 255], [30, 140, 130, 255])),
        Tex::ResoniteShard => item_blob(&GEM, &palette([175, 100, 255, 255], [80, 40, 150, 255])),
        Tex::Leather => item_blob(
            &[
                "................",
                "................",
                "...DDD....DDD...",
                "..DHHHDDDDHHHD..",
                "..DHHHHHHHHHHD..",
                "...DHHHHHHHHD...",
                "...DHHWHHHHHD...",
                "...DHHHHHHHHD...",
                "...DHHHHHWHHD...",
                "...DHHHHHHHHD...",
                "..DHHHHHHHHHHD..",
                "..DHHHDDDDHHHD..",
                "...DDD....DDD...",
                "................",
                "................",
                "................",
            ],
            &palette([150, 90, 50, 255], [90, 50, 25, 255]),
        ),
        Tex::RawMeat | Tex::CookedMeat | Tex::RawMutton | Tex::CookedMutton => {
            let (h, d) = match t {
                Tex::RawMeat => ([230, 110, 110, 255], [160, 50, 50, 255]),
                Tex::CookedMeat => ([170, 100, 55, 255], [100, 55, 25, 255]),
                Tex::RawMutton => ([220, 90, 90, 255], [140, 40, 40, 255]),
                _ => ([150, 85, 45, 255], [85, 45, 20, 255]),
            };
            item_blob(
                &[
                    "................",
                    "................",
                    "................",
                    "......DDDD......",
                    "....DDHHHHDD....",
                    "...DHHWWHHHHD...",
                    "..DHHWHHHHHHHD..",
                    "..DHHHHHHHHHHD..",
                    "..DHHHHHHHHHD...",
                    "...DHHHHHHDD....",
                    "....DDHHDD.BB...",
                    "......DD..BWB...",
                    "...........BB...",
                    "................",
                    "................",
                    "................",
                ],
                &[('H', h), ('D', d), ('W', shade(h, 1.25)), ('B', [235, 230, 210, 255])],
            )
        }
        Tex::Apple => item_blob(
            &[
                "................",
                "................",
                ".......S.L......",
                ".......SLL......",
                "....DDDSDDD.....",
                "...DHHHHHHHD....",
                "..DHWWHHHHHHD...",
                "..DHWHHHHHHHD...",
                "..DHHHHHHHHHD...",
                "..DHHHHHHHHHD...",
                "...DHHHHHHHD....",
                "....DHHDHHD.....",
                ".....DD.DD......",
                "................",
                "................",
                "................",
            ],
            &[('H', [215, 30, 40, 255]), ('D', [130, 15, 20, 255]), ('W', [255, 140, 140, 255]), ('S', STICK_D), ('L', [80, 160, 50, 255])],
        ),
        Tex::Bread => item_blob(
            &[
                "................",
                "................",
                "................",
                "................",
                "................",
                "......DDDDDD....",
                "....DDHWHWHHD...",
                "...DHWHHWHHHHD..",
                "..DHHHHHHHHHHD..",
                "..DHHHHHHHHHD...",
                "...DDHHHHHDD....",
                ".....DDDDD......",
                "................",
                "................",
                "................",
                "................",
            ],
            &palette([200, 150, 70, 255], [130, 85, 30, 255]),
        ),
        Tex::Wheat => item_blob(
            &[
                "................",
                "..........H.H...",
                ".........HHH....",
                "........HHHH.H..",
                ".......HDHH.H...",
                "......HDHH......",
                ".....SDH........",
                "....SS..........",
                "...SS...........",
                "..SS............",
                ".SS.............",
                "................",
                "................",
                "................",
                "................",
                "................",
            ],
            &[('H', [220, 190, 80, 255]), ('D', [170, 135, 50, 255]), ('S', [150, 160, 60, 255])],
        ),
        Tex::Feather => item_blob(
            &[
                "................",
                "...........WW...",
                "..........WWWW..",
                ".........WWGWW..",
                "........WWGWW...",
                ".......WWGWW....",
                "......WWGWW.....",
                ".....WWGWW......",
                "....WWGWW.......",
                "....WGWW........",
                "...WGW..........",
                "...G............",
                "..G.............",
                "................",
                "................",
                "................",
            ],
            &[('W', [240, 240, 240, 255]), ('G', [170, 170, 170, 255])],
        ),
        Tex::Bone => item_blob(
            &[
                "................",
                "............WW..",
                "...........WWWW.",
                "............GWW.",
                "...........WG...",
                "..........WG....",
                ".........WG.....",
                "........WG......",
                ".......WG.......",
                "......WG........",
                ".....WG.........",
                "....WG..........",
                ".WWGG...........",
                "WWWW............",
                ".WW.............",
                "................",
            ],
            &[('W', [240, 236, 220, 255]), ('G', [190, 185, 165, 255])],
        ),
        Tex::StringItem => item_blob(
            &[
                "................",
                "................",
                "...W............",
                "....W.....WW....",
                ".....W...W..W...",
                "......W.W....W..",
                ".......W......W.",
                "......W.W.....W.",
                ".....W...W...W..",
                "....W.....WWW...",
                "...W............",
                "..W.............",
                "................",
                "................",
                "................",
                "................",
            ],
            &[('W', [235, 235, 235, 255])],
        ),
        Tex::Gel => item_blob(
            &[
                "................",
                "................",
                "................",
                "................",
                ".....DDDDDD.....",
                "....DHHHHWWD....",
                "...DHHHHHHWWD...",
                "...DHHHHHHHHD...",
                "...DHHHHHHHHD...",
                "...DHHHHHHHHD...",
                "....DHHHHHHD....",
                ".....DDDDDD.....",
                "................",
                "................",
                "................",
                "................",
            ],
            &[('H', [110, 200, 90, 220]), ('D', [50, 120, 40, 255]), ('W', [210, 255, 200, 230])],
        ),
        Tex::Bucket | Tex::WaterBucket => {
            let water = t == Tex::WaterBucket;
            item_blob(
                &[
                    "................",
                    "................",
                    "................",
                    "....DDDDDDDD....",
                    "...DWWWWWWWWD...",
                    "...DHWWWWWWHD...",
                    "...DHHHHHHHHD...",
                    "....DHHHHHHD....",
                    "....DHHHHHHD....",
                    "....DHHHHHHD....",
                    ".....DHHHHD.....",
                    ".....DDDDDD.....",
                    "................",
                    "................",
                    "................",
                    "................",
                ],
                &[
                    ('H', [190, 190, 196, 255]),
                    ('D', [100, 100, 108, 255]),
                    ('W', if water { [60, 110, 220, 255] } else { [70, 70, 76, 255] }),
                ],
            )
        }
        Tex::Resonator => item_blob(
            &[
                "................",
                "......W.W.......",
                ".....W.W.W......",
                "......CCC.......",
                ".....CPPPC......",
                ".....CPWPC......",
                ".....CPPPC......",
                "......CCC.......",
                ".......M........",
                ".......M........",
                "......MMM.......",
                ".......S........",
                ".......S........",
                ".......S........",
                "......sss.......",
                "................",
            ],
            &[
                ('W', [200, 230, 255, 255]),
                ('C', [205, 120, 75, 255]),
                ('P', [170, 95, 255, 255]),
                ('M', [180, 180, 185, 255]),
                ('S', STICK),
                ('s', STICK_D),
            ],
        ),
        Tex::WoodPickaxe => tool(0, 0),
        Tex::StonePickaxe => tool(0, 1),
        Tex::CopperPickaxe => tool(0, 2),
        Tex::IronPickaxe => tool(0, 3),
        Tex::DiamondPickaxe => tool(0, 4),
        Tex::WoodAxe => tool(1, 0),
        Tex::StoneAxe => tool(1, 1),
        Tex::CopperAxe => tool(1, 2),
        Tex::IronAxe => tool(1, 3),
        Tex::DiamondAxe => tool(1, 4),
        Tex::WoodShovel => tool(2, 0),
        Tex::StoneShovel => tool(2, 1),
        Tex::CopperShovel => tool(2, 2),
        Tex::IronShovel => tool(2, 3),
        Tex::DiamondShovel => tool(2, 4),
        Tex::WoodSword => tool(3, 0),
        Tex::StoneSword => tool(3, 1),
        Tex::CopperSword => tool(3, 2),
        Tex::IronSword => tool(3, 3),
        Tex::DiamondSword => tool(3, 4),
        Tex::MobSkin => noisy([235, 235, 235, 255], 0.06, seed),
        Tex::MobFur => {
            let mut c = Canvas::new();
            c.fill(|x, y| shade([240, 240, 240, 255], 0.78 + vnoise(x as f32, y as f32 * 2.0, 2.0, seed) * 0.3));
            c
        }
        Tex::IconHeart | Tex::IconHeartHalf | Tex::IconHeartEmpty => {
            let rows = [
                "................",
                "................",
                "...KKKK..KKKK...",
                "..KRRWRKKRRRRK..",
                ".KRRWWRRRRRRRRK.",
                ".KRWRRRRRRRRRRK.",
                ".KRRRRRRRRRRRRK.",
                ".KRRRRRRRRRRRDK.",
                "..KRRRRRRRRRDK..",
                "...KRRRRRRRDK...",
                "....KRRRRRDK....",
                ".....KRRRDK.....",
                "......KRDK......",
                ".......KK.......",
                "................",
                "................",
            ];
            let mut c = Canvas::new();
            let red: Rgba = [220, 30, 40, 255];
            let empty: Rgba = [60, 20, 24, 255];
            for (y, row) in rows.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    let full = match t {
                        Tex::IconHeart => true,
                        Tex::IconHeartHalf => x < 8,
                        _ => false,
                    };
                    let col = match ch {
                        'K' => [20, 10, 10, 255],
                        'W' => if full { [255, 200, 200, 255] } else { empty },
                        'R' => if full { red } else { empty },
                        'D' => if full { [150, 15, 25, 255] } else { empty },
                        _ => continue,
                    };
                    c.set(x as i32, y as i32, col);
                }
            }
            c
        }
        Tex::IconFood | Tex::IconFoodHalf | Tex::IconFoodEmpty => {
            let rows = [
                "................",
                "................",
                "........KKKK....",
                ".......KMMMMK...",
                "......KMWMMMMK..",
                "......KMMMMMMK..",
                ".....KMMMMMMMK..",
                ".....KMMMMMMDK..",
                "....KKMMMMMDK...",
                "...KBKKMMMDK....",
                "..KBBK.KKKK.....",
                ".KBBK...........",
                "KBWK............",
                ".KK.............",
                "................",
                "................",
            ];
            let mut c = Canvas::new();
            for (y, row) in rows.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    let full = match t {
                        Tex::IconFood => true,
                        Tex::IconFoodHalf => x >= 7,
                        _ => false,
                    };
                    let dim: Rgba = [50, 35, 25, 255];
                    let col = match ch {
                        'K' => [20, 12, 8, 255],
                        'M' => if full { [180, 100, 50, 255] } else { dim },
                        'W' => if full { [230, 160, 110, 255] } else { dim },
                        'D' => if full { [120, 60, 25, 255] } else { dim },
                        'B' => if full { [235, 230, 215, 255] } else { dim },
                        _ => continue,
                    };
                    c.set(x as i32, y as i32, col);
                }
            }
            c
        }
        Tex::IconBubble => item_blob(
            &[
                "................",
                "................",
                "................",
                ".....KKKKKK.....",
                "....KBBBBBBK....",
                "...KBWWBBBBBK...",
                "...KBWBBBBBBK...",
                "...KBBBBBBBBK...",
                "...KBBBBBBBBK...",
                "...KBBBBBBBBK...",
                "...KBBBBBBBBK...",
                "....KBBBBBBK....",
                ".....KKKKKK.....",
                "................",
                "................",
                "................",
            ],
            &[('K', [30, 60, 120, 255]), ('B', [80, 150, 240, 255]), ('W', [230, 245, 255, 255])],
        ),
        Tex::MobEyes => {
            let mut c = Canvas::new();
            c.fill(|_, _| [255, 255, 255, 255]);
            c
        }
    };
    c.into_bytes()
}

fn generate_canvas(t: Tex) -> Canvas {
    let bytes = generate(t);
    let mut c = Canvas::new();
    for (i, px) in bytes.chunks_exact(4).enumerate() {
        c.px[i] = [px[0], px[1], px[2], px[3]];
    }
    c
}

/// Собирает все тайлы в атлас (сетка 16 тайлов в ширину).
pub fn build_atlas() -> (u32, u32, Vec<u8>) {
    let n = tile_count();
    let cols = 16usize;
    let rows = n.div_ceil(cols);
    let w = cols * TILE;
    let h = rows * TILE;
    let mut img = vec![0u8; w * h * 4];
    for (i, &t) in ALL_TILES.iter().enumerate() {
        let tile = generate(t);
        let ox = (i % cols) * TILE;
        let oy = (i / cols) * TILE;
        for y in 0..TILE {
            let dst = ((oy + y) * w + ox) * 4;
            img[dst..dst + TILE * 4].copy_from_slice(&tile[y * TILE * 4..(y + 1) * TILE * 4]);
        }
    }
    (w as u32, h as u32, img)
}

/// Режет атлас обратно на слои.
pub fn split_atlas(w: u32, h: u32, img: &[u8]) -> Option<Vec<Vec<u8>>> {
    let cols = (w as usize) / TILE;
    let rows = (h as usize) / TILE;
    if cols * rows < tile_count() {
        return None;
    }
    let mut layers = Vec::with_capacity(tile_count());
    for i in 0..tile_count() {
        let ox = (i % cols) * TILE;
        let oy = (i / cols) * TILE;
        let mut tile = Vec::with_capacity(TILE * TILE * 4);
        for y in 0..TILE {
            let src = ((oy + y) * w as usize + ox) * 4;
            tile.extend_from_slice(&img[src..src + TILE * 4]);
        }
        layers.push(tile);
    }
    Some(layers)
}
