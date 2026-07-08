//! `nexus-cog` — CLI entry point.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use nexus_cog_cli::commands::{backup, decay, palace};
use nexus_cog_cli::ctx::Ctx;

#[derive(Parser)]
#[command(name = "nexus-cog", version, about = "Nexus Cog CLI")]
struct Cli {
    /// Path to the SQLite palace database.
    #[arg(long, default_value = "~/.local/share/nexus-cog/palace.db")]
    db: PathBuf,

    /// Palace namespace id.
    #[arg(long, default_value = "default")]
    palace: String,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List rooms.
    PalaceRooms,
    /// Show summary.
    PalaceSummary,
    /// Add an item to a room.
    PalaceAddItem {
        #[arg(long)]
        room: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
        #[arg(long, default_value_t = 0.5)]
        confidence: f64,
    },
    /// Export palace to JSON.
    BackupExportJson {
        #[arg(long)]
        out: PathBuf,
    },
    /// Apply memory decay.
    DecayApply,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db = expand_tilde(&cli.db);
    let ctx = Ctx::open(db, &cli.palace)?;
    match cli.cmd {
        Cmd::PalaceRooms => palace::rooms(&ctx),
        Cmd::PalaceSummary => palace::summary(&ctx),
        Cmd::PalaceAddItem { room, key, value, confidence } => {
            palace::add_item(&ctx, &room, &key, &value, confidence as f32)
        }
        Cmd::BackupExportJson { out } => backup::export_json(&ctx, &out),
        Cmd::DecayApply => decay::apply(&ctx),
    }
}

fn expand_tilde(p: &PathBuf) -> PathBuf {
    if let Some(s) = p.to_str() {
        if let Some(rest) = s.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
    }
    p.clone()
}
