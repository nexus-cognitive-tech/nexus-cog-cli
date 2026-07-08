//! `nexus-cog-mcp` — MCP server built on the same engine handles as the CLI.
//!
//! Every CLI subcommand is also an MCP tool — single source of truth.
//! Run with: `nexus-cog-mcp` (talks MCP over stdio).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::handler::server::ServerHandler;
use rmcp::model::ContentBlock as Content;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, InitializeResult,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, Tool,
};
use rmcp::service::RoleServer;
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use serde_json::{json, Value};
use tokio::sync::RwLock;

use nexus_cog_cli::ctx::Ctx;
use nexus_cog_cli::commands;

#[derive(Parser)]
#[command(name = "nexus-cog-mcp", version, about = "Nexus Cog MCP server")]
struct Args {
    /// Path to the SQLite palace database.
    #[arg(long, env = "NEXUS_COG_DB", default_value = "~/.local/share/nexus-cog/palace.db")]
    db: String,

    /// Palace namespace id.
    #[arg(long, env = "NEXUS_COG_PALACE", default_value = "default")]
    palace: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let db = expand_tilde(&args.db);
    let ctx = Arc::new(RwLock::new(Ctx::open(db, args.palace.clone())?));

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
    ctx: Arc<RwLock<Ctx>>,
}

impl NexusCogMcp {
    pub fn new(ctx: Arc<RwLock<Ctx>>) -> Self {
        Self { ctx }
    }
}

fn empty_schema() -> Arc<serde_json::Map<String, Value>> {
    Arc::new(serde_json::Map::new())
}

fn object_schema(
    properties: &[(&str, &str)],
    required: &[&str],
) -> Arc<serde_json::Map<String, Value>> {
    let mut props = serde_json::Map::new();
    for (k, v_type) in properties {
        let mut p = serde_json::Map::new();
        p.insert("type".into(), json!(*v_type));
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

fn all_tools() -> Vec<Tool> {
    let mut tools = Vec::new();

    // Palace
    tools.push(build_tool("palace_rooms", "List palace rooms.", empty_schema()));
    tools.push(build_tool("palace_summary", "Palace summary.", empty_schema()));
    tools.push(build_tool(
        "palace_add_room",
        "Add a new room to the palace.",
        object_schema(&[("name", "string"), ("type", "string")], &["name", "type"]),
    ));
    tools.push(build_tool(
        "palace_add_item",
        "Add an item to a room.",
        object_schema(
            &[("room_id", "string"), ("key", "string"), ("value", "string")],
            &["room_id", "key", "value"],
        ),
    ));
    tools.push(build_tool(
        "palace_recall",
        "Semantic recall across the palace.",
        object_schema(&[("query", "string")], &["query"]),
    ));
    tools.push(build_tool(
        "palace_connect",
        "Connect two rooms.",
        object_schema(
            &[("from", "string"), ("to", "string"), ("relation", "string")],
            &["from", "to", "relation"],
        ),
    ));

    // Brain
    tools.push(build_tool(
        "brain_verify",
        "Run the 8-check code verifier.",
        object_schema(&[("code", "string")], &["code"]),
    ));
    tools.push(build_tool(
        "brain_risks",
        "Detect security/performance/reliability risks.",
        object_schema(&[("code", "string")], &["code"]),
    ));
    tools.push(build_tool(
        "brain_search",
        "Semantic code search.",
        object_schema(&[("query", "string"), ("code", "string")], &["query", "code"]),
    ));
    tools.push(build_tool(
        "brain_architecture",
        "Architecture analysis of a code corpus.",
        object_schema(&[("code", "string")], &["code"]),
    ));
    tools.push(build_tool(
        "brain_diff",
        "Semantic diff between two versions.",
        object_schema(
            &[("old", "string"), ("new", "string"), ("file", "string")],
            &["old", "new", "file"],
        ),
    ));
    tools.push(build_tool(
        "brain_hypothesis",
        "Propose an A/B hypothesis.",
        object_schema(
            &[
                ("title", "string"),
                ("description", "string"),
                ("code_a", "string"),
                ("code_b", "string"),
            ],
            &["title", "description", "code_a", "code_b"],
        ),
    ));

    // Cognitive
    tools.push(build_tool(
        "cognitive_think",
        "Run the 6-phase cognitive scaffold.",
        object_schema(&[("task", "string")], &["task"]),
    ));
    tools.push(build_tool(
        "cognitive_mirror",
        "Audit a response reasoning.",
        object_schema(
            &[("subject", "string"), ("response", "string")],
            &["subject", "response"],
        ),
    ));
    tools.push(build_tool("cognitive_chain_start", "Start a thought chain.", empty_schema()));
    tools.push(build_tool(
        "cognitive_chain_add",
        "Add a thought to the chain.",
        object_schema(&[("type", "string"), ("content", "string")], &["type", "content"]),
    ));
    tools.push(build_tool(
        "cognitive_analyze_response",
        "Analyze a response.",
        object_schema(&[("response", "string")], &["response"]),
    ));

    // Causal
    tools.push(build_tool(
        "causal_add_node",
        "Add a causal node.",
        object_schema(&[("id", "string"), ("name", "string")], &["id", "name"]),
    ));
    tools.push(build_tool(
        "causal_add_edge",
        "Add a causal edge.",
        object_schema(&[("from", "string"), ("to", "string")], &["from", "to"]),
    ));
    tools.push(build_tool(
        "causal_forward",
        "Forward impact chain.",
        object_schema(&[("entity", "string")], &["entity"]),
    ));
    tools.push(build_tool(
        "causal_backward",
        "Backward impact chain.",
        object_schema(&[("entity", "string")], &["entity"]),
    ));
    tools.push(build_tool(
        "causal_counterfactual",
        "Counterfactual analysis.",
        object_schema(&[("entity", "string")], &["entity"]),
    ));
    tools.push(build_tool(
        "causal_pre_mortem",
        "Pre-mortem analysis.",
        object_schema(&[("entity", "string")], &["entity"]),
    ));
    tools.push(build_tool("causal_dump", "Snapshot the causal graph.", empty_schema()));

    // Patterns
    tools.push(build_tool("patterns_list", "List patterns.", empty_schema()));
    tools.push(build_tool(
        "patterns_match",
        "Match known patterns in code.",
        object_schema(&[("code", "string")], &["code"]),
    ));
    tools.push(build_tool(
        "patterns_suggest",
        "Suggest a pattern for a task.",
        object_schema(&[("task", "string")], &["task"]),
    ));

    // Provenance
    tools.push(build_tool(
        "provenance_record",
        "Record a provenance entry.",
        object_schema(
            &[
                ("artifact", "string"),
                ("origin", "string"),
                ("content", "string"),
                ("source", "string"),
                ("prompt", "string"),
            ],
            &["artifact", "origin", "content", "source", "prompt"],
        ),
    ));
    tools.push(build_tool(
        "provenance_explain",
        "Explain artifact lineage.",
        object_schema(&[("id", "string")], &["id"]),
    ));
    tools.push(build_tool(
        "provenance_search",
        "Search provenance records.",
        object_schema(&[("query", "string")], &["query"]),
    ));

    // Intel
    tools.push(build_tool(
        "intel_recall",
        "Recall long-term memory.",
        object_schema(&[("query", "string")], &["query"]),
    ));
    tools.push(build_tool(
        "intel_store",
        "Store a long-term memory entry.",
        object_schema(&[("key", "string"), ("value", "string")], &["key", "value"]),
    ));
    tools.push(build_tool("intel_stats", "Memory statistics.", empty_schema()));
    tools.push(build_tool("intel_learner_stats", "Learner statistics.", empty_schema()));
    tools.push(build_tool(
        "intel_predict",
        "Predict task success.",
        object_schema(&[("task", "string")], &["task"]),
    ));
    tools.push(build_tool(
        "intel_record_interaction",
        "Record an interaction outcome.",
        object_schema(
            &[("task", "string"), ("success", "string")],
            &["task", "success"],
        ),
    ));
    tools.push(build_tool(
        "intel_suggest_approach",
        "Suggest approach.",
        object_schema(&[("task", "string")], &["task"]),
    ));

    // Intent
    tools.push(build_tool(
        "intent_declare",
        "Declare module intent.",
        object_schema(&[("module", "string"), ("purpose", "string")], &["module", "purpose"]),
    ));
    tools.push(build_tool(
        "intent_check",
        "Check intent against code.",
        object_schema(
            &[("module", "string"), ("current_code", "string")],
            &["module", "current_code"],
        ),
    ));
    tools.push(build_tool(
        "intent_drift",
        "Record intent drift.",
        object_schema(&[("module", "string"), ("observation", "string")], &["module", "observation"]),
    ));
    tools.push(build_tool("intent_index", "Intent preservation index.", empty_schema()));

    // Antifragile
    tools.push(build_tool(
        "antifragile_adversarial",
        "Generate adversarial inputs.",
        empty_schema(),
    ));
    tools.push(build_tool(
        "antifragile_edge",
        "Explore edge cases.",
        object_schema(&[("code", "string"), ("target", "string")], &["code", "target"]),
    ));

    // Backup
    tools.push(build_tool(
        "backup_json",
        "Export palace to JSON.",
        object_schema(&[("out", "string")], &["out"]),
    ));

    // Cross-cutting
    tools.push(build_tool("decay_apply", "Apply memory decay.", empty_schema()));

    tools
}

fn req_str(args: &HashMap<String, Value>, key: &str) -> Result<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("missing required field `{key}`"))
}

#[allow(clippy::too_many_lines)]
async fn dispatch(
    name: &str,
    args: HashMap<String, Value>,
    ctx: Arc<RwLock<Ctx>>,
) -> Result<Value> {
    match name {
        "palace_rooms" => {
            let c = ctx.read().await;
            commands::palace::rooms(&c)
        }
        "palace_summary" => {
            let c = ctx.read().await;
            commands::palace::summary(&c)
        }
        "palace_add_room" => {
            let mut c = ctx.write().await;
            commands::palace::add_room(
                &mut c,
                &req_str(&args, "name")?,
                Some(req_str(&args, "type")?.as_str()),
            )
        }
        "palace_add_item" => {
            let mut c = ctx.write().await;
            let tags = args
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            commands::palace::add_item(
                &mut c,
                &req_str(&args, "room_id")?,
                &req_str(&args, "key")?,
                &req_str(&args, "value")?,
                args.get("confidence").and_then(|v| v.as_f64()),
                tags,
            )
        }
        "palace_recall" => {
            let c = ctx.read().await;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            commands::palace::recall(&c, &req_str(&args, "query")?, limit)
        }
        "palace_connect" => {
            let mut c = ctx.write().await;
            commands::palace::connect(
                &mut c,
                &req_str(&args, "from")?,
                &req_str(&args, "to")?,
                &req_str(&args, "relation")?,
                args.get("strength").and_then(|v| v.as_f64()),
            )
        }
        "brain_verify" => {
            let c = ctx.read().await;
            commands::brain::verify(
                &c,
                &req_str(&args, "code")?,
                args.get("language").and_then(|v| v.as_str()),
            )
        }
        "brain_risks" => {
            let c = ctx.read().await;
            commands::brain::risks(&c, &req_str(&args, "code")?, args.get("file").and_then(|v| v.as_str()))
        }
        "brain_search" => {
            let c = ctx.read().await;
            commands::brain::search(
                &c,
                &req_str(&args, "query")?,
                &[(
                    args.get("path").and_then(|v| v.as_str()).unwrap_or("<inline>").to_string(),
                    req_str(&args, "code")?,
                )],
            )
        }
        "brain_architecture" => {
            let c = ctx.read().await;
            commands::brain::architecture(
                &c,
                &[(
                    args.get("path").and_then(|v| v.as_str()).unwrap_or("<inline>").to_string(),
                    req_str(&args, "code")?,
                )],
            )
        }
        "brain_diff" => {
            let c = ctx.read().await;
            commands::brain::diff(&c, &req_str(&args, "file")?, &req_str(&args, "old")?, &req_str(&args, "new")?)
        }
        "brain_hypothesis" => {
            let c = ctx.read().await;
            commands::brain::hypothesis(
                &c,
                &req_str(&args, "title")?,
                &req_str(&args, "description")?,
                &req_str(&args, "code_a")?,
                &req_str(&args, "code_b")?,
            )
        }
        "cognitive_think" => {
            let mut c = ctx.write().await;
            commands::cognitive::think(&mut c, &req_str(&args, "task")?, args.get("context").and_then(|v| v.as_str()))
        }
        "cognitive_mirror" => {
            let c = ctx.read().await;
            commands::cognitive::mirror(&c, &req_str(&args, "subject")?, &req_str(&args, "response")?)
        }
        "cognitive_chain_start" => {
            let mut c = ctx.write().await;
            commands::cognitive::start_chain(&mut c)
        }
        "cognitive_chain_add" => {
            let mut c = ctx.write().await;
            commands::cognitive::add_thought(
                &mut c,
                &req_str(&args, "type")?,
                &req_str(&args, "content")?,
                args.get("confidence").and_then(|v| v.as_f64()),
            )
        }
        "cognitive_analyze_response" => {
            let c = ctx.read().await;
            commands::cognitive::analyze_response(&c, &req_str(&args, "response")?)
        }
        "causal_add_node" => {
            let mut c = ctx.write().await;
            commands::causal::add_node(
                &mut c,
                &req_str(&args, "id")?,
                &req_str(&args, "name")?,
                args.get("type").and_then(|v| v.as_str()),
                args.get("description").and_then(|v| v.as_str()),
            )
        }
        "causal_add_edge" => {
            let mut c = ctx.write().await;
            commands::causal::add_edge(&mut c, &req_str(&args, "from")?, &req_str(&args, "to")?)
        }
        "causal_forward" => {
            let c = ctx.read().await;
            commands::causal::forward(&c, &req_str(&args, "entity")?)
        }
        "causal_backward" => {
            let c = ctx.read().await;
            commands::causal::backward(&c, &req_str(&args, "entity")?)
        }
        "causal_counterfactual" => {
            let c = ctx.read().await;
            commands::causal::counterfactual(&c, &req_str(&args, "entity")?)
        }
        "causal_pre_mortem" => {
            let c = ctx.read().await;
            commands::causal::pre_mortem(&c, &req_str(&args, "entity")?)
        }
        "causal_dump" => {
            let c = ctx.read().await;
            commands::causal::dump(&c)
        }
        "patterns_list" => {
            let c = ctx.read().await;
            commands::patterns::list(&c)
        }
        "patterns_match" => {
            let c = ctx.read().await;
            commands::patterns::match_code(&c, &req_str(&args, "code")?, args.get("language").and_then(|v| v.as_str()))
        }
        "patterns_suggest" => {
            let c = ctx.read().await;
            commands::patterns::suggest(&c, &req_str(&args, "task")?, args.get("language").and_then(|v| v.as_str()))
        }
        "provenance_record" => {
            let mut c = ctx.write().await;
            commands::provenance::record(
                &mut c,
                &req_str(&args, "artifact")?,
                &req_str(&args, "origin")?,
                &req_str(&args, "content")?,
                &req_str(&args, "source")?,
                &req_str(&args, "prompt")?,
            )
        }
        "provenance_explain" => {
            let c = ctx.read().await;
            commands::provenance::explain(&c, &req_str(&args, "id")?)
        }
        "provenance_search" => {
            let c = ctx.read().await;
            commands::provenance::search(&c, &req_str(&args, "query")?)
        }
        "intel_recall" => {
            let c = ctx.read().await;
            commands::intel::recall(&c, &req_str(&args, "query")?)
        }
        "intel_store" => {
            let mut c = ctx.write().await;
            commands::intel::store(
                &mut c,
                &req_str(&args, "key")?,
                &req_str(&args, "value")?,
                args.get("category").and_then(|v| v.as_str()),
                args.get("importance").and_then(|v| v.as_f64()),
            )
        }
        "intel_stats" => {
            let c = ctx.read().await;
            commands::intel::stats(&c)
        }
        "intel_learner_stats" => {
            let c = ctx.read().await;
            commands::intel::learner_stats(&c)
        }
        "intel_predict" => {
            let c = ctx.read().await;
            let tools: Vec<String> = args
                .get("tools")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            commands::intel::predict(&c, &req_str(&args, "task")?, &tools)
        }
        "intel_record_interaction" => {
            let mut c = ctx.write().await;
            commands::intel::record_interaction(
                &mut c,
                &req_str(&args, "task")?,
                args.get("success").and_then(|v| v.as_bool()).unwrap_or(true),
                args.get("quality").and_then(|v| v.as_f64()),
                args.get("rounds").and_then(|v| v.as_u64()).map(|n| n as u32),
                args.get("tools")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
            )
        }
        "intel_suggest_approach" => {
            let c = ctx.read().await;
            commands::intel::suggest_approach(&c, &req_str(&args, "task")?, args.get("complexity").and_then(|v| v.as_str()))
        }
        "intent_declare" => {
            let mut c = ctx.write().await;
            commands::intent::declare(&mut c, &req_str(&args, "module")?, &req_str(&args, "purpose")?)
        }
        "intent_check" => {
            let c = ctx.read().await;
            commands::intent::check(&c, &req_str(&args, "module")?, &req_str(&args, "current_code")?)
        }
        "intent_drift" => {
            let mut c = ctx.write().await;
            commands::intent::drift(
                &mut c,
                &req_str(&args, "module")?,
                &req_str(&args, "observation")?,
                args.get("drift_score").and_then(|v| v.as_f64()),
            )
        }
        "intent_index" => {
            let mut c = ctx.write().await;
            commands::intent::index(&mut c)
        }
        "antifragile_adversarial" => {
            let c = ctx.read().await;
            commands::antifragile::adversarial(&c, args.get("target").and_then(|v| v.as_str()))
        }
        "antifragile_edge" => {
            let c = ctx.read().await;
            commands::antifragile::edge_cases(&c, &req_str(&args, "code")?, &req_str(&args, "target")?)
        }
        "backup_json" => {
            let c = ctx.read().await;
            let out = std::path::PathBuf::from(req_str(&args, "out")?);
            commands::backup::export_json(&c, &out)
        }
        "decay_apply" => {
            let mut c = ctx.write().await;
            let report = commands::decay::apply(&mut c, &commands::decay::default_config())?;
            Ok(commands::decay::report_to_value(&report))
        }
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
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(server_info)
            .with_instructions(
                "Nexus Cog — cognitive tools for AI agents. \
                 Every CLI subcommand is also an MCP tool.",
            )
    }
}
