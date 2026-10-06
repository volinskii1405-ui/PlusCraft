//! Пул рабочих потоков: генерация, меширование и сохранение чанков не
//! блокируют поток рендера.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::{select, unbounded, Receiver, Sender};

use super::chunk::{ChunkData, ChunkPos};
use super::gen::WorldGen;
use super::mesher::{self, MeshOutput};
use super::neighborhood::Neighborhood;
use crate::save::WorldSave;

pub enum Job {
    Gen(ChunkPos),
    Mesh { pos: ChunkPos, nb: Neighborhood, version: u64, smooth: bool },
    Save(ChunkPos, Arc<ChunkData>),
}

pub enum JobResult {
    /// Данные чанка и флаг «загружен из сохранения».
    Gen(ChunkPos, (ChunkData, bool)),
    Mesh(ChunkPos, u64, MeshOutput),
}

pub struct JobPool {
    high: Option<Sender<Job>>,
    low: Option<Sender<Job>>,
    results: Receiver<JobResult>,
    threads: Vec<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    saves_pending: Arc<AtomicUsize>,
}

impl JobPool {
    pub fn new(gen: Arc<WorldGen>, save: Option<Arc<WorldSave>>) -> Self {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        // Один поток оставляем рендеру/игровой логике.
        let n = n.saturating_sub(1).clamp(2, 8);
        let (high_tx, high_rx) = unbounded::<Job>();
        let (low_tx, low_rx) = unbounded::<Job>();
        let (res_tx, res_rx) = unbounded::<JobResult>();
        let stop = Arc::new(AtomicBool::new(false));
        let saves_pending = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::with_capacity(n);
        for i in 0..n {
            let (high_rx, low_rx, res_tx) = (high_rx.clone(), low_rx.clone(), res_tx.clone());
            let (gen, save, stop, saves) = (gen.clone(), save.clone(), stop.clone(), saves_pending.clone());
            let handle = std::thread::Builder::new()
                .name(format!("world-worker-{i}"))
                .spawn(move || {
                    loop {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        // Сначала срочные задачи, потом обычные.
                        let job = match high_rx.try_recv() {
                            Ok(j) => j,
                            Err(_) => select! {
                                recv(high_rx) -> j => match j { Ok(j) => j, Err(_) => break },
                                recv(low_rx) -> j => match j { Ok(j) => j, Err(_) => break },
                            },
                        };
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let result = match job {
                            Job::Gen(pos) => {
                                let loaded = save.as_ref().and_then(|s| match s.load_chunk(pos) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        log::error!("загрузка чанка {pos:?}: {e:#}");
                                        None
                                    }
                                });
                                match loaded {
                                    Some(d) => Some(JobResult::Gen(pos, (d, true))),
                                    None => Some(JobResult::Gen(pos, (gen.generate(pos), false))),
                                }
                            }
                            Job::Mesh { pos, nb, version, smooth } => {
                                Some(JobResult::Mesh(pos, version, mesher::build(&nb, smooth)))
                            }
                            Job::Save(pos, data) => {
                                if let Some(s) = &save {
                                    if let Err(e) = s.save_chunk(pos, &data) {
                                        log::error!("сохранение чанка {pos:?}: {e:#}");
                                    }
                                }
                                saves.fetch_sub(1, Ordering::SeqCst);
                                None
                            }
                        };
                        if let Some(r) = result {
                            if res_tx.send(r).is_err() {
                                break;
                            }
                        }
                    }
                })
                .expect("запуск рабочего потока");
            threads.push(handle);
        }
        Self { high: Some(high_tx), low: Some(low_tx), results: res_rx, threads, stop, saves_pending }
    }

    pub fn threads(&self) -> usize {
        self.threads.len()
    }

    pub fn submit_high(&self, job: Job) {
        self.count(&job);
        if let Some(s) = &self.high {
            let _ = s.send(job);
        }
    }

    pub fn submit_low(&self, job: Job) {
        self.count(&job);
        if let Some(s) = &self.low {
            let _ = s.send(job);
        }
    }

    fn count(&self, job: &Job) {
        if matches!(job, Job::Save(..)) {
            self.saves_pending.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn try_recv(&self) -> Option<JobResult> {
        self.results.try_recv().ok()
    }

    /// Ждёт завершения всех отложенных сохранений.
    pub fn flush_saves(&self) {
        let start = std::time::Instant::now();
        while self.saves_pending.load(Ordering::SeqCst) > 0 && start.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

impl Drop for JobPool {
    fn drop(&mut self) {
        self.flush_saves();
        self.stop.store(true, Ordering::Relaxed);
        self.high.take();
        self.low.take();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}
