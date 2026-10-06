//! Уникальные механики PlusCraft: динамика воды, обвалы пород, резонанс
//! кристаллов, свет как ресурс. (Мутации мобов — в `entities::mob`.)

pub mod cavein;
pub mod resonance;
pub mod torches;
pub mod water;

use cavein::CaveIns;
use resonance::Pulse;
use torches::{Darkness, TorchFuel};
use water::Water;

#[derive(Default)]
pub struct Mechanics {
    pub water: Water,
    pub caveins: CaveIns,
    pub torches: TorchFuel,
    pub darkness: Darkness,
    pub pulses: Vec<Pulse>,
    /// Накопитель для тиков мира (20 в секунду).
    pub tick_acc: f32,
}
