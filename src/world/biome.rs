//! Биомы: параметры рельефа и цвета растительности.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BiomeId {
    Plains = 0,
    Forest = 1,
    Desert = 2,
    Mountains = 3,
    SnowyTaiga = 4,
    Ocean = 5,
    Beach = 6,
}

pub struct Biome {
    pub name: &'static str,
    pub grass: [u8; 3],
    pub foliage: [u8; 3],
}

pub static BIOMES: [Biome; 7] = [
    Biome { name: "Равнины", grass: [255, 255, 255], foliage: [255, 255, 255] },
    Biome { name: "Лес", grass: [215, 240, 205], foliage: [200, 235, 190] },
    Biome { name: "Пустыня", grass: [255, 235, 170], foliage: [235, 225, 160] },
    Biome { name: "Горы", grass: [200, 225, 215], foliage: [190, 220, 205] },
    Biome { name: "Снежная тайга", grass: [200, 225, 225], foliage: [185, 215, 210] },
    Biome { name: "Океан", grass: [235, 255, 240], foliage: [225, 250, 230] },
    Biome { name: "Пляж", grass: [255, 250, 210], foliage: [245, 245, 200] },
];

pub fn get(id: u8) -> &'static Biome {
    BIOMES.get(id as usize).unwrap_or(&BIOMES[0])
}
