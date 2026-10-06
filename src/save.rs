//! Сохранения: изменённые чанки (bincode + lz4) и данные уровня.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::world::chunk::{ChunkData, ChunkPos, CHUNK_AREA, CHUNK_VOL};

/// Версия формата файлов чанков.
const CHUNK_FORMAT: u32 = 1;

pub struct WorldSave {
    pub dir: PathBuf,
}

impl WorldSave {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir.join("chunks")).with_context(|| format!("создание {}", dir.display()))?;
        Ok(Self { dir: dir.to_path_buf() })
    }

    fn chunk_path(&self, pos: ChunkPos) -> PathBuf {
        self.dir.join("chunks").join(format!("c.{}.{}.bin", pos.x, pos.z))
    }

    pub fn save_chunk(&self, pos: ChunkPos, data: &ChunkData) -> Result<()> {
        let raw = bincode::serialize(&(CHUNK_FORMAT, data)).context("сериализация чанка")?;
        let packed = lz4_flex::compress_prepend_size(&raw);
        let path = self.chunk_path(pos);
        // Атомарная запись: во временный файл, затем переименование.
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, packed).with_context(|| format!("запись {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn load_chunk(&self, pos: ChunkPos) -> Result<Option<ChunkData>> {
        let path = self.chunk_path(pos);
        let packed = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("чтение {}", path.display())),
        };
        let raw = lz4_flex::decompress_size_prepended(&packed).context("распаковка чанка")?;
        let (format, data): (u32, ChunkData) = bincode::deserialize(&raw).context("разбор чанка")?;
        if format != CHUNK_FORMAT || data.blocks.len() != CHUNK_VOL || data.biomes.len() != CHUNK_AREA {
            anyhow::bail!("несовместимый формат чанка {}", path.display());
        }
        Ok(Some(data))
    }
}
