//! Реестр предметов. ID 1..255 — блоки (совпадают с ID блока), 256+ —
//! самостоятельные предметы.

use crate::assets::textures::Tex;
use crate::world::block::{self, BlockId, ToolKind};

pub type ItemId = u16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemKind {
    Block(BlockId),
    Material,
    Tool { kind: ToolKind, tier: u8, durability: u16, damage: f32 },
    Food { hunger: f32 },
    Bucket,
    WaterBucket,
    /// Резонатор — посылает эхо-импульс (уникальная механика).
    Resonator { durability: u16 },
}

#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub name: &'static str,
    pub key: &'static str,
    pub tex: Tex,
    pub kind: ItemKind,
    pub max_stack: u8,
}

pub mod id {
    use super::ItemId;
    use crate::world::block::id as b;
    // Блоки-предметы — тот же ID.
    pub const DIRT: ItemId = b::DIRT as ItemId;
    pub const COBBLE: ItemId = b::COBBLE as ItemId;
    pub const FURNACE: ItemId = b::FURNACE as ItemId;
    pub const TORCH: ItemId = b::TORCH as ItemId;

    pub const STICK: ItemId = 256;
    pub const COAL: ItemId = 257;
    pub const CHARCOAL: ItemId = 258;
    pub const RAW_COPPER: ItemId = 259;
    pub const COPPER_INGOT: ItemId = 260;
    pub const RAW_IRON: ItemId = 261;
    pub const IRON_INGOT: ItemId = 262;
    pub const RAW_GOLD: ItemId = 263;
    pub const GOLD_INGOT: ItemId = 264;
    pub const DIAMOND: ItemId = 265;
    pub const RESONITE_SHARD: ItemId = 266;
    pub const LEATHER: ItemId = 267;
    pub const RAW_MEAT: ItemId = 268;
    pub const COOKED_MEAT: ItemId = 269;
    pub const APPLE: ItemId = 270;
    pub const BONE: ItemId = 271;
    pub const STRING: ItemId = 272;
    pub const GEL: ItemId = 273;
    pub const BUCKET: ItemId = 274;
    pub const WATER_BUCKET: ItemId = 275;
    pub const RESONATOR: ItemId = 276;
    pub const FLINT: ItemId = 277;
    pub const BREAD: ItemId = 278;
    pub const WHEAT: ItemId = 279;
    pub const FEATHER: ItemId = 280;
    pub const RAW_MUTTON: ItemId = 281;
    pub const COOKED_MUTTON: ItemId = 282;
    /// Инструменты: 300 + вид*5 + тир (вид: кирка, топор, лопата, меч).
    pub const TOOLS_START: ItemId = 300;
    pub const TOOLS_END: ItemId = 320;
}

const MAT: fn(&'static str, &'static str, Tex) -> ItemDef =
    |name, key, tex| ItemDef { name, key, tex, kind: ItemKind::Material, max_stack: 64 };

fn food(name: &'static str, key: &'static str, tex: Tex, hunger: f32) -> ItemDef {
    ItemDef { name, key, tex, kind: ItemKind::Food { hunger }, max_stack: 64 }
}

/// Список самостоятельных предметов (256..).
fn simple_items() -> Vec<ItemDef> {
    vec![
        MAT("Палка", "stick", Tex::Stick),
        MAT("Уголь", "coal", Tex::Coal),
        MAT("Древесный уголь", "charcoal", Tex::Charcoal),
        MAT("Сырая медь", "raw_copper", Tex::RawCopper),
        MAT("Медный слиток", "copper_ingot", Tex::CopperIngot),
        MAT("Сырое железо", "raw_iron", Tex::RawIron),
        MAT("Железный слиток", "iron_ingot", Tex::IronIngot),
        MAT("Сырое золото", "raw_gold", Tex::RawGold),
        MAT("Золотой слиток", "gold_ingot", Tex::GoldIngot),
        MAT("Алмаз", "diamond", Tex::Diamond),
        MAT("Осколок резонита", "resonite_shard", Tex::ResoniteShard),
        MAT("Кожа", "leather", Tex::Leather),
        food("Сырое мясо", "raw_meat", Tex::RawMeat, 2.0),
        food("Жареное мясо", "cooked_meat", Tex::CookedMeat, 7.0),
        food("Яблоко", "apple", Tex::Apple, 3.0),
        MAT("Кость", "bone", Tex::Bone),
        MAT("Нить", "string", Tex::StringItem),
        MAT("Слизь", "gel", Tex::Gel),
        ItemDef { name: "Ведро", key: "bucket", tex: Tex::Bucket, kind: ItemKind::Bucket, max_stack: 16 },
        ItemDef { name: "Ведро воды", key: "water_bucket", tex: Tex::WaterBucket, kind: ItemKind::WaterBucket, max_stack: 1 },
        ItemDef { name: "Резонатор", key: "resonator", tex: Tex::Resonator, kind: ItemKind::Resonator { durability: 64 }, max_stack: 1 },
        MAT("Кремень", "flint", Tex::Flint),
        food("Хлеб", "bread", Tex::Bread, 5.0),
        MAT("Пшеница", "wheat", Tex::Wheat),
        MAT("Перо", "feather", Tex::Feather),
        food("Сырая баранина", "raw_mutton", Tex::RawMutton, 2.0),
        food("Жареная баранина", "cooked_mutton", Tex::CookedMutton, 6.0),
    ]
}

const TIER_NAMES: [&str; 5] = ["Деревянн", "Каменн", "Медн", "Железн", "Алмазн"];
const TIER_KEYS: [&str; 5] = ["wood", "stone", "copper", "iron", "diamond"];
const TIER_DURABILITY: [u16; 5] = [60, 132, 200, 250, 1560];

fn tool_def(index: usize) -> ItemDef {
    let kind_idx = index / 5;
    let tier = index % 5;
    let (kind, noun, ending, tex_base, dmg) = match kind_idx {
        0 => (ToolKind::Pickaxe, "кирка", "ая", Tex::WoodPickaxe as u16, 2.0),
        1 => (ToolKind::Axe, "топор", "ый", Tex::WoodAxe as u16, 3.0),
        2 => (ToolKind::Shovel, "лопата", "ая", Tex::WoodShovel as u16, 1.5),
        _ => (ToolKind::Sword, "меч", "ый", Tex::WoodSword as u16, 4.0),
    };
    let name: &'static str = Box::leak(format!("{}{} {}", TIER_NAMES[tier], ending, noun).into_boxed_str());
    let key: &'static str = Box::leak(
        format!(
            "{}_{}",
            TIER_KEYS[tier],
            match kind {
                ToolKind::Pickaxe => "pickaxe",
                ToolKind::Axe => "axe",
                ToolKind::Shovel => "shovel",
                _ => "sword",
            }
        )
        .into_boxed_str(),
    );
    let tex = crate::assets::textures::ALL_TILES[(tex_base as usize) + tier];
    ItemDef {
        name,
        key,
        tex,
        kind: ItemKind::Tool {
            kind,
            tier: tier as u8 + 1,
            durability: TIER_DURABILITY[tier],
            damage: dmg + tier as f32,
        },
        max_stack: 1,
    }
}

pub struct ItemRegistry {
    blocks: Vec<Option<ItemDef>>,
    items: Vec<ItemDef>,
    tools: Vec<ItemDef>,
}

impl ItemRegistry {
    fn build() -> Self {
        let blocks = (0..block::id::COUNT)
            .map(|i| {
                let d = block::def(i as BlockId);
                if i == 0 || d.hardness < 0.0 && i != block::id::BEDROCK as usize {
                    None
                } else {
                    Some(ItemDef {
                        name: d.name,
                        key: d.key,
                        tex: if d.shape == block::Shape::Cube { d.tex_side } else { d.tex_top },
                        kind: ItemKind::Block(i as BlockId),
                        max_stack: 64,
                    })
                }
            })
            .collect();
        Self { blocks, items: simple_items(), tools: (0..20).map(tool_def).collect() }
    }
}

pub fn registry() -> &'static ItemRegistry {
    static REG: std::sync::OnceLock<ItemRegistry> = std::sync::OnceLock::new();
    REG.get_or_init(ItemRegistry::build)
}

pub fn def(item: ItemId) -> Option<&'static ItemDef> {
    let r = registry();
    if (item as usize) < 256 {
        r.blocks.get(item as usize).and_then(|d| d.as_ref())
    } else if (id::TOOLS_START..id::TOOLS_END).contains(&item) {
        r.tools.get((item - id::TOOLS_START) as usize)
    } else {
        r.items.get((item - 256) as usize)
    }
}

pub fn name(item: ItemId) -> &'static str {
    def(item).map(|d| d.name).unwrap_or("???")
}

pub fn max_stack(item: ItemId) -> u8 {
    def(item).map(|d| d.max_stack).unwrap_or(64)
}

/// Все существующие предметы (для креатива и поиска по ключу).
pub fn all_items() -> Vec<ItemId> {
    let mut v = Vec::new();
    for i in 1..256u16 {
        if def(i).is_some() {
            v.push(i);
        }
    }
    let r = registry();
    for i in 0..r.items.len() {
        v.push(256 + i as u16);
    }
    for i in 0..r.tools.len() {
        v.push(id::TOOLS_START + i as u16);
    }
    v
}

pub fn by_key(key: &str) -> Option<ItemId> {
    all_items().into_iter().find(|&i| def(i).map(|d| d.key == key).unwrap_or(false))
}

pub fn tool_id(kind: ToolKind, tier: u8) -> ItemId {
    let k = match kind {
        ToolKind::Pickaxe => 0,
        ToolKind::Axe => 1,
        ToolKind::Shovel => 2,
        _ => 3,
    };
    id::TOOLS_START + k * 5 + (tier.saturating_sub(1)) as u16
}
