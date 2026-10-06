//! Рецепты крафта, плавки и топливо. Загружаются из
//! `assets/data/recipes.ron` (если файла нет — встроенная копия).

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::inventory::{ItemStack, Slot};
use crate::item::{self, ItemId};

const EMBEDDED: &str = include_str!("../assets/data/recipes.ron");

#[derive(Deserialize)]
struct RawShaped {
    pattern: Vec<String>,
    key: HashMap<char, String>,
    result: String,
    count: u8,
}

#[derive(Deserialize)]
struct RawShapeless {
    ingredients: Vec<String>,
    result: String,
    count: u8,
}

#[derive(Deserialize)]
struct RawSmelt {
    input: String,
    output: String,
    time: f32,
}

#[derive(Deserialize)]
struct RawFuel {
    item: String,
    burn: f32,
}

#[derive(Deserialize)]
struct RawFile {
    shaped: Vec<RawShaped>,
    shapeless: Vec<RawShapeless>,
    smelting: Vec<RawSmelt>,
    fuels: Vec<RawFuel>,
}

/// Ингредиент: один из нескольких допустимых предметов.
pub type Ingredient = Vec<ItemId>;

pub struct ShapedRecipe {
    pub width: usize,
    pub height: usize,
    /// width*height клеток, None — пусто.
    pub cells: Vec<Option<Ingredient>>,
    pub result: ItemStack,
}

pub struct ShapelessRecipe {
    pub ingredients: Vec<Ingredient>,
    pub result: ItemStack,
}

pub struct SmeltRecipe {
    pub input: ItemId,
    pub output: ItemId,
    pub time: f32,
}

#[derive(Default)]
pub struct Recipes {
    pub shaped: Vec<ShapedRecipe>,
    pub shapeless: Vec<ShapelessRecipe>,
    pub smelting: Vec<SmeltRecipe>,
    pub fuels: HashMap<ItemId, f32>,
}

fn resolve(key: &str) -> Result<ItemId> {
    item::by_key(key).ok_or_else(|| anyhow!("неизвестный предмет «{key}»"))
}

fn ingredient(spec: &str) -> Result<Ingredient> {
    spec.split('|').map(|k| resolve(k.trim())).collect()
}

impl Recipes {
    pub fn load() -> Self {
        let path = crate::paths::assets_dir().join("data").join("recipes.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|_| EMBEDDED.to_string());
        match Self::parse(&text) {
            Ok(r) => {
                log::info!(
                    "Рецепты: {} с узором, {} бесформенных, {} плавок, {} видов топлива",
                    r.shaped.len(),
                    r.shapeless.len(),
                    r.smelting.len(),
                    r.fuels.len()
                );
                r
            }
            Err(e) => {
                log::error!("{}: {e:#} — использую встроенные рецепты", path.display());
                Self::parse(EMBEDDED).unwrap_or_default()
            }
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let raw: RawFile = ron::from_str(text).context("разбор recipes.ron")?;
        let mut r = Recipes::default();
        for s in raw.shaped {
            let height = s.pattern.len();
            let width = s.pattern.iter().map(|l| l.chars().count()).max().unwrap_or(0);
            if width == 0 || width > 3 || height > 3 {
                return Err(anyhow!("узор {:?} должен быть от 1×1 до 3×3", s.pattern));
            }
            let mut cells = Vec::with_capacity(width * height);
            for line in &s.pattern {
                let chars: Vec<char> = line.chars().collect();
                for x in 0..width {
                    let c = chars.get(x).copied().unwrap_or(' ');
                    if c == ' ' {
                        cells.push(None);
                    } else {
                        let spec = s.key.get(&c).ok_or_else(|| anyhow!("символ '{c}' не описан в key ({})", s.result))?;
                        cells.push(Some(ingredient(spec)?));
                    }
                }
            }
            r.shaped.push(ShapedRecipe { width, height, cells, result: ItemStack::new(resolve(&s.result)?, s.count) });
        }
        for s in raw.shapeless {
            let ingredients = s.ingredients.iter().map(|i| ingredient(i)).collect::<Result<Vec<_>>>()?;
            r.shapeless.push(ShapelessRecipe { ingredients, result: ItemStack::new(resolve(&s.result)?, s.count) });
        }
        for s in raw.smelting {
            r.smelting.push(SmeltRecipe { input: resolve(&s.input)?, output: resolve(&s.output)?, time: s.time.max(0.1) });
        }
        for f in raw.fuels {
            r.fuels.insert(resolve(&f.item)?, f.burn);
        }
        Ok(r)
    }

    /// Результат крафта для сетки size×size (слоты построчно).
    pub fn match_grid(&self, grid: &[Slot], size: usize) -> Option<ItemStack> {
        // Ограничивающий прямоугольник непустых клеток.
        let (mut x0, mut y0, mut x1, mut y1) = (size, size, 0, 0);
        let mut any = false;
        for y in 0..size {
            for x in 0..size {
                if grid[y * size + x].is_some() {
                    any = true;
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        if !any {
            return None;
        }
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let at = |x: usize, y: usize| grid[(y0 + y) * size + x0 + x].map(|s| s.item);

        for r in &self.shaped {
            if r.width != w || r.height != h {
                continue;
            }
            for mirror in [false, true] {
                let ok = (0..h).all(|y| {
                    (0..w).all(|x| {
                        let rx = if mirror { w - 1 - x } else { x };
                        match (&r.cells[y * w + rx], at(x, y)) {
                            (None, None) => true,
                            (Some(ing), Some(it)) => ing.contains(&it),
                            _ => false,
                        }
                    })
                });
                if ok {
                    return Some(r.result);
                }
            }
        }

        let mut items: Vec<ItemId> = grid.iter().flatten().map(|s| s.item).collect();
        for r in &self.shapeless {
            if r.ingredients.len() != items.len() {
                continue;
            }
            // Жадное сопоставление: каждому ингредиенту — свой предмет.
            let mut left = items.clone();
            let ok = r.ingredients.iter().all(|ing| {
                if let Some(pos) = left.iter().position(|it| ing.contains(it)) {
                    left.swap_remove(pos);
                    true
                } else {
                    false
                }
            });
            if ok {
                return Some(r.result);
            }
        }
        items.clear();
        None
    }

    pub fn smelt(&self, input: ItemId) -> Option<&SmeltRecipe> {
        self.smelting.iter().find(|r| r.input == input)
    }

    pub fn fuel(&self, item: ItemId) -> Option<f32> {
        self.fuels.get(&item).copied()
    }

    pub fn total(&self) -> usize {
        self.shaped.len() + self.shapeless.len() + self.smelting.len()
    }
}

/// Забирает по одному предмету из каждой непустой клетки сетки.
pub fn consume_grid(grid: &mut [Slot]) {
    for slot in grid.iter_mut() {
        if let Some(s) = slot {
            // Ведро воды возвращает пустое ведро (на будущее для рецептов).
            if s.item == item::id::WATER_BUCKET {
                *slot = Some(ItemStack::new(item::id::BUCKET, 1));
                continue;
            }
            s.count -= 1;
            if s.count == 0 {
                *slot = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::id as b;

    fn grid(items: &[Option<ItemId>]) -> Vec<Slot> {
        items.iter().map(|i| i.map(|id| ItemStack::new(id, 1))).collect()
    }

    #[test]
    fn embedded_recipes_parse_and_count() {
        let r = Recipes::parse(EMBEDDED).expect("recipes.ron");
        assert!(r.total() >= 30, "рецептов {}", r.total());
        assert!(r.fuel(item::id::COAL).is_some());
    }

    #[test]
    fn planks_and_sticks() {
        let r = Recipes::parse(EMBEDDED).unwrap();
        let log = b::OAK_LOG as ItemId;
        let planks = b::PLANKS as ItemId;
        // Бревно в любом месте 2×2 -> 4 доски.
        let g = grid(&[None, None, None, Some(log)]);
        assert_eq!(r.match_grid(&g, 2).map(|s| (s.item, s.count)), Some((planks, 4)));
        // Две доски вертикально -> палки, сдвиг внутри 3×3 не мешает.
        let g = grid(&[None, None, None, None, None, Some(planks), None, None, Some(planks)]);
        assert_eq!(r.match_grid(&g, 3).map(|s| s.item), Some(item::id::STICK));
    }

    #[test]
    fn pickaxe_needs_table_and_axe_mirrors() {
        let r = Recipes::parse(EMBEDDED).unwrap();
        let c = b::COBBLE as ItemId;
        let s = item::id::STICK;
        let g = grid(&[Some(c), Some(c), Some(c), None, Some(s), None, None, Some(s), None]);
        let res = r.match_grid(&g, 3).map(|s| s.item);
        assert_eq!(res, Some(item::tool_id(crate::world::block::ToolKind::Pickaxe, 2)));
        // Зеркальный топор.
        let g = grid(&[Some(c), Some(c), None, Some(s), Some(c), None, Some(s), None, None]);
        assert_eq!(r.match_grid(&g, 3).map(|s| s.item), Some(item::tool_id(crate::world::block::ToolKind::Axe, 2)));
        // Неверный узор — ничего.
        let g = grid(&[Some(c), None, Some(c), None, Some(s), None, None, Some(s), None]);
        assert_eq!(r.match_grid(&g, 3), None);
    }

    #[test]
    fn smelting() {
        let r = Recipes::parse(EMBEDDED).unwrap();
        assert_eq!(r.smelt(item::id::RAW_IRON).map(|s| s.output), Some(item::id::IRON_INGOT));
    }
}
