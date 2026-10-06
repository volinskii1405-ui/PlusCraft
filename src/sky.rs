//! Цикл дня и ночи: положение солнца и луны, цвета неба, тумана и
//! небесного освещения.
//!
//! Время суток `t` ∈ [0, 1): 0 — рассвет, 0.25 — полдень, 0.5 — закат,
//! 0.75 — полночь.

use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct SkyState {
    pub sun_dir: Vec3,
    /// Множитель небесного освещения 0..1.
    pub daylight: f32,
    pub sky_top: [f32; 3],
    pub sky_horizon: [f32; 3],
    /// Цвет небесного света (тёплый на закате, голубоватый ночью).
    pub sky_light: [f32; 3],
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn sky_state(t: f64) -> SkyState {
    let angle = (t as f32) * std::f32::consts::TAU;
    // Солнце ходит с востока (+X) через зенит на запад, с небольшим наклоном орбиты.
    let sun_dir = Vec3::new(angle.cos(), angle.sin(), 0.28).normalize();
    let h = sun_dir.y;

    let day = smoothstep(-0.18, 0.25, h);
    // Ночью остаётся «лунный» свет.
    let daylight = 0.14 + 0.86 * day;

    let day_top = [0.33, 0.55, 0.95];
    let day_horizon = [0.72, 0.84, 0.98];
    let night_top = [0.01, 0.015, 0.04];
    let night_horizon = [0.05, 0.06, 0.12];
    let sunset_horizon = [0.98, 0.52, 0.28];

    let mut top = mix3(night_top, day_top, day);
    let mut horizon = mix3(night_horizon, day_horizon, day);
    // Закат/рассвет: оранжевый горизонт, когда солнце низко.
    let twilight = (1.0 - smoothstep(0.0, 0.32, h.abs())) * smoothstep(-0.3, 0.0, h);
    horizon = mix3(horizon, sunset_horizon, twilight * 0.55);
    top = mix3(top, [0.35, 0.35, 0.6], twilight * 0.25);

    let sky_light = mix3(mix3([0.6, 0.68, 1.0], [1.0, 1.0, 1.0], day), [1.0, 0.82, 0.65], twilight * 0.6);

    SkyState { sun_dir, daylight, sky_top: top, sky_horizon: horizon, sky_light }
}

/// Человекочитаемое время суток (часы:минуты, 0.0 = 6:00).
pub fn clock(t: f64) -> String {
    let hours = (t * 24.0 + 6.0) % 24.0;
    let h = hours.floor() as u32;
    let m = ((hours - h as f64) * 60.0).floor() as u32;
    format!("{h:02}:{m:02}")
}

/// Ночь ли сейчас (для спавна враждебных мобов).
pub fn is_night(t: f64) -> bool {
    sky_state(t).daylight < 0.4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_night_brightness() {
        assert!(sky_state(0.25).daylight > 0.95, "полдень");
        assert!(sky_state(0.75).daylight < 0.2, "полночь");
        assert!(sky_state(0.25).sun_dir.y > 0.9);
        assert!(sky_state(0.75).sun_dir.y < -0.9);
        assert!(is_night(0.75) && !is_night(0.25));
        assert_eq!(clock(0.25), "12:00");
    }
}
