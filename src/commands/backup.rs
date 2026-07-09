//! Backup: cortex snapshot to JSON, SQLite file copy.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::ctx::Ctx;

/// Dump the cortex state to a JSON file.
pub fn export_json(ctx: &Ctx, out: &PathBuf) -> Result<Value> {
    let snapshot = ctx.cortex.snapshot();
    let json = serde_json::to_string_pretty(&cortex_snapshot_value(&snapshot))?;
    std::fs::write(out, json)?;
    Ok(json!({ "path": out.display().to_string(), "ok": true }))
}

/// Convert the cortex snapshot into a serialisable structure.
fn cortex_snapshot_value(c: &nexus_cog_neural::Cortex) -> Value {
    json!({
        "stats": c.stats(),
        "modulators": c.modulators(),
        "hierarchy_len": c.hierarchy().len(),
        "regions": c.hierarchy().region_ids(),
        "replay_frames": c.replay().len(),
        "working_memory_filled": c.working_memory().n_filled(),
        "episodes": c.hippocampus().len(),
    })
}

/// SQLite file copy.
pub fn backup_sqlite(ctx: &Ctx, dst: &PathBuf) -> Result<Value> {
    std::fs::copy(&ctx.db_path, dst)?;
    let metadata = std::fs::metadata(dst)?;
    Ok(json!({
        "source": ctx.db_path.display().to_string(),
        "destination": dst.display().to_string(),
        "size_bytes": metadata.len(),
    }))
}
