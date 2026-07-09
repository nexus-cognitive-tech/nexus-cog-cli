//! Memory decay — implemented as a sleep cycle in the brain-like
//! by the [`CortexConfig`] knobs that drive hippocampal eviction
//! and working-memory decay.

use anyhow::Result;
use nexus_cog_neural::ConsolidationReport;
use serde_json::{json, Value};

use crate::ctx::Ctx;

/// Run one consolidation cycle.
pub fn apply(ctx: &Ctx, _half_life_days: f32, min_importance: f32, replay_per_cycle: usize) -> Result<ConsolidationReport> {
    let report = ctx.cortex.sleep(replay_per_cycle);
    let _ = min_importance;
    Ok(report)
}

pub fn default_config() -> (f32, f32, usize) {
    (14.0, 0.05, 32)
}

pub fn report_to_value(r: &ConsolidationReport) -> Value {
    json!({
        "episodes_replayed": r.episodes_replayed,
        "unique_patterns": r.unique_patterns,
        "avg_target_overlap": r.avg_target_overlap,
        "elapsed_ms": r.elapsed_ms,
    })
}
