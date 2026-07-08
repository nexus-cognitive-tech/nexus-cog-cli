//! Memory decay.

use anyhow::Result;
use nexus_cog_palace::{DecayConfig, DecayReport};
use serde_json::Value;

use crate::ctx::Ctx;

pub fn apply(ctx: &Ctx, config: &DecayConfig) -> Result<DecayReport> {
    Ok(ctx.palace.apply_decay(config)?)
}

pub fn default_config() -> DecayConfig {
    DecayConfig::default()
}

pub fn report_to_value(r: &DecayReport) -> Value {
    serde_json::json!({
        "items_pruned_by_importance": r.items_pruned_by_importance,
        "items_pruned_by_ttl": r.items_pruned_by_ttl,
        "rooms_pruned": r.rooms_pruned,
    })
}
