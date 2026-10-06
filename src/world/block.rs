//! Реестр блоков. ID блоков фиксированы (используются в сохранениях).
//!
//! Значение ячейки чанка — `u16`: младший байт — ID блока, старший — метаданные
//! (уровень жидкости, топливо факела, направление печи и т.п.).

use crate::assets::textures::Tex;

pub type BlockId = u8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Air,
    Cube,
    Cross,
    Torch,
    Rail,
    Liquid,
    Ladder,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Opaque,
    /// Альфа-тест в непрозрачном проходе (листва, растения).
    Cutout,
    /// Смешивание в отдельном проходе (вода, стекло, лёд).
    Translucent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ToolKind {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Sword,
}

/// Что выпадает при разрушении.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drop {
    /// Сам блок.
    SelfBlock,
    Nothing,
    /// Предмет: (ID предмета, мин. количество, макс. количество).
    Item(u16, u8, u8),
    /// С шансом (в процентах) — предмет.
    Chance(u16, u8),
}

#[derive(Clone, Copy, Debug)]
pub struct BlockDef {
    pub name: &'static str,
    pub key: &'static str,
    pub shape: Shape,
    pub layer: Layer,
    pub tex_top: Tex,
    pub tex_bottom: Tex,
    pub tex_side: Tex,
    /// Отдельная текстура «лицевой» стороны (печь, верстак, сундук).
    pub tex_front: Option<Tex>,
    pub solid: bool,
    pub opaque: bool,
    /// Яркость собственного свечения 0..15.
    pub light: u8,
    /// Дополнительное ослабление света при прохождении (вода, листва).
    pub absorb: u8,
    /// Базовое время разрушения рукой (с); < 0 — неразрушим.
    pub hardness: f32,
    pub tool: ToolKind,
    /// Минимальный тир инструмента для выпадения: 0 — рука, 1 — дерево,
    /// 2 — камень, 3 — медь, 4 — железо, 5 — алмаз.
    pub min_tier: u8,
    pub drop: Drop,
    pub replaceable: bool,
    /// Падает, если снизу пусто (песок, гравий).
    pub gravity: bool,
    pub emissive: bool,
    pub waving: bool,
    pub tint: bool,
    /// Порода, способная обрушиться без крепей (уникальная механика).
    pub unstable: bool,
}

const BASE: BlockDef = BlockDef {
    name: "?",
    key: "unknown",
    shape: Shape::Cube,
    layer: Layer::Opaque,
    tex_top: Tex::Stone,
    tex_bottom: Tex::Stone,
    tex_side: Tex::Stone,
    tex_front: None,
    solid: true,
    opaque: true,
    light: 0,
    absorb: 0,
    hardness: 1.0,
    tool: ToolKind::None,
    min_tier: 0,
    drop: Drop::SelfBlock,
    replaceable: false,
    gravity: false,
    emissive: false,
    waving: false,
    tint: false,
    unstable: false,
};

const fn all(t: Tex) -> (Tex, Tex, Tex) {
    (t, t, t)
}

macro_rules! block {
    ($name:expr, $key:expr, $tex:expr, { $($field:ident : $val:expr),* $(,)? }) => {{
        let (t, b, s) = $tex;
        BlockDef { name: $name, key: $key, tex_top: t, tex_bottom: b, tex_side: s, $($field: $val,)* ..BASE }
    }};
}

pub mod id {
    use super::BlockId;
    pub const AIR: BlockId = 0;
    pub const STONE: BlockId = 1;
    pub const DIRT: BlockId = 2;
    pub const GRASS: BlockId = 3;
    pub const SAND: BlockId = 4;
    pub const GRAVEL: BlockId = 5;
    pub const BEDROCK: BlockId = 6;
    pub const WATER: BlockId = 7;
    pub const LAVA: BlockId = 8;
    pub const OAK_LOG: BlockId = 9;
    pub const OAK_LEAVES: BlockId = 10;
    pub const PLANKS: BlockId = 11;
    pub const GLASS: BlockId = 12;
    pub const COBBLE: BlockId = 13;
    pub const SANDSTONE: BlockId = 14;
    pub const SNOW: BlockId = 15;
    pub const SNOW_GRASS: BlockId = 16;
    pub const CACTUS: BlockId = 17;
    pub const COAL_ORE: BlockId = 18;
    pub const COPPER_ORE: BlockId = 19;
    pub const IRON_ORE: BlockId = 20;
    pub const GOLD_ORE: BlockId = 21;
    pub const DIAMOND_ORE: BlockId = 22;
    pub const RESONITE_ORE: BlockId = 23;
    pub const TORCH: BlockId = 24;
    pub const BURNT_TORCH: BlockId = 25;
    pub const CRAFTING_TABLE: BlockId = 26;
    pub const FURNACE: BlockId = 27;
    pub const FURNACE_LIT: BlockId = 28;
    pub const CHEST: BlockId = 29;
    pub const RAIL: BlockId = 30;
    pub const SUPPORT: BlockId = 31;
    pub const STONE_BRICKS: BlockId = 32;
    pub const SPRUCE_LOG: BlockId = 33;
    pub const SPRUCE_LEAVES: BlockId = 34;
    pub const TALL_GRASS: BlockId = 35;
    pub const FLOWER_RED: BlockId = 36;
    pub const FLOWER_YELLOW: BlockId = 37;
    pub const DEAD_BUSH: BlockId = 38;
    pub const ICE: BlockId = 39;
    pub const CLAY: BlockId = 40;
    pub const COPPER_BLOCK: BlockId = 41;
    pub const IRON_BLOCK: BlockId = 42;
    pub const GOLD_BLOCK: BlockId = 43;
    pub const DIAMOND_BLOCK: BlockId = 44;
    pub const COBWEB: BlockId = 45;
    pub const MOSSY_COBBLE: BlockId = 46;
    pub const WOOL: BlockId = 47;
    pub const RESONITE_LAMP: BlockId = 48;
    pub const OBSIDIAN: BlockId = 49;
    pub const LADDER: BlockId = 50;
    pub const SPRUCE_PLANKS: BlockId = 51;
    pub const DEEP_STONE: BlockId = 52;
    pub const DEEP_COAL_ORE: BlockId = 53;
    pub const DEEP_IRON_ORE: BlockId = 54;
    pub const DEEP_GOLD_ORE: BlockId = 55;
    pub const DEEP_DIAMOND_ORE: BlockId = 56;
    pub const COUNT: usize = 57;
}

use crate::item::id as it;

pub static BLOCKS: [BlockDef; id::COUNT] = [
    // 0
    BlockDef {
        name: "Воздух",
        key: "air",
        shape: Shape::Air,
        solid: false,
        opaque: false,
        replaceable: true,
        hardness: 0.0,
        drop: Drop::Nothing,
        ..BASE
    },
    block!("Камень", "stone", all(Tex::Stone), { hardness: 1.5, tool: ToolKind::Pickaxe, min_tier: 1, drop: Drop::Item(it::COBBLE, 1, 1), unstable: true }),
    block!("Земля", "dirt", all(Tex::Dirt), { hardness: 0.5, tool: ToolKind::Shovel }),
    block!("Трава", "grass", (Tex::GrassTop, Tex::Dirt, Tex::GrassSide), { hardness: 0.6, tool: ToolKind::Shovel, drop: Drop::Item(it::DIRT, 1, 1), tint: true }),
    block!("Песок", "sand", all(Tex::Sand), { hardness: 0.5, tool: ToolKind::Shovel, gravity: true }),
    block!("Гравий", "gravel", all(Tex::Gravel), { hardness: 0.6, tool: ToolKind::Shovel, gravity: true, drop: Drop::Chance(it::FLINT, 15) }),
    block!("Коренная порода", "bedrock", all(Tex::Bedrock), { hardness: -1.0, drop: Drop::Nothing }),
    BlockDef {
        name: "Вода",
        key: "water",
        shape: Shape::Liquid,
        layer: Layer::Translucent,
        tex_top: Tex::Water,
        tex_bottom: Tex::Water,
        tex_side: Tex::Water,
        solid: false,
        opaque: false,
        absorb: 2,
        hardness: -1.0,
        drop: Drop::Nothing,
        replaceable: true,
        ..BASE
    },
    BlockDef {
        name: "Лава",
        key: "lava",
        shape: Shape::Liquid,
        layer: Layer::Opaque,
        tex_top: Tex::Lava,
        tex_bottom: Tex::Lava,
        tex_side: Tex::Lava,
        solid: false,
        opaque: false,
        light: 15,
        absorb: 1,
        hardness: -1.0,
        drop: Drop::Nothing,
        replaceable: true,
        emissive: true,
        ..BASE
    },
    block!("Дубовое бревно", "oak_log", (Tex::OakLogTop, Tex::OakLogTop, Tex::OakLogSide), { hardness: 2.0, tool: ToolKind::Axe }),
    block!("Дубовая листва", "oak_leaves", all(Tex::OakLeaves), { layer: Layer::Cutout, opaque: false, absorb: 1, hardness: 0.3, drop: Drop::Chance(it::APPLE, 6), waving: true, tint: true }),
    block!("Доски", "planks", all(Tex::Planks), { hardness: 1.5, tool: ToolKind::Axe }),
    block!("Стекло", "glass", all(Tex::Glass), { layer: Layer::Translucent, opaque: false, hardness: 0.4, drop: Drop::Nothing }),
    block!("Булыжник", "cobblestone", all(Tex::Cobble), { hardness: 2.0, tool: ToolKind::Pickaxe, min_tier: 1, unstable: true }),
    block!("Песчаник", "sandstone", (Tex::SandstoneTop, Tex::SandstoneTop, Tex::SandstoneSide), { hardness: 0.9, tool: ToolKind::Pickaxe, min_tier: 1 }),
    block!("Снег", "snow", all(Tex::Snow), { hardness: 0.3, tool: ToolKind::Shovel }),
    block!("Заснеженная трава", "snow_grass", (Tex::Snow, Tex::Dirt, Tex::SnowGrassSide), { hardness: 0.6, tool: ToolKind::Shovel, drop: Drop::Item(it::DIRT, 1, 1) }),
    block!("Кактус", "cactus", (Tex::CactusTop, Tex::CactusTop, Tex::CactusSide), { layer: Layer::Cutout, hardness: 0.4 }),
    block!("Угольная руда", "coal_ore", all(Tex::CoalOre), { hardness: 2.5, tool: ToolKind::Pickaxe, min_tier: 1, drop: Drop::Item(it::COAL, 1, 1), unstable: true }),
    block!("Медная руда", "copper_ore", all(Tex::CopperOre), { hardness: 2.8, tool: ToolKind::Pickaxe, min_tier: 2, drop: Drop::Item(it::RAW_COPPER, 1, 3), unstable: true }),
    block!("Железная руда", "iron_ore", all(Tex::IronOre), { hardness: 3.0, tool: ToolKind::Pickaxe, min_tier: 2, drop: Drop::Item(it::RAW_IRON, 1, 1), unstable: true }),
    block!("Золотая руда", "gold_ore", all(Tex::GoldOre), { hardness: 3.0, tool: ToolKind::Pickaxe, min_tier: 3, drop: Drop::Item(it::RAW_GOLD, 1, 1), unstable: true }),
    block!("Алмазная руда", "diamond_ore", all(Tex::DiamondOre), { hardness: 3.5, tool: ToolKind::Pickaxe, min_tier: 4, drop: Drop::Item(it::DIAMOND, 1, 1), unstable: true }),
    block!("Резонит", "resonite_ore", all(Tex::ResoniteOre), { hardness: 3.5, tool: ToolKind::Pickaxe, min_tier: 4, drop: Drop::Item(it::RESONITE_SHARD, 1, 3), light: 7, unstable: true }),
    BlockDef {
        name: "Факел",
        key: "torch",
        shape: Shape::Torch,
        layer: Layer::Cutout,
        tex_top: Tex::Torch,
        tex_bottom: Tex::Torch,
        tex_side: Tex::Torch,
        solid: false,
        opaque: false,
        light: 14,
        hardness: 0.0,
        emissive: true,
        ..BASE
    },
    BlockDef {
        name: "Прогоревший факел",
        key: "burnt_torch",
        shape: Shape::Torch,
        layer: Layer::Cutout,
        tex_top: Tex::BurntTorch,
        tex_bottom: Tex::BurntTorch,
        tex_side: Tex::BurntTorch,
        solid: false,
        opaque: false,
        hardness: 0.0,
        ..BASE
    },
    block!("Верстак", "crafting_table", (Tex::CraftingTop, Tex::Planks, Tex::CraftingSide), { tex_front: Some(Tex::CraftingFront), hardness: 2.0, tool: ToolKind::Axe }),
    block!("Печь", "furnace", (Tex::FurnaceTop, Tex::FurnaceTop, Tex::FurnaceSide), { tex_front: Some(Tex::FurnaceFront), hardness: 3.0, tool: ToolKind::Pickaxe, min_tier: 1 }),
    block!("Горящая печь", "furnace_lit", (Tex::FurnaceTop, Tex::FurnaceTop, Tex::FurnaceSide), { tex_front: Some(Tex::FurnaceFrontLit), hardness: 3.0, tool: ToolKind::Pickaxe, min_tier: 1, light: 13, drop: Drop::Item(it::FURNACE, 1, 1) }),
    block!("Сундук", "chest", (Tex::ChestTop, Tex::ChestTop, Tex::ChestSide), { tex_front: Some(Tex::ChestFront), hardness: 2.5, tool: ToolKind::Axe }),
    BlockDef {
        name: "Рельсы",
        key: "rail",
        shape: Shape::Rail,
        layer: Layer::Cutout,
        tex_top: Tex::Rail,
        tex_bottom: Tex::Rail,
        tex_side: Tex::Rail,
        solid: false,
        opaque: false,
        hardness: 0.7,
        tool: ToolKind::Pickaxe,
        ..BASE
    },
    block!("Крепь", "support", (Tex::SupportTop, Tex::SupportTop, Tex::Support), { layer: Layer::Cutout, opaque: false, hardness: 1.5, tool: ToolKind::Axe }),
    block!("Каменные кирпичи", "stone_bricks", all(Tex::StoneBricks), { hardness: 2.0, tool: ToolKind::Pickaxe, min_tier: 1 }),
    block!("Еловое бревно", "spruce_log", (Tex::SpruceLogTop, Tex::SpruceLogTop, Tex::SpruceLogSide), { hardness: 2.0, tool: ToolKind::Axe }),
    block!("Еловая хвоя", "spruce_leaves", all(Tex::SpruceLeaves), { layer: Layer::Cutout, opaque: false, absorb: 1, hardness: 0.3, drop: Drop::Nothing, waving: true }),
    BlockDef {
        name: "Высокая трава",
        key: "tall_grass",
        shape: Shape::Cross,
        layer: Layer::Cutout,
        tex_top: Tex::TallGrass,
        tex_bottom: Tex::TallGrass,
        tex_side: Tex::TallGrass,
        solid: false,
        opaque: false,
        hardness: 0.0,
        drop: Drop::Chance(it::WHEAT, 12),
        replaceable: true,
        waving: true,
        tint: true,
        ..BASE
    },
    BlockDef {
        name: "Мак",
        key: "flower_red",
        shape: Shape::Cross,
        layer: Layer::Cutout,
        tex_top: Tex::FlowerRed,
        tex_bottom: Tex::FlowerRed,
        tex_side: Tex::FlowerRed,
        solid: false,
        opaque: false,
        hardness: 0.0,
        waving: true,
        ..BASE
    },
    BlockDef {
        name: "Одуванчик",
        key: "flower_yellow",
        shape: Shape::Cross,
        layer: Layer::Cutout,
        tex_top: Tex::FlowerYellow,
        tex_bottom: Tex::FlowerYellow,
        tex_side: Tex::FlowerYellow,
        solid: false,
        opaque: false,
        hardness: 0.0,
        waving: true,
        ..BASE
    },
    BlockDef {
        name: "Сухой куст",
        key: "dead_bush",
        shape: Shape::Cross,
        layer: Layer::Cutout,
        tex_top: Tex::DeadBush,
        tex_bottom: Tex::DeadBush,
        tex_side: Tex::DeadBush,
        solid: false,
        opaque: false,
        hardness: 0.0,
        drop: Drop::Item(it::STICK, 0, 2),
        replaceable: true,
        ..BASE
    },
    block!("Лёд", "ice", all(Tex::Ice), { layer: Layer::Translucent, opaque: false, absorb: 2, hardness: 0.5, tool: ToolKind::Pickaxe, drop: Drop::Nothing }),
    block!("Глина", "clay", all(Tex::Clay), { hardness: 0.6, tool: ToolKind::Shovel }),
    block!("Медный блок", "copper_block", all(Tex::CopperBlock), { hardness: 4.0, tool: ToolKind::Pickaxe, min_tier: 2 }),
    block!("Железный блок", "iron_block", all(Tex::IronBlock), { hardness: 5.0, tool: ToolKind::Pickaxe, min_tier: 2 }),
    block!("Золотой блок", "gold_block", all(Tex::GoldBlock), { hardness: 4.0, tool: ToolKind::Pickaxe, min_tier: 3 }),
    block!("Алмазный блок", "diamond_block", all(Tex::DiamondBlock), { hardness: 5.0, tool: ToolKind::Pickaxe, min_tier: 4 }),
    BlockDef {
        name: "Паутина",
        key: "cobweb",
        shape: Shape::Cross,
        layer: Layer::Cutout,
        tex_top: Tex::Cobweb,
        tex_bottom: Tex::Cobweb,
        tex_side: Tex::Cobweb,
        solid: false,
        opaque: false,
        hardness: 2.0,
        tool: ToolKind::Sword,
        drop: Drop::Item(it::STRING, 1, 1),
        ..BASE
    },
    block!("Замшелый булыжник", "mossy_cobblestone", all(Tex::MossyCobble), { hardness: 2.0, tool: ToolKind::Pickaxe, min_tier: 1, unstable: true }),
    block!("Шерсть", "wool", all(Tex::Wool), { hardness: 0.8 }),
    block!("Резонитовая лампа", "resonite_lamp", all(Tex::ResoniteLamp), { hardness: 0.6, light: 15, emissive: true }),
    block!("Обсидиан", "obsidian", all(Tex::Obsidian), { hardness: 15.0, tool: ToolKind::Pickaxe, min_tier: 5 }),
    BlockDef {
        name: "Лестница",
        key: "ladder",
        shape: Shape::Ladder,
        layer: Layer::Cutout,
        tex_top: Tex::Ladder,
        tex_bottom: Tex::Ladder,
        tex_side: Tex::Ladder,
        solid: false,
        opaque: false,
        hardness: 0.4,
        tool: ToolKind::Axe,
        ..BASE
    },
    block!("Еловые доски", "spruce_planks", all(Tex::SprucePlanks), { hardness: 1.5, tool: ToolKind::Axe }),
    block!("Глубинный камень", "deep_stone", all(Tex::DeepStone), { hardness: 2.5, tool: ToolKind::Pickaxe, min_tier: 1, drop: Drop::Item(it::COBBLE, 1, 1), unstable: true }),
    block!("Глубинная угольная руда", "deep_coal_ore", all(Tex::DeepCoalOre), { hardness: 3.0, tool: ToolKind::Pickaxe, min_tier: 1, drop: Drop::Item(it::COAL, 1, 2), unstable: true }),
    block!("Глубинная железная руда", "deep_iron_ore", all(Tex::DeepIronOre), { hardness: 3.5, tool: ToolKind::Pickaxe, min_tier: 2, drop: Drop::Item(it::RAW_IRON, 1, 2), unstable: true }),
    block!("Глубинная золотая руда", "deep_gold_ore", all(Tex::DeepGoldOre), { hardness: 3.5, tool: ToolKind::Pickaxe, min_tier: 3, drop: Drop::Item(it::RAW_GOLD, 1, 2), unstable: true }),
    block!("Глубинная алмазная руда", "deep_diamond_ore", all(Tex::DeepDiamondOre), { hardness: 4.0, tool: ToolKind::Pickaxe, min_tier: 4, drop: Drop::Item(it::DIAMOND, 1, 1), unstable: true }),
];

#[inline]
pub fn def(id: BlockId) -> &'static BlockDef {
    BLOCKS.get(id as usize).unwrap_or(&BLOCKS[0])
}

#[inline]
pub fn block_id(v: u16) -> BlockId {
    (v & 0xFF) as BlockId
}

#[inline]
pub fn block_meta(v: u16) -> u8 {
    (v >> 8) as u8
}

#[inline]
pub fn make(id: BlockId, meta: u8) -> u16 {
    id as u16 | ((meta as u16) << 8)
}

/// Непрозрачен ли блок для света и отсечения граней.
#[inline]
pub fn is_opaque(v: u16) -> bool {
    def(block_id(v)).opaque
}

#[inline]
pub fn is_solid(v: u16) -> bool {
    def(block_id(v)).solid
}

pub fn is_liquid(id: BlockId) -> bool {
    id == id::WATER || id == id::LAVA
}

/// Текстура грани блока: `face` 0=+Y, 1=-Y, 2=+X, 3=-X, 4=+Z, 5=-Z.
/// Для блоков с «лицевой» стороной метаданные хранят её направление (2..5).
pub fn face_tex(v: u16, face: usize) -> Tex {
    let d = def(block_id(v));
    match face {
        0 => d.tex_top,
        1 => d.tex_bottom,
        _ => {
            if let Some(front) = d.tex_front {
                let facing = (block_meta(v) & 7) as usize;
                let facing = if (2..=5).contains(&facing) { facing } else { 4 };
                if face == facing {
                    return front;
                }
            }
            d.tex_side
        }
    }
}

/// Находит блок по строковому ключу (для рецептов).
pub fn by_key(key: &str) -> Option<BlockId> {
    BLOCKS.iter().position(|b| b.key == key).map(|i| i as BlockId)
}
