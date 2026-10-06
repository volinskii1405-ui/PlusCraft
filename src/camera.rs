//! Камера от первого лица.

use glam::{DVec3, Mat4, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: DVec3,
    /// Рыскание (радианы), 0 — взгляд вдоль -Z.
    pub yaw: f32,
    /// Тангаж (радианы), положительный — вверх.
    pub pitch: f32,
    pub fov_deg: f32,
}

impl Camera {
    pub fn new(pos: DVec3) -> Self {
        Self { pos, yaw: 0.0, pitch: 0.0, fov_deg: 75.0 }
    }

    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }

    /// Горизонтальное направление «вперёд» (для ходьбы).
    pub fn forward_flat(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(-sy, 0.0, -cy)
    }

    pub fn right_flat(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cy, 0.0, -sy)
    }

    /// Матрица вида без переноса — рендер ведётся относительно камеры, чтобы
    /// не терять точность float вдали от начала координат.
    pub fn view_rotation(&self) -> Mat4 {
        Mat4::look_to_rh(Vec3::ZERO, self.forward(), Vec3::Y)
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx;
        self.pitch = (self.pitch - dy).clamp(-1.55, 1.55);
        self.yaw = self.yaw.rem_euclid(std::f32::consts::TAU);
    }
}
