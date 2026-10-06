//! Мир: чанки, генерация, свет, меширование, фоновые задачи.

pub mod biome;
pub mod block;
pub mod caves;
pub mod chunk;
pub mod gen;
pub mod jobs;
pub mod light;
pub mod mesher;
pub mod mines;
pub mod neighborhood;
pub mod noise;
pub mod ores;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use block::{block_id, def, id};
use chunk::{idx, Chunk, ChunkPos, CHUNK_H, CHUNK_W};
use jobs::{Job, JobPool, JobResult};
use mesher::MeshOutput;
use neighborhood::Neighborhood;

use crate::save::WorldSave;

pub struct World {
    pub seed: u64,
    pub gen: Arc<gen::WorldGen>,
    pub chunks: HashMap<ChunkPos, Chunk>,
    pool: JobPool,
    gen_pending: HashSet<ChunkPos>,
    mesh_in_flight: usize,
    gen_in_flight: usize,
    /// Чанки, которые надо перемешать вне очереди (после правок игрока).
    urgent: HashSet<ChunkPos>,
    pub save: Option<Arc<WorldSave>>,
    pub smooth_lighting: bool,
    /// Готовые меши для загрузки в рендерер.
    pub ready_meshes: Vec<(ChunkPos, MeshOutput)>,
    /// Выгруженные чанки — их меши надо удалить из рендерера.
    pub unloaded: Vec<ChunkPos>,
    /// Порядок обхода чанков по удалённости (кэш для текущего радиуса).
    spiral: Vec<(i32, i32)>,
    spiral_radius: i32,
    pub center: ChunkPos,
    pub radius: i32,
}

impl World {
    pub fn new(seed: u64, save: Option<Arc<WorldSave>>, smooth_lighting: bool) -> Self {
        let gen = Arc::new(gen::WorldGen::new(seed));
        let pool = JobPool::new(gen.clone(), save.clone());
        Self {
            seed,
            gen,
            chunks: HashMap::new(),
            pool,
            gen_pending: HashSet::new(),
            mesh_in_flight: 0,
            gen_in_flight: 0,
            urgent: HashSet::new(),
            save,
            smooth_lighting,
            ready_meshes: Vec::new(),
            unloaded: Vec::new(),
            spiral: Vec::new(),
            spiral_radius: -1,
            center: ChunkPos::new(0, 0),
            radius: 8,
        }
    }

    pub fn worker_count(&self) -> usize {
        self.pool.threads()
    }

    pub fn pending_jobs(&self) -> (usize, usize) {
        (self.gen_in_flight, self.mesh_in_flight)
    }

    fn rebuild_spiral(&mut self, r: i32) {
        let mut v = Vec::new();
        for dz in -r..=r {
            for dx in -r..=r {
                v.push((dx, dz));
            }
        }
        v.sort_by_key(|&(x, z)| x * x + z * z);
        self.spiral = v;
        self.spiral_radius = r;
    }

    /// Обновление стриминга: подгрузка/выгрузка, постановка задач, приём результатов.
    pub fn update(&mut self, center: ChunkPos, radius: i32) {
        self.center = center;
        self.radius = radius;
        if self.spiral_radius != radius + 1 {
            self.rebuild_spiral(radius + 1);
        }

        self.receive();

        // Выгрузка далёких чанков.
        let keep = radius + 2;
        let far: Vec<ChunkPos> = self
            .chunks
            .keys()
            .filter(|p| (p.x - center.x).abs() > keep || (p.z - center.z).abs() > keep)
            .copied()
            .collect();
        for p in far {
            if let Some(c) = self.chunks.remove(&p) {
                if c.modified {
                    self.pool.submit_low(Job::Save(p, c.data.clone()));
                }
            }
            self.unloaded.push(p);
        }

        // Генерация недостающих (радиус + 1, чтобы у видимых были соседи).
        let max_gen = self.pool.threads() * 3;
        let mut i = 0;
        while self.gen_in_flight < max_gen && i < self.spiral.len() {
            let (dx, dz) = self.spiral[i];
            i += 1;
            let p = center.offset(dx, dz);
            if self.chunks.contains_key(&p) || self.gen_pending.contains(&p) {
                continue;
            }
            self.gen_pending.insert(p);
            self.gen_in_flight += 1;
            self.pool.submit_low(Job::Gen(p));
        }

        // Срочные перемешивания (правки игрока).
        let urgent: Vec<ChunkPos> = self.urgent.drain().collect();
        for p in urgent {
            self.try_submit_mesh(p, true);
        }

        // Меширование видимых чанков по удалённости.
        let max_mesh = self.pool.threads() * 3;
        let mut i = 0;
        while self.mesh_in_flight < max_mesh && i < self.spiral.len() {
            let (dx, dz) = self.spiral[i];
            i += 1;
            if dx.abs() > radius || dz.abs() > radius {
                continue;
            }
            let p = center.offset(dx, dz);
            self.try_submit_mesh(p, false);
        }
    }

    fn try_submit_mesh(&mut self, p: ChunkPos, urgent: bool) -> bool {
        let Some(c) = self.chunks.get(&p) else { return false };
        if !c.mesh_dirty || c.mesh_pending {
            if urgent && c.mesh_dirty {
                // Ещё в работе — повторим после результата.
                self.urgent.insert(p);
            }
            return false;
        }
        let mut chunks: [[Option<Arc<chunk::ChunkData>>; 3]; 3] = Default::default();
        for dz in -1..=1 {
            for dx in -1..=1 {
                match self.chunks.get(&p.offset(dx, dz)) {
                    Some(n) => chunks[(dz + 1) as usize][(dx + 1) as usize] = Some(n.data.clone()),
                    None => return false,
                }
            }
        }
        let version = c.version;
        let nb = Neighborhood { center: p, chunks };
        if let Some(c) = self.chunks.get_mut(&p) {
            c.mesh_dirty = false;
            c.mesh_pending = true;
        }
        self.mesh_in_flight += 1;
        let job = Job::Mesh { pos: p, nb, version, smooth: self.smooth_lighting };
        if urgent {
            self.pool.submit_high(job);
        } else {
            self.pool.submit_low(job);
        }
        true
    }

    fn receive(&mut self) {
        while let Some(r) = self.pool.try_recv() {
            match r {
                JobResult::Gen(p, data) => {
                    self.gen_in_flight = self.gen_in_flight.saturating_sub(1);
                    self.gen_pending.remove(&p);
                    if (p.x - self.center.x).abs() > self.radius + 2 || (p.z - self.center.z).abs() > self.radius + 2 {
                        continue;
                    }
                    let (data, from_save) = data;
                    let mut c = Chunk::new(p, data);
                    c.modified = from_save;
                    self.chunks.insert(p, c);
                }
                JobResult::Mesh(p, _version, mut out) => {
                    self.mesh_in_flight = self.mesh_in_flight.saturating_sub(1);
                    if let Some(c) = self.chunks.get_mut(&p) {
                        c.mesh_pending = false;
                        c.light = Some(Arc::new(std::mem::take(&mut out.light)));
                        self.ready_meshes.push((p, out));
                    }
                }
            }
        }
    }

    /// Блок по мировым координатам (0 — воздух/не загружено).
    pub fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        if y < 0 {
            return block::make(id::BEDROCK, 0);
        }
        if y >= CHUNK_H as i32 {
            return 0;
        }
        let p = ChunkPos::of_block(x, z);
        match self.chunks.get(&p) {
            Some(c) => c.get((x & 15) as usize, y as usize, (z & 15) as usize),
            None => 0,
        }
    }

    pub fn get_id(&self, x: i32, y: i32, z: i32) -> u8 {
        block_id(self.get(x, y, z))
    }

    pub fn is_loaded_at(&self, x: i32, z: i32) -> bool {
        self.chunks.contains_key(&ChunkPos::of_block(x, z))
    }

    /// Твёрдый ли блок для коллизий. Незагруженные чанки считаются твёрдыми,
    /// чтобы игрок не проваливался, пока мир догружается.
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 {
            return true;
        }
        if y >= CHUNK_H as i32 {
            return false;
        }
        let p = ChunkPos::of_block(x, z);
        match self.chunks.get(&p) {
            Some(c) => def(block_id(c.get((x & 15) as usize, y as usize, (z & 15) as usize))).solid,
            None => true,
        }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, v: u16) -> bool {
        if !(0..CHUNK_H as i32).contains(&y) {
            return false;
        }
        let p = ChunkPos::of_block(x, z);
        let Some(c) = self.chunks.get_mut(&p) else { return false };
        let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
        if c.get(lx, y as usize, lz) == v {
            return true;
        }
        c.set(lx, y as usize, lz, v);
        c.modified = true;
        // Свет может измениться в радиусе 15 блоков — перемешиваем соседей.
        for dz in -1..=1 {
            for dx in -1..=1 {
                let np = p.offset(dx, dz);
                if let Some(n) = self.chunks.get_mut(&np) {
                    n.mesh_dirty = true;
                    self.urgent.insert(np);
                }
            }
        }
        true
    }

    /// Свет (небо, блоки) по мировым координатам.
    pub fn light(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        if y >= CHUNK_H as i32 {
            return (15, 0);
        }
        if y < 0 {
            return (0, 0);
        }
        match self.chunks.get(&ChunkPos::of_block(x, z)) {
            Some(c) => c.light_at((x & 15) as usize, y as usize, (z & 15) as usize),
            None => (15, 0),
        }
    }

    /// Высота самого верхнего твёрдого блока колонки.
    pub fn surface_y(&self, x: i32, z: i32) -> Option<i32> {
        let c = self.chunks.get(&ChunkPos::of_block(x, z))?;
        let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
        (0..CHUNK_H).rev().find(|&y| def(block_id(c.data.blocks[idx(lx, y, lz)])).solid).map(|y| y as i32)
    }

    pub fn biome_at(&self, x: i32, z: i32) -> u8 {
        match self.chunks.get(&ChunkPos::of_block(x, z)) {
            Some(c) => c.data.biomes[((z & 15) as usize) * CHUNK_W + (x & 15) as usize],
            None => self.gen.column(x, z).biome as u8,
        }
    }

    /// Пометить все чанки на перемешивание (например, сменилась настройка света).
    pub fn remesh_all(&mut self) {
        for c in self.chunks.values_mut() {
            c.mesh_dirty = true;
        }
    }

    /// Синхронно сохраняет все изменённые чанки.
    pub fn save_all(&mut self) {
        let Some(save) = self.save.clone() else { return };
        for c in self.chunks.values_mut() {
            if c.modified {
                if let Err(e) = save.save_chunk(c.pos, &c.data) {
                    log::error!("сохранение чанка {:?}: {e:#}", c.pos);
                }
            }
        }
        self.pool.flush_saves();
    }
}
