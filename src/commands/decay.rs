//! Decay subcommands.

use anyhow::Result;

use crate::ctx::Ctx;
use nexus_cog_palace::DecayConfig;

pub fn apply(ctx: &Ctx) -> Result<()> {
    let report = ctx.palace.apply_decay(&DecayConfig::default())?;
    println!(
        "decayed: {} items pruned (importance), {} by ttl, {} rooms",
        report.items_pruned_by_importance,
        report.items_pruned_by_ttl,
        report.rooms_pruned
    );
    Ok(())
}
