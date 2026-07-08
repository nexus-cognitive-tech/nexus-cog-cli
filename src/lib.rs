//! nexus-cog-cli — the enterprise-grade command-line interface for the
//! Nexus Cog cognitive stack.
//!
//! ## Highlights
//!
//! - **All engines as subcommands** — palace, brain, cognitive, causal,
//!   patterns, provenance, intel, intent, antifragile
//! - **Multi-profile config** — named palace profiles in
//!   `~/.config/nexus-cog/config.toml`
//! - **Output formats** — `--format json|yaml|table|plain`, with colours
//!   auto-disabled for non-TTY
//! - **Pipeline mode** — `--stdin` reads input, `--output -` writes to
//!   stdout, lets you chain commands
//! - **REPL** — `nexus-cog repl` opens an interactive shell
//! - **Shell completion** — `nexus-cog completions <bash|zsh|fish>`
//! - **Embedder install** — `nexus-cog embedder install ollama ...`

pub mod cli;
pub mod commands;
pub mod completion;
pub mod config;
pub mod ctx;
pub mod format;
pub mod repl;
