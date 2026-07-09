//! Top-level clap CLI definition.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::Verbosity;

use crate::commands::embedder::EmbedderKind;
use crate::format::OutputFormat;

#[derive(Debug, Parser)]
#[command(
    name = "nexus-cog",
    version,
    author,
    about = "Nexus Cog — cognitive CLI for AI agents",
    long_about = "Enterprise command-line interface for the Nexus Cog cognitive stack: \
                  palace, brain, cognitive, causal, patterns, provenance, intel, intent, antifragile.",
    propagate_version = true,
    disable_help_subcommand = true,
    arg_required_else_help = true,
)]
pub struct Cli {
    /// Path to the SQLite palace database. Defaults to
    /// `<workspace>/.nexus-cog/palace.db`; see `--workspace`.
    #[arg(long, global = true, env = "NEXUS_COG_DB")]
    pub db: Option<PathBuf>,

    /// Workspace root for per-project storage. The database lives at
    /// `<workspace>/.nexus-cog/palace.db` unless `--db` overrides it.
    /// Defaults to the current working directory. The old global default
    /// (`~/.local/share/nexus-cog/palace.db`) leaked state across unrelated
    /// agents and has been removed.
    #[arg(long, global = true, env = "NEXUS_COG_WORKSPACE")]
    pub workspace: Option<PathBuf>,

    /// Palace namespace id.
    #[arg(long, global = true, default_value = "default", env = "NEXUS_COG_PALACE")]

    /// Named profile to use (from `~/.config/nexus-cog/config.toml`).
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Output format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Verbosity.
    #[command(flatten)]
    pub verbose: Verbosity,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    // ────── Palace ──────
    #[command(subcommand)]
    Palace(PalaceCmd),

    // ────── Brain ──────
    #[command(subcommand)]
    Brain(BrainCmd),

    // ────── Cognitive ──────
    #[command(subcommand)]
    Cognitive(CognitiveCmd),

    // ────── Causal ──────
    #[command(subcommand)]
    Causal(CausalCmd),

    // ────── Patterns ──────
    #[command(subcommand)]
    Patterns(PatternsCmd),

    // ────── Provenance ──────
    #[command(subcommand)]
    Provenance(ProvenanceCmd),

    // ────── Intel ──────
    #[command(subcommand)]
    Intel(IntelCmd),

    // ────── Intent ──────
    #[command(subcommand)]
    Intent(IntentCmd),

    // ────── Antifragile ──────
    #[command(subcommand)]
    Antifragile(AntifragileCmd),

    // ────── Cross-cutting ──────
    #[command(subcommand)]
    Backup(BackupCmd),

    /// Apply memory decay.
    Decay,

    /// Interactive REPL.
    Repl,

    #[command(subcommand)]
    Config(ConfigCmd),

    #[command(subcommand)]
    Embedder(EmbedderCmd),

    /// Generate shell completion scripts.
    Completions {
        #[arg(value_enum)]
        shell: ShellChoice,
    },

    /// Show version + diagnostics.
    Doctor,

    /// Run as an MCP server (stdio transport).
    Mcp {
        /// Path to the SQLite database (overrides `--workspace`).
        #[arg(long, env = "NEXUS_COG_DB")]
        db: Option<std::path::PathBuf>,

        /// Workspace root — DB lives at `<workspace>/.nexus-cog/palace.db`.
        #[arg(long, env = "NEXUS_COG_WORKSPACE")]
        workspace: Option<std::path::PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShellChoice {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
}

// ──────── Palace ────────

#[derive(Debug, Subcommand)]
pub enum PalaceCmd {
    /// List all rooms.
    Rooms,
    /// Show summary.
    Summary,
    /// Add a room.
    AddRoom {
        /// Room name.
        name: String,
        /// Room type.
        #[arg(long, default_value = "concept")]
        r#type: String,
    },
    /// Add an item to a room.
    AddItem {
        /// Room ID.
        #[arg(long)]
        room: String,
        /// Item key.
        #[arg(long)]
        key: String,
        /// Item value.
        #[arg(long)]
        value: String,
        /// Confidence in [0, 1].
        #[arg(long, default_value_t = 0.8)]
        confidence: f64,
        /// Comma-separated tags.
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Semantic recall.
    Recall {
        /// Search query.
        query: String,
        /// Max results.
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Connect two rooms.
    Connect {
        #[arg(long)] from: String,
        #[arg(long)] to: String,
        #[arg(long)] relation: String,
        #[arg(long, default_value_t = 0.5)] strength: f64,
    },
}

// ──────── Brain ────────

#[derive(Debug, Subcommand)]
pub enum BrainCmd {
    /// Run the 8-check code verifier.
    Verify {
        #[arg(long)] code: String,
    },
    /// Detect risks (unsafe, unwrap, secrets, SQLi, deadlock).
    Risks {
        #[arg(long)] code: String,
        #[arg(long, default_value = "<inline>")] file: String,
    },
    /// Semantic code search (multi-strategy: exact + synonym-expanded).
    Search {
        query: String,
        #[arg(long, default_value = "")] code: String,
        #[arg(long, default_value = "<inline>")] path: String,
        #[arg(long)] limit: Option<usize>,
    },
    /// Architecture analysis.
    Architecture {
        #[arg(long)] code: String,
        #[arg(long, default_value = "<inline>")] path: String,
    },
    /// Build a code dependency graph.
    Graph {
        #[arg(long)] code: String,
        #[arg(long, default_value = "<inline>")] path: String,
    },
    /// Semantic diff between two versions.
    Diff {
        #[arg(long)] old: String,
        #[arg(long)] new: String,
        #[arg(long, default_value = "<inline>")] file: String,
    },
    /// Propose a hypothesis comparing two approaches.
    Hypothesis {
        #[arg(long)] title: String,
        #[arg(long)] description: String,
        #[arg(long)] code_a: String,
        #[arg(long)] code_b: String,
    },
    /// Analyse a file on disk: verify + risks + architecture.
    File {
        /// Path to the file.
        path: PathBuf,
    },
}

// ──────── Cognitive ────────

#[derive(Debug, Subcommand)]
pub enum CognitiveCmd {
    /// Run the 6-phase cognitive scaffold.
    Think {
        task: String,
        #[arg(long, default_value = "")] context: String,
    },
    /// Audit a response for completeness and consistency.
    Mirror {
        subject: String,
        response: String,
    },
    /// Start a fresh thought chain.
    ChainStart,
    /// Add a thought to the active chain.
    ChainAdd {
        r#type: String,
        content: String,
        #[arg(long, default_value_t = 0.8)] confidence: f64,
    },
    /// Analyze a response against the scaffold.
    Analyze {
        response: String,
    },
}

// ──────── Causal ────────

#[derive(Debug, Subcommand)]
pub enum CausalCmd {
    AddNode {
        id: String,
        name: String,
        #[arg(long, default_value = "concept")] r#type: String,
        #[arg(long, default_value = "")] description: String,
    },
    AddEdge {
        from: String,
        to: String,
    },
    /// Forward impact chain: what breaks if I change X?
    Forward {
        entity: String,
    },
    /// Backward impact chain: root-cause analysis.
    Backward {
        entity: String,
    },
    /// Counterfactual analysis.
    Counterfactual {
        entity: String,
    },
    /// Pre-mortem analysis.
    PreMortem {
        entity: String,
    },
    /// Blast-radius analysis: how much of the system is affected by a change.
    Blast {
        entity: String,
    },
    /// Snapshot the graph.
    Dump,
}

// ──────── Patterns ────────

#[derive(Debug, Subcommand)]
pub enum PatternsCmd {
    List,
    /// Match known patterns in code.
    Match { code: String },
    /// Suggest the most relevant pattern.
    Suggest { task: String },
}

// ──────── Provenance ────────

#[derive(Debug, Subcommand)]
pub enum ProvenanceCmd {
    Record {
        artifact: String,
        origin: String,
        content: String,
        source: String,
        prompt: String,
    },
    Explain {
        id: String,
    },
    /// Free-text search.
    Search {
        query: String,
    },
}

// ──────── Intel ────────

#[derive(Debug, Subcommand)]
pub enum IntelCmd {
    /// Hybrid BM25 + FTS5 recall over long-term memory.
    Recall {
        query: String,
        #[arg(long)] limit: Option<usize>,
        #[arg(long)] category: Option<String>,
        #[arg(long)] min_importance: Option<f64>,
    },
    Store {
        key: String,
        value: String,
        #[arg(long, default_value = "learning")] category: String,
        #[arg(long, default_value_t = 0.7)] importance: f64,
    },
    Stats,
    /// Adaptive learner statistics.
    LearnerStats,
    Predict {
        task: String,
        #[arg(long, value_delimiter = ',')] tools: Vec<String>,
    },
    Record {
        task: String,
        #[arg(long)] success: bool,
        #[arg(long, default_value_t = 0.7)] quality: f64,
        #[arg(long, default_value_t = 1)] rounds: u32,
        #[arg(long, value_delimiter = ',')] tools: Vec<String>,
    },
    /// Suggest an approach based on past interactions.
    Suggest {
        task: String,
        #[arg(long, default_value = "")] complexity: String,
    },
}

// ──────── Intent ────────

#[derive(Debug, Subcommand)]
pub enum IntentCmd {
    Declare { module: String, purpose: String },
    Check {
        module: String,
        #[arg(long)] current_code: String,
        #[arg(long, default_value_t = false)] strict: bool,
    },
    Drift {
        module: String,
        observation: String,
        #[arg(long, default_value_t = 0.5)] drift_score: f64,
    },
    Index,
}

// ──────── Antifragile ────────

#[derive(Debug, Subcommand)]
pub enum AntifragileCmd {
    Adversarial {
        #[arg(long, default_value = "general")] target: String,
    },
    Edge {
        code: String,
        target: String,
    },
}

// ──────── Backup ────────

#[derive(Debug, Subcommand)]
pub enum BackupCmd {
    /// Export palace to JSON.
    Json {
        #[arg(long)] out: PathBuf,
    },
    /// Byte-for-byte SQLite backup via VACUUM INTO.
    Sqlite {
        #[arg(long)] dst: PathBuf,
    },
}

// ──────── Config ────────

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Show the resolved config.
    Show,
    /// Create a default config file.
    Init,
    /// Add a named palace profile.
    AddProfile {
        name: String,
        #[arg(long)] db: Option<String>,
    },
}

// ──────── Embedder ────────

#[derive(Debug, Subcommand)]
pub enum EmbedderCmd {
    Info {
        #[arg(value_enum, default_value_t = EmbedderKind::Noop)]
        kind: EmbedderKind,
    },
}

// Re-export of the runtime-arg type so we can wrap into a clap ValueEnum.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormatArg {
    Table,
    Json,
    Yaml,
    Plain,
}

// Bring Args derive into scope for subcommand structs that use it.
#[allow(dead_code)]
fn _ensure_args_in_scope<T: Args>(_: &T) {}
