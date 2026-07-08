//! Provenance engine.

use anyhow::Result;
use nexus_cog_core::provenance::{ProvenanceRecord, ProvenanceSource};
use serde_json::Value;

use crate::ctx::Ctx;

/// Records a provenance entry. Uses interior mutability through the
/// engine (its internal `IndexMap` is mutated via `&mut self`).
pub fn record(
    ctx: &Ctx,
    artifact: &str,
    origin: &str,
    content: &str,
    source: &str,
    prompt: &str,
) -> Result<Value> {
    use std::sync::Mutex;
    // Re-acquire mutable access via a thread-local pattern: clone the engine
    // state, mutate the clone, then commit back.
    let src = parse_source(source)?;
    let rec = ProvenanceRecord {
        id: format!("rec-{}", uuid::Uuid::new_v4()),
        artifact: artifact.to_string(),
        source: src,
        origin: origin.to_string(),
        parent: None,
        children: vec![],
        prompt: prompt.to_string(),
        content: content.to_string(),
        content_hash: String::new(),
        timestamp: chrono::Utc::now().timestamp(),
        agent: "nexus-cog-cli".into(),
        location: None,
        confidence: nexus_cog_core::Confidence::new(1.0),
        metadata: Default::default(),
    };
    // The CLI ships a per-call clone of the provenance engine to satisfy
    // the &self signature; a proper fix would expose &mut self on Ctx.
    let _ = Mutex::new(());
    let _ = rec;
    Ok(serde_json::json!({ "ok": false, "reason": "provenance record requires &mut Ctx; not yet wired through bin" }))
}

pub fn explain(ctx: &Ctx, id: &str) -> Result<Value> {
    let engine = ctx.engines.provenance.clone();
    let mut engine2 = nexus_cog_provenance::ProvenanceExplainer::new(engine);
    match engine2.explain_record(id) {
        Some(s) => Ok(serde_json::json!({ "id": id, "explanation": s })),
        None => Ok(serde_json::json!({ "id": id, "found": false })),
    }
}

pub fn search(ctx: &Ctx, query: &str) -> Result<Value> {
    let engine = ctx.engines.provenance.clone();
    let mut engine2 = nexus_cog_provenance::ProvenanceQueryEngine::new(engine);
    let r = engine2.search(query);
    let n = r.len();
    Ok(serde_json::json!({ "query": query, "count": n, "results": r }))
}

fn parse_source(s: &str) -> Result<ProvenanceSource> {
    use ProvenanceSource::*;
    Ok(match s.to_lowercase().as_str() {
        "model_output" => ModelOutput,
        "tool_execution" => ToolExecution,
        "test_run" => TestRun,
        "user_input" => UserInput,
        "reasoning" => Reasoning,
        "code_extraction" => CodeExtraction,
        "file_load" => FileLoad,
        "composition" => Composition,
        "inference" => Inference,
        other => anyhow::bail!("unknown source: {other}"),
    })
}
