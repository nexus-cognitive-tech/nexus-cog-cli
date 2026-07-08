//! Shared CLI context — connects to a palace on disk and exposes operations.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use nexus_cog_palace::{PersistentPalace, SqliteBackend};

pub struct Ctx {
    pub db_path: PathBuf,
    pub palace: PersistentPalace,
}

impl Ctx {
    pub fn open(db_path: PathBuf, palace_id: &str) -> Result<Self> {
        let backend = Arc::new(SqliteBackend::open(&db_path)?);
        let palace = PersistentPalace::new(backend, palace_id);
        palace.load()?;
        Ok(Self { db_path, palace })
    }

    pub fn save(&self) -> Result<()> {
        self.palace.save()?;
        Ok(())
    }
}
