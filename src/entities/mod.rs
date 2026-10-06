//! Сущности: выпавшие предметы (и мобы — см. `mob.rs`).

pub mod mesh;
pub mod mob;
pub mod path;
pub mod spawn;

use glam::{DVec3, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::inventory::ItemStack;
use crate::item::{self, ItemKind};
use crate::physics::{self, Aabb};
use crate::renderer::vertex::EntityVertex;
use crate::world::block::{self, Shape};
use crate::world::World;

use mesh::{pack_light, FaceTex};

/// Время жизни выпавшего предмета, с.
const ITEM_LIFETIME: f32 = 300.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemEntity {
    pub stack: ItemStack,
    pub pos: DVec3,
    pub vel: DVec3,
    pub age: f32,
    pub pickup_delay: f32,
}

impl ItemEntity {
    pub fn new(stack: ItemStack, pos: DVec3, vel: DVec3) -> Self {
        Self { stack, pos, vel, age: 0.0, pickup_delay: 0.5 }
    }

    fn aabb(&self) -> Aabb {
        Aabb::from_feet(self.pos, 0.25, 0.25)
    }

    pub fn step(&mut self, world: &World, dt: f64) {
        self.age += dt as f32;
        self.pickup_delay = (self.pickup_delay - dt as f32).max(0.0);
        if !world.is_loaded_at(self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
            return;
        }
        let in_water = world.get_id(self.pos.x.floor() as i32, (self.pos.y + 0.1).floor() as i32, self.pos.z.floor() as i32)
            == block::id::WATER;
        if in_water {
            self.vel.y += (1.5 - self.vel.y) * (3.0 * dt).min(1.0);
            self.vel.x *= 1.0 - (2.0 * dt).min(1.0);
            self.vel.z *= 1.0 - (2.0 * dt).min(1.0);
        } else {
            self.vel.y = (self.vel.y - 25.0 * dt).max(-40.0);
        }
        let mut b = self.aabb();
        // Предмет внутри блока (например, выпал из разрушенного) — выталкиваем вверх.
        if physics::collides(world, &b) {
            self.pos.y += 4.0 * dt;
            self.vel = DVec3::new(self.vel.x * 0.5, 0.0, self.vel.z * 0.5);
            return;
        }
        let (_, hit) = physics::move_box(world, &mut b, self.vel * dt);
        if hit[1] {
            self.vel.y = 0.0;
            // Трение о землю.
            self.vel.x *= 1.0 - (8.0 * dt).min(1.0);
            self.vel.z *= 1.0 - (8.0 * dt).min(1.0);
        }
        if hit[0] {
            self.vel.x = 0.0;
        }
        if hit[2] {
            self.vel.z = 0.0;
        }
        self.pos = DVec3::new((b.min.x + b.max.x) * 0.5, b.min.y, (b.min.z + b.max.z) * 0.5);
    }

    pub fn expired(&self) -> bool {
        self.age > ITEM_LIFETIME || self.pos.y < -64.0
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Entities {
    pub items: Vec<ItemEntity>,
    #[serde(default)]
    pub mobs: Vec<mob::Mob>,
}

impl Entities {
    pub fn spawn_item(&mut self, stack: ItemStack, pos: DVec3, vel: DVec3) {
        if stack.count == 0 {
            return;
        }
        self.items.push(ItemEntity::new(stack, pos, vel));
    }

    pub fn step_items(&mut self, world: &World, dt: f64) {
        for it in &mut self.items {
            it.step(world, dt);
        }
        self.items.retain(|i| !i.expired());
        // Слияние одинаковых предметов, лежащих рядом.
        let n = self.items.len();
        if n > 1 && n < 400 {
            for i in 0..n {
                for j in (i + 1)..n {
                    let (a, b) = self.items.split_at_mut(j);
                    let (x, y) = (&mut a[i], &mut b[0]);
                    if x.stack.count == 0 || y.stack.count == 0 || !x.stack.stackable_with(&y.stack) {
                        continue;
                    }
                    if x.pos.distance_squared(y.pos) < 0.6 {
                        let room = x.stack.max_stack().saturating_sub(x.stack.count);
                        let k = room.min(y.stack.count);
                        x.stack.count += k;
                        y.stack.count -= k;
                    }
                }
            }
            self.items.retain(|i| i.stack.count > 0);
        }
    }

    /// Геометрия выпавших предметов: блоки — маленькие кубики, прочее — карточки.
    pub fn build_item_mesh(&self, world: &World, cam: DVec3, time: f32, out: &mut Vec<EntityVertex>) {
        for it in &self.items {
            let rel = (it.pos - cam).as_vec3();
            if rel.length_squared() > 64.0 * 64.0 {
                continue;
            }
            let (sky, blk) = world.light(it.pos.x.floor() as i32, (it.pos.y + 0.2).floor() as i32, it.pos.z.floor() as i32);
            let light = pack_light(sky, blk);
            let bob = (time * 2.5 + it.age * 0.7).sin() * 0.06 + 0.18;
            let rot = Quat::from_rotation_y(time * 1.5 + it.pos.x as f32);
            let copies = if it.stack.count > 16 { 3 } else if it.stack.count > 1 { 2 } else { 1 };
            for c in 0..copies {
                let off = Vec3::new(c as f32 * 0.07, c as f32 * 0.05, c as f32 * -0.06);
                let model = Mat4::from_rotation_translation(rot, rel + Vec3::new(0.0, bob, 0.0) + off);
                push_item_model(out, &model, it.stack.item, light, 0.25);
            }
        }
    }
}

/// Модель предмета: блок — кубик с текстурами граней, остальное — карточка.
pub fn push_item_model(out: &mut Vec<EntityVertex>, model: &Mat4, item_id: item::ItemId, light: u16, size: f32) {
    let Some(def) = item::def(item_id) else { return };
    match def.kind {
        ItemKind::Block(b) if block::def(b).shape == Shape::Cube || block::def(b).shape == Shape::Liquid => {
            let v = block::make(b, 4);
            let faces = [
                FaceTex::full(block::face_tex(v, 0) as u16),
                FaceTex::full(block::face_tex(v, 1) as u16),
                FaceTex::full(block::face_tex(v, 2) as u16),
                FaceTex::full(block::face_tex(v, 3) as u16),
                FaceTex::full(block::face_tex(v, 4) as u16),
                FaceTex::full(block::face_tex(v, 5) as u16),
            ];
            let h = size * 0.5;
            mesh::push_box(out, model, Vec3::splat(-h), Vec3::splat(h), &faces, [255, 255, 255, 255], light);
        }
        _ => {
            let m = *model * Mat4::from_translation(Vec3::new(0.0, size * 0.3, 0.0));
            mesh::push_card(out, &m, size * 1.6, def.tex as u16, [255, 255, 255, 255], light);
        }
    }
}
