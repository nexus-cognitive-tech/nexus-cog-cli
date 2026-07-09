//! MCP server mode for the `nexus-cog` binary.
//!
//! Run with: `nexus-cog mcp`. Every CLI subcommand is also an MCP tool —
//! single source of truth via `nexus_cog_cli::commands::*`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock as Content, Implementation,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
    ServerCapabilities, Tool,
};
use rmcp::service::RoleServer;
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::ctx::Ctx;
use crate::commands;

#[derive(Debug, Parser)]
pub struct McpArgs {
    /// Path to the SQLite palace database.
    ///
    /// **Per-workspace by default.** When omitted, the server derives the
    /// path as `<workspace>/.nexus-cog/palace.db`, where `workspace` is the
    /// directory supplied via `--workspace` or, failing that, the current
    /// working directory. The previous behaviour (a single shared file at
    /// `~/.local/share/nexus-cog/palace.db`) leaked state across unrelated
    /// agents and has been removed.
    #[arg(long, env = "NEXUS_COG_DB")]
    pub db: Option<String>,

    /// Workspace root. The database lives under `<workspace>/.nexus-cog/`.
    /// Defaults to the current working directory.
    #[arg(long, env = "NEXUS_COG_WORKSPACE")]
    pub workspace: Option<String>,

    /// Palace namespace id.
    #[arg(long, env = "NEXUS_COG_PALACE", default_value = "default")]
    pub palace: String,
}

pub async fn run(args: McpArgs) -> Result<()> {
    let workspace = args
        .workspace
        .as_deref()
        .map(std::path::Path::new)
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let db = match args.db.as_deref() {
        Some(explicit) => expand_tilde(explicit),
        None => {
            let dir = workspace.join(".nexus-cog");
            std::fs::create_dir_all(&dir).ok();
            dir.join("palace.db")
        }
    };
    tracing::info!(?db, ?workspace, "opening per-workspace palace");
    let ctx = Arc::new(RwLock::new(Ctx::open(db, args.palace)?));
    let server = NexusCogMcp::new(ctx).serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}

fn expand_tilde(s: &str) -> std::path::PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(s)
}

#[derive(Clone)]
pub struct NexusCogMcp {
    pub ctx: Arc<RwLock<Ctx>>,
}

impl NexusCogMcp {
    pub fn new(ctx: Arc<RwLock<Ctx>>) -> Self {
        Self { ctx }
    }
}

fn empty_schema() -> Arc<serde_json::Map<String, Value>> {
    let mut m = serde_json::Map::new();
    m.insert("type".into(), json!("object"));
    Arc::new(m)
}

/// Build a JSON-Schema object from a list of (name, type, description) triples.
/// `required` is the subset that must be present.
fn object_schema(
    properties: &[(&str, &str, &str)],
    required: &[&str],
) -> Arc<serde_json::Map<String, Value>> {
    let mut props = serde_json::Map::new();
    for (k, v_type, desc) in properties {
        let mut p = serde_json::Map::new();
        p.insert("type".into(), json!(*v_type));
        p.insert("description".into(), json!(*desc));
        props.insert((*k).into(), Value::Object(p));
    }
    let mut m = serde_json::Map::new();
    m.insert("type".into(), json!("object"));
    m.insert("properties".into(), Value::Object(props));
    m.insert("required".into(), json!(required));
    Arc::new(m)
}

fn build_tool(name: &str, description: &str, input_schema: Arc<serde_json::Map<String, Value>>) -> Tool {
    let mut t = Tool::default();
    t.name = Cow::Owned(name.into());
    t.description = Some(Cow::Owned(description.into()));
    t.input_schema = input_schema;
    t
}

pub fn all_tools() -> Vec<Tool> {
    let mut tools = Vec::new();

    // ---------- Palace ----------
    tools.push(build_tool("palace_rooms", "List palace rooms.", empty_schema()));
    tools.push(build_tool("palace_summary", "Palace summary (rooms / items / connections).", empty_schema()));
    tools.push(build_tool(
        "palace_add_room",
        "Add a new room to the palace. `type` is one of: concept, pattern, decision, bug, learning, tool, user, project.",
        object_schema(
            &[
                ("name", "string", "Human-readable room name."),
                ("type", "string", "Room type (see description)."),
            ],
            &["name", "type"],
        ),
    ));
    tools.push(build_tool(
        "palace_add_item",
        "Add an item to a room.",
        object_schema(
            &[
                ("room_id", "string", "Target room ID."),
                ("key", "string", "Item key (unique within the room)."),
                ("value", "string", "Item value."),
                ("confidence", "number", "Optional confidence in [0,1]; defaults to 0.5."),
                ("tags", "array", "Optional array of tag strings."),
            ],
            &["room_id", "key", "value"],
        ),
    ));
    tools.push(build_tool(
        "palace_recall",
        "Semantic recall across the palace. Hybrid BM25 + confidence ranking.",
        object_schema(
            &[
                ("query", "string", "Free-form search query."),
                ("limit", "integer", "Max results; defaults to 10."),
                ("min_confidence", "number", "Optional minimum confidence in [0,1]."),
                ("required_tag", "string", "Optional tag that every recalled item must carry."),
                ("room_type", "string", "Optional room-type filter: concept|pattern|decision|bug|learning|tool|user|project."),
            ],
            &["query"],
        ),
    ));
    tools.push(build_tool(
        "palace_connect",
        "Connect two rooms.",
        object_schema(
            &[
                ("from", "string", "Source room ID."),
                ("to", "string", "Target room ID."),
                ("relation", "string", "Relation label (e.g. 'uses')."),
                ("strength", "number", "Optional strength in [0,1]; defaults to 0.5."),
            ],
            &["from", "to", "relation"],
        ),
    ));

    // ---------- Brain ----------
    tools.push(build_tool(
        "brain_verify",
        "Run the 8-check code verifier.",
        object_schema(
            &[
                ("code", "string", "Source code to verify."),
                ("language", "string", "Optional language hint (e.g. 'rust')."),
            ],
            &["code"],
        ),
    ));
    tools.push(build_tool(
        "brain_risks",
        "Detect security/performance/reliability risks.",
        object_schema(
            &[
                ("code", "string", "Source code to analyse."),
                ("file", "string", "Optional filename for context."),
            ],
            &["code"],
        ),
    ));
    tools.push(build_tool(
        "brain_search",
        "Multi-strategy code search (exact + synonym-expanded semantic + structural + behavioral).",
        object_schema(
            &[
                ("query", "string", "Search query."),
                ("code", "string", "Inline source corpus (single virtual file)."),
                ("path", "string", "Optional virtual path for the inline corpus."),
                ("language", "string", "Optional language hint (e.g. 'rust')."),
                ("limit", "integer", "Maximum results to return (default 20, max 200)."),
            ],
            &["query", "code"],
        ),
    ));
    tools.push(build_tool(
        "brain_architecture",
        "Architecture analysis of a code corpus.",
        object_schema(
            &[
                ("code", "string", "Inline source corpus."),
                ("path", "string", "Optional virtual path."),
            ],
            &["code"],
        ),
    ));
    tools.push(build_tool(
        "brain_diff",
        "Semantic diff between two versions.",
        object_schema(
            &[
                ("file", "string", "Filename used in the report."),
                ("old", "string", "Original source."),
                ("new", "string", "New source."),
            ],
            &["file", "old", "new"],
        ),
    ));
    tools.push(build_tool(
        "brain_hypothesis",
        "A/B comparison of two code approaches with a real decision matrix (correctness, complexity, error handling, performance, testability, security, maintainability).",
        object_schema(
            &[
                ("title", "string", "Short title."),
                ("description", "string", "What the hypothesis is about."),
                ("code_a", "string", "Source code of approach A."),
                ("code_b", "string", "Source code of approach B."),
                ("language", "string", "Optional language hint (e.g. 'rust')."),
                ("criteria", "array", "Optional explicit list of decision criteria to evaluate."),
            ],
            &["title", "description", "code_a", "code_b"],
        ),
    ));

    // ---------- Cognitive ----------
    tools.push(build_tool(
        "cognitive_think",
        "Run the 6-phase cognitive scaffold and execute it against the supplied response (if any).",
        object_schema(
            &[
                ("task", "string", "Task description."),
                ("context", "string", "Optional context snippet."),
                ("response", "string", "Optional model response to analyse against the 6 phases."),
            ],
            &["task"],
        ),
    ));
    tools.push(build_tool(
        "cognitive_mirror",
        "Audit a response for completeness and consistency.",
        object_schema(
            &[
                ("subject", "string", "Subject of the response."),
                ("response", "string", "Response text to audit."),
            ],
            &["subject", "response"],
        ),
    ));
    tools.push(build_tool("cognitive_chain_start", "Start a thought chain.", empty_schema()));
    tools.push(build_tool(
        "cognitive_chain_add",
        "Add a thought to the chain.",
        object_schema(
            &[
                ("type", "string", "Thought type: problem|analysis|hypothesis|verification|reflection|decision|implementation|question."),
                ("content", "string", "The thought text."),
                ("confidence", "number", "Optional confidence in [0,1]."),
            ],
            &["type", "content"],
        ),
    ));
    tools.push(build_tool(
        "cognitive_analyze_response",
        "Analyze a response.",
        object_schema(&[("response", "string", "Response text.")], &["response"]),
    ));

    // ---------- Causal ----------
    tools.push(build_tool(
        "causal_add_node",
        "Add a causal node.",
        object_schema(
            &[
                ("id", "string", "Stable node ID."),
                ("name", "string", "Short human name."),
                ("type", "string", "Node type: code_entity|behavior|feature|invariant|assumption|decision|constraint|bug|external_dep."),
                ("description", "string", "Optional longer description."),
            ],
            &["id", "name"],
        ),
    ));
    tools.push(build_tool(
        "causal_add_edge",
        "Add a causal edge.",
        object_schema(
            &[
                ("from", "string", "Source node ID."),
                ("to", "string", "Target node ID."),
                ("type", "string", "Edge type: causes|enables|prevents|mitigates|correlates."),
                ("strength", "number", "Optional strength in [0,1]; defaults to 0.5."),
            ],
            &["from", "to"],
        ),
    ));
    tools.push(build_tool(
        "causal_forward",
        "Forward impact chain.",
        object_schema(&[("entity", "string", "Source node ID.")], &["entity"]),
    ));
    tools.push(build_tool(
        "causal_backward",
        "Backward impact chain.",
        object_schema(&[("entity", "string", "Target node ID.")], &["entity"]),
    ));
    tools.push(build_tool(
        "causal_counterfactual",
        "Counterfactual analysis — derives changes that would have prevented `entity`.",
        object_schema(&[("entity", "string", "Outcome node ID.")], &["entity"]),
    ));
    tools.push(build_tool(
        "causal_pre_mortem",
        "Pre-mortem analysis — failure scenarios derived from the graph.",
        object_schema(&[("entity", "string", "Subject node ID.")], &["entity"]),
    ));
    tools.push(build_tool(
        "causal_blast",
        "Blast-radius analysis for a proposed change.",
        object_schema(&[("entity", "string", "Changed entity.")], &["entity"]),
    ));
    tools.push(build_tool("causal_dump", "Snapshot the causal graph as JSON.", empty_schema()));

    // ---------- Patterns ----------
    tools.push(build_tool("patterns_list", "List known patterns.", empty_schema()));
    tools.push(build_tool(
        "patterns_match",
        "Match known patterns in code.",
        object_schema(
            &[
                ("code", "string", "Source code."),
                ("language", "string", "Optional language; defaults to 'rust'."),
            ],
            &["code"],
        ),
    ));
    tools.push(build_tool(
        "patterns_suggest",
        "Suggest a pattern for a task.",
        object_schema(
            &[
                ("task", "string", "Task description."),
                ("language", "string", "Optional language; defaults to 'rust'."),
            ],
            &["task"],
        ),
    ));

    // ---------- Provenance ----------
    tools.push(build_tool(
        "provenance_record",
        "Record a provenance entry. SHA-256 of `content` is computed and stored in `content_hash`; pass `parent` to link this record into the lineage graph.",
        object_schema(
            &[
                ("artifact", "string", "Artifact identifier (e.g. file path)."),
                ("origin", "string", "Origin (model name, tool name, etc.)."),
                ("content", "string", "Artifact content."),
                ("source", "string", "Source: model_output|tool_execution|test_run|user_input|reasoning|code_extraction|file_load|composition|inference."),
                ("prompt", "string", "Prompt that produced the artifact."),
                ("parent", "string", "Optional parent record ID — adds a 'derived_from' edge in the lineage graph."),
                ("agent", "string", "Optional agent / session name (default 'nexus-cog-cli')."),
                ("confidence", "number", "Optional confidence in [0,1] (default 1.0)."),
            ],
            &["artifact", "origin", "content", "source", "prompt"],
        ),
    ));
    tools.push(build_tool(
        "provenance_explain",
        "Explain artifact lineage. `match_mode` controls how `id` is resolved: 'exact' (default) only matches the full UUID; 'prefix' resolves any unique record whose ID begins with the supplied value (handy for chat UIs that show only the first 8 chars); 'fuzzy' also accepts a substring match across artifact / origin / content if no exact record was found.",
        object_schema(
            &[
                ("id", "string", "Record ID (UUID, prefix, or fuzzy needle)."),
                ("match_mode", "string", "Optional: 'exact' | 'prefix' | 'fuzzy' (default 'prefix')."),
            ],
            &["id"],
        ),
    ));
    tools.push(build_tool(
        "provenance_search",
        "Search provenance records.",
        object_schema(&[("query", "string", "Search query.")], &["query"]),
    ));
    tools.push(build_tool("provenance_snapshot", "Snapshot the full provenance graph.", empty_schema()));

    // ---------- Intel ----------
    tools.push(build_tool(
        "intel_recall",
        "Recall long-term memory. Hybrid BM25 + recency + importance ranking.",
        object_schema(
            &[
                ("query", "string", "Free-form query."),
                ("limit", "integer", "Maximum results to return (default 10, max 100)."),
                ("category", "string", "Optional category filter: decision|pattern|error|learning|preference|context|fact|reference."),
                ("min_importance", "number", "Optional minimum importance in [0,1]."),
            ],
            &["query"],
        ),
    ));
    tools.push(build_tool(
        "intel_store",
        "Store a long-term memory entry. `category` is required (no silent default).",
        object_schema(
            &[
                ("key", "string", "Stable key."),
                ("value", "string", "Stored value."),
                ("category", "string", "decision|pattern|error|learning|preference|context|fact|reference."),
                ("importance", "number", "Optional importance in [0,1]; defaults to 0.7."),
            ],
            &["key", "value", "category"],
        ),
    ));
    tools.push(build_tool("intel_stats", "Memory statistics.", empty_schema()));
    tools.push(build_tool("intel_learner_stats", "Learner statistics.", empty_schema()));
    tools.push(build_tool(
        "intel_predict",
        "Predict task success.",
        object_schema(
            &[
                ("task", "string", "Task description."),
                ("tools", "array", "Optional list of available tool names."),
            ],
            &["task"],
        ),
    ));
    tools.push(build_tool(
        "intel_record_interaction",
        "Record an interaction outcome. `success` is bool or 'true'/'false' string.",
        object_schema(
            &[
                ("task", "string", "Task description."),
                ("success", "boolean", "Whether the task succeeded."),
                ("quality", "number", "Optional quality score in [0,1]."),
                ("rounds", "integer", "Optional rounds taken."),
                ("tools", "array", "Optional list of tools used."),
            ],
            &["task", "success"],
        ),
    ));
    tools.push(build_tool(
        "intel_suggest_approach",
        "Suggest an approach based on historical data. Returns a structured object: { has_sufficient_data, suggestion, confidence, basis: [...], alternatives: [...] }. Always non-null — an empty suggestion is signalled by has_sufficient_data=false.",
        object_schema(
            &[
                ("task", "string", "Task description."),
                ("complexity", "string", "Optional complexity: trivial|low|medium|high|expert."),
            ],
            &["task"],
        ),
    ));

    // ---------- Intent ----------
    tools.push(build_tool(
        "intent_declare",
        "Declare module intent.",
        object_schema(
            &[
                ("module", "string", "Module name."),
                ("purpose", "string", "What the module is supposed to do."),
            ],
            &["module", "purpose"],
        ),
    ));
    tools.push(build_tool(
        "intent_check",
        "Check intent against a code snippet. Runs the security / intent drift detector (hardcoded credentials, weak crypto, JWT bypass, SQL injection, missing error handling, missing authorisation).",
        object_schema(
            &[
                ("module", "string", "Module name (must have been declared)."),
                ("current_code", "string", "Source code snippet to inspect."),
                ("strict", "boolean", "Treat 'info' severity drifts as violations. Default false."),
            ],
            &["module", "current_code"],
        ),
    ));
    tools.push(build_tool(
        "intent_drift",
        "Record intent drift.",
        object_schema(
            &[
                ("module", "string", "Module name."),
                ("observation", "string", "What drifted."),
                ("drift_score", "number", "Optional drift score in [0,1]; defaults to 0.5."),
            ],
            &["module", "observation"],
        ),
    ));
    tools.push(build_tool("intent_index", "Intent preservation index.", empty_schema()));

    // ---------- Antifragile ----------
    tools.push(build_tool(
        "antifragile_adversarial",
        "Generate adversarial inputs.",
        object_schema(
            &[
                ("target", "string", "Optional target description."),
                ("limit", "integer", "Maximum number of inputs to return. Defaults to 50, capped at 500."),
                ("offset", "integer", "Number of inputs to skip from the start (for pagination). Defaults to 0."),
                ("categories", "array", "Optional subset of categories: empty|boundary|special_characters|large|malformed|repetition|injection|numeric_edge|type_confusion|concurrency|fuzz."),
                ("include_fuzz", "boolean", "Include random-byte fuzz inputs (off by default)."),
            ],
            &[],
        ),
    ));
    tools.push(build_tool(
        "antifragile_edge",
        "Explore edge cases for a piece of code.",
        object_schema(
            &[
                ("code", "string", "Source code."),
                ("target", "string", "Target name."),
            ],
            &["code", "target"],
        ),
    ));

    // ---------- Maintenance ----------
    tools.push(build_tool(
        "backup_json",
        "Export palace to JSON.",
        object_schema(&[("out", "string", "Destination file path.")], &["out"]),
    ));
    tools.push(build_tool(
        "decay_apply",
        "Apply memory decay.",
        object_schema(
            &[
                ("half_life_days", "number", "Half-life of an item's importance in days (default 14)."),
                ("min_importance", "number", "Items / rooms whose decayed importance drops below this are pruned (default 0.05)."),
                ("prune_older_than_days", "integer", "Optional hard TTL — items older than this are pruned regardless of importance."),
                ("access_count_boost", "boolean", "Boost importance by access_count (default true)."),
            ],
            &[],
        ),
    ));

    tools
}

fn req_str(args: &HashMap<String, Value>, key: &str) -> Result<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("missing required field `{key}`"))
}

/// Parse a `success`-shaped field that may arrive as either a JSON bool or a
/// JSON string (`"true"`/`"false"`/`"1"`/`"0"`). Returns `None` only when the
/// field is missing entirely, in which case the caller decides on a default.
fn parse_success(v: Option<&Value>) -> Option<bool> {
    match v? {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "y" | "success" | "ok" => Some(true),
            "false" | "0" | "no" | "n" | "fail" | "failure" | "error" => Some(false),
            _ => None,
        },
        Value::Number(n) => n.as_i64().map(|i| i != 0),
        _ => None,
    }
}

#[allow(clippy::too_many_lines)]
pub async fn dispatch(
    name: &str,
    args: HashMap<String, Value>,
    ctx: Arc<RwLock<Ctx>>,
) -> Result<Value> {
    match name {
        "palace_rooms" => { let c = ctx.read().await; commands::palace::rooms(&c) }
        "palace_summary" => { let c = ctx.read().await; commands::palace::summary(&c) }
        "palace_add_room" => { let mut c = ctx.write().await; commands::palace::add_room(&mut c, &req_str(&args, "name")?, Some(req_str(&args, "type")?.as_str())) }
        "palace_add_item" => { let mut c = ctx.write().await;
            let tags = args.get("tags").and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            commands::palace::add_item(&mut c, &req_str(&args, "room_id")?, &req_str(&args, "key")?, &req_str(&args, "value")?, args.get("confidence").and_then(|v| v.as_f64()), tags) }
        "palace_recall" => { let c = ctx.read().await; commands::palace::recall(&c, &req_str(&args, "query")?, args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize, args.get("min_confidence").and_then(|v| v.as_f64()), args.get("required_tag").and_then(|v| v.as_str()), args.get("room_type").and_then(|v| v.as_str())) }
        "palace_connect" => { let mut c = ctx.write().await; commands::palace::connect(&mut c, &req_str(&args, "from")?, &req_str(&args, "to")?, &req_str(&args, "relation")?, args.get("strength").and_then(|v| v.as_f64())) }

        "brain_verify" => { let c = ctx.read().await; commands::brain::verify(&c, &req_str(&args, "code")?, args.get("language").and_then(|v| v.as_str())) }
        "brain_risks" => { let c = ctx.read().await; commands::brain::risks(&c, &req_str(&args, "code")?, args.get("file").and_then(|v| v.as_str())) }
        "brain_search" => { let c = ctx.read().await; commands::brain::search(&c, &req_str(&args, "query")?, &[(args.get("path").and_then(|v| v.as_str()).unwrap_or("<inline>").to_string(), req_str(&args, "code")?)], args.get("language").and_then(|v| v.as_str()), args.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize)) }
        "brain_architecture" => { let c = ctx.read().await; commands::brain::architecture(&c, &[(args.get("path").and_then(|v| v.as_str()).unwrap_or("<inline>").to_string(), req_str(&args, "code")?)]) }
        "brain_diff" => { let c = ctx.read().await; commands::brain::diff(&c, &req_str(&args, "file")?, &req_str(&args, "old")?, &req_str(&args, "new")?) }
        "brain_hypothesis" => { let c = ctx.read().await; let criteria = args.get("criteria").and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()); commands::brain::hypothesis(&c, &req_str(&args, "title")?, &req_str(&args, "description")?, &req_str(&args, "code_a")?, &req_str(&args, "code_b")?, args.get("language").and_then(|v| v.as_str()), criteria) }

        "cognitive_think" => { let mut c = ctx.write().await; commands::cognitive::think(&mut c, &req_str(&args, "task")?, args.get("context").and_then(|v| v.as_str()), args.get("response").and_then(|v| v.as_str())) }
        "cognitive_mirror" => { let c = ctx.read().await; commands::cognitive::mirror(&c, &req_str(&args, "subject")?, &req_str(&args, "response")?) }
        "cognitive_chain_start" => { let mut c = ctx.write().await; commands::cognitive::start_chain(&mut c) }
        "cognitive_chain_add" => { let mut c = ctx.write().await; commands::cognitive::add_thought(&mut c, &req_str(&args, "type")?, &req_str(&args, "content")?, args.get("confidence").and_then(|v| v.as_f64())) }
        "cognitive_analyze_response" => { let c = ctx.read().await; commands::cognitive::analyze_response(&c, &req_str(&args, "response")?) }

        "causal_add_node" => { let mut c = ctx.write().await; commands::causal::add_node(&mut c, &req_str(&args, "id")?, &req_str(&args, "name")?, args.get("type").and_then(|v| v.as_str()), args.get("description").and_then(|v| v.as_str())) }
        "causal_add_edge" => { let mut c = ctx.write().await; commands::causal::add_edge(&mut c, &req_str(&args, "from")?, &req_str(&args, "to")?, args.get("type").and_then(|v| v.as_str()), args.get("strength").and_then(|v| v.as_f64())) }
        "causal_forward" => { let c = ctx.read().await; commands::causal::forward(&c, &req_str(&args, "entity")?) }
        "causal_backward" => { let c = ctx.read().await; commands::causal::backward(&c, &req_str(&args, "entity")?) }
        "causal_counterfactual" => { let c = ctx.read().await; commands::causal::counterfactual(&c, &req_str(&args, "entity")?) }
        "causal_pre_mortem" => { let c = ctx.read().await; commands::causal::pre_mortem(&c, &req_str(&args, "entity")?) }
        "causal_blast" => { let c = ctx.read().await; commands::causal::blast(&c, &req_str(&args, "entity")?) }
        "causal_dump" => { let c = ctx.read().await; commands::causal::dump(&c) }

        "patterns_list" => { let c = ctx.read().await; commands::patterns::list(&c) }
        "patterns_match" => { let c = ctx.read().await; commands::patterns::match_code(&c, &req_str(&args, "code")?, args.get("language").and_then(|v| v.as_str())) }
        "patterns_suggest" => { let c = ctx.read().await; commands::patterns::suggest(&c, &req_str(&args, "task")?, args.get("language").and_then(|v| v.as_str())) }

        "provenance_record" => { let mut c = ctx.write().await; commands::provenance::record(&mut c, &req_str(&args, "artifact")?, &req_str(&args, "origin")?, &req_str(&args, "content")?, &req_str(&args, "source")?, &req_str(&args, "prompt")?, args.get("parent").and_then(|v| v.as_str()), args.get("agent").and_then(|v| v.as_str()), args.get("confidence").and_then(|v| v.as_f64())) }
        "provenance_explain" => { let c = ctx.read().await; commands::provenance::explain(&c, &req_str(&args, "id")?, args.get("match_mode").and_then(|v| v.as_str())) }
        "provenance_search" => { let c = ctx.read().await; commands::provenance::search(&c, &req_str(&args, "query")?) }
        "provenance_snapshot" => { let c = ctx.read().await; commands::provenance::snapshot(&c) }

        "intel_recall" => { let mut c = ctx.write().await; commands::intel::recall(&mut c, &req_str(&args, "query")?, args.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize), args.get("category").and_then(|v| v.as_str()), args.get("min_importance").and_then(|v| v.as_f64())) }
        "intel_store" => { let mut c = ctx.write().await; commands::intel::store(&mut c, &req_str(&args, "key")?, &req_str(&args, "value")?, args.get("category").and_then(|v| v.as_str()), args.get("importance").and_then(|v| v.as_f64())) }
        "intel_stats" => { let c = ctx.read().await; commands::intel::stats(&c) }
        "intel_learner_stats" => { let c = ctx.read().await; commands::intel::learner_stats(&c) }
        "intel_predict" => { let c = ctx.read().await; let tools: Vec<String> = args.get("tools").and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default(); commands::intel::predict(&c, &req_str(&args, "task")?, &tools) }
        "intel_record_interaction" => { let mut c = ctx.write().await; commands::intel::record_interaction(&mut c, &req_str(&args, "task")?, parse_success(args.get("success")), args.get("quality").and_then(|v| v.as_f64()), args.get("rounds").and_then(|v| v.as_u64()).map(|n| n as u32), args.get("tools").and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()) }
        "intel_suggest_approach" => { let c = ctx.read().await; commands::intel::suggest_approach(&c, &req_str(&args, "task")?, args.get("complexity").and_then(|v| v.as_str())) }

        "intent_declare" => { let mut c = ctx.write().await; commands::intent::declare(&mut c, &req_str(&args, "module")?, &req_str(&args, "purpose")?) }
        "intent_check" => { let mut c = ctx.write().await; commands::intent::check(&mut c, &req_str(&args, "module")?, &req_str(&args, "current_code")?, args.get("strict").and_then(|v| v.as_bool()).unwrap_or(false)) }
        "intent_drift" => { let mut c = ctx.write().await; commands::intent::drift(&mut c, &req_str(&args, "module")?, &req_str(&args, "observation")?, args.get("drift_score").and_then(|v| v.as_f64())) }
        "intent_index" => { let c = ctx.read().await; commands::intent::index(&c) }

        "antifragile_adversarial" => { let c = ctx.read().await; commands::antifragile::adversarial(&c, args.get("target").and_then(|v| v.as_str()), args.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize), args.get("offset").and_then(|v| v.as_u64()).map(|n| n as usize), args.get("categories").and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()), args.get("include_fuzz").and_then(|v| v.as_bool())) }
        "antifragile_edge" => { let c = ctx.read().await; commands::antifragile::edge_cases(&c, &req_str(&args, "code")?, &req_str(&args, "target")?) }

        "backup_json" => { let c = ctx.read().await; let out = std::path::PathBuf::from(req_str(&args, "out")?); commands::backup::export_json(&c, &out) }
        "decay_apply" => { let c = ctx.read().await; let mut half_life_days = 14.0; let mut min_importance = 0.05; let mut replay_per_cycle = 32; if let Some(v) = args.get("half_life_days").and_then(|v| v.as_f64()) { half_life_days = v as f32; } if let Some(v) = args.get("min_importance").and_then(|v| v.as_f64()) { min_importance = v as f32; } if let Some(v) = args.get("prune_older_than_days").and_then(|v| v.as_u64()) { replay_per_cycle = v as usize; } let report = commands::decay::apply(&c, half_life_days, min_importance, replay_per_cycle)?; Ok(commands::decay::report_to_value(&report)) }

        _ => Ok(json!({ "error": format!("unknown tool: {name}") })),
    }
}

impl ServerHandler for NexusCogMcp {
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::Error> {
        Ok(ListToolsResult {
            tools: all_tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let name = request.name.to_string();
        let args_map: HashMap<String, Value> = match request.arguments {
            Some(map) => map.into_iter().map(|(k, v)| (k, v.into())).collect(),
            None => HashMap::new(),
        };
        match dispatch(&name, args_map, self.ctx.clone()).await {
            Ok(value) => {
                let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("{e}"))])),
        }
    }

    fn get_info(&self) -> InitializeResult {
        let server_info = Implementation::new("nexus-cog", env!("CARGO_PKG_VERSION"))
            .with_title("Nexus Cog");
        InitializeResult::new(
            ServerCapabilities::builder().enable_tools().build()
        )
            .with_server_info(server_info)
            .with_instructions(
                "Nexus Cog — cognitive tools for AI agents. \
                 Every CLI subcommand is also an MCP tool.",
            )
    }
}
