//! Backup subcommands.

use anyhow::Result;
use std::path::Path;

use crate::ctx::Ctx;

pub fn export_json(ctx: &Ctx, out: &Path) -> Result<()> {
    ctx.palace.export_json(out)?;
    println!("exported to {}", out.display());
    Ok(())
}
