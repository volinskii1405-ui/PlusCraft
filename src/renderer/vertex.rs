//! Форматы вершин для всех пайплайнов.

use ash::vk;
use bytemuck::{Pod, Zeroable};

/// Вершина чанка — 20 байт.
/// * `pos`: x, y, z в 1/16 блока относительно угла чанка; w — слой текстуры;
/// * `uv`: в 1/16 текселя тайла (greedy-квады повторяют текстуру);
/// * `info`: [нормаль(3 бита) | AO(2 бита) << 3, небесный свет ×4, блочный свет ×4, флаги];
/// * `color`: тинт (цвет биома для травы/листвы).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ChunkVertex {
    pub pos: [u16; 4],
    pub uv: [u16; 2],
    pub info: [u8; 4],
    pub color: [u8; 4],
}

/// Флаги вершины чанка.
pub const VFLAG_WAVE: u8 = 1; // колышется на ветру (листва, трава)
pub const VFLAG_LIQUID: u8 = 2; // поверхность жидкости
pub const VFLAG_EMISSIVE: u8 = 4; // светится сама (лава, кристаллы)

impl ChunkVertex {
    pub fn binding() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }
    pub fn attributes() -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            attr(0, vk::Format::R16G16B16A16_UINT, 0),
            attr(1, vk::Format::R16G16_UINT, 8),
            attr(2, vk::Format::R8G8B8A8_UINT, 12),
            attr(3, vk::Format::R8G8B8A8_UNORM, 16),
        ]
    }
}

/// Вершина динамической 3D-геометрии (мобы, предметы, рамка выделения, солнце).
/// Позиция — относительно камеры.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct EntityVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    /// [слой текстуры (0xFFFF = без текстуры), (небесный свет << 8) | блочный свет]
    pub extra: [u16; 2],
}

pub const NO_TEXTURE: u16 = 0xFFFF;

impl EntityVertex {
    pub fn binding() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }
    pub fn attributes() -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            attr(0, vk::Format::R32G32B32_SFLOAT, 0),
            attr(1, vk::Format::R32G32_SFLOAT, 12),
            attr(2, vk::Format::R8G8B8A8_UNORM, 20),
            attr(3, vk::Format::R16G16_UINT, 24),
        ]
    }
}

/// Вершина интерфейса (экранные пиксели).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    /// Источник текстуры: `UI_SOLID` — сплошной цвет, `UI_ATLAS_BIT | слой` —
    /// атлас интерфейса (шрифт), иначе — слой массива текстур блоков.
    pub tex: u32,
}

pub const UI_SOLID: u32 = 0xFFFF_FFFF;
pub const UI_ATLAS_BIT: u32 = 0x8000_0000;

impl UiVertex {
    pub fn binding() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }
    pub fn attributes() -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            attr(0, vk::Format::R32G32_SFLOAT, 0),
            attr(1, vk::Format::R32G32_SFLOAT, 8),
            attr(2, vk::Format::R8G8B8A8_UNORM, 16),
            attr(3, vk::Format::R32_UINT, 20),
        ]
    }
}

fn attr(location: u32, format: vk::Format, offset: u32) -> vk::VertexInputAttributeDescription {
    vk::VertexInputAttributeDescription { location, binding: 0, format, offset }
}

/// Uniform-буфер с глобальными параметрами кадра (std140, только vec4/mat4).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Globals {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    /// xyz — мировая позиция камеры (для шума/волн), w — время в секундах.
    pub cam_pos: [f32; 4],
    /// rgb — цвет тумана, a — начало тумана.
    pub fog_color: [f32; 4],
    /// x — конец тумана, y — дневной свет 0..1, z — под водой (0/1), w — мин. яркость.
    pub params: [f32; 4],
    /// xyz — направление на солнце, w — фаза/время суток 0..1.
    pub sun_dir: [f32; 4],
    pub sky_top: [f32; 4],
    pub sky_horizon: [f32; 4],
    /// rgb — цвет блочного света, w — настройка яркости (гамма).
    pub block_light: [f32; 4],
    /// rgb — цвет неба для небесного освещения, w — эффект темноты (дебафф) 0..1.
    pub sky_light: [f32; 4],
}
