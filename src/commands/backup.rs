//! Backup: JSON export, SQLite VACUUM INTO.

use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

use crate::ctx::Ctx;

pub fn export_json(ctx: &Ctx, out: &PathBuf) -> Result<Value> {
    ctx.palace.export_json(out)?;
    Ok(serde_json::json!({ "path": out.display().to_string(), "ok": true }))
}

pub fn backup_sqlite(ctx: &Ctx, dst: &PathBuf) -> Result<Value> {
    use nexus_cog_palace::backup_sqlite;
    let info = backup_sqlite(&ctx.db_path, dst)?;
    Ok(serde_json::json!({
        "source": info.source.display().to_string(),
        "destination": info.destination.display().to_string(),
        "size_bytes": info.size_bytes,
    }))
}
