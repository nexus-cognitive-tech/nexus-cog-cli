//! CLI subcommands grouped by engine.
//!
//! Each engine module exposes a flat list of functions invoked by the
//! top-level clap `Cli` enum. Functions take `&Ctx` and `serde_json::Value`
//! arguments so they can be reused by the REPL and the pipeline runner.

pub mod antifragile;
pub mod backup;
pub mod brain;
pub mod causal;
pub mod cognitive;
pub mod common;
pub mod config;
pub mod decay;
pub mod embedder;
pub mod intent;
pub mod intel;
pub mod palace;
pub mod patterns;
pub mod provenance;
