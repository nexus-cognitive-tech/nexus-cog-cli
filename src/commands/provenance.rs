//! Provenance engine.

use anyhow::Result;
use nexus_cog_core::provenance::{ProvenanceRecord, ProvenanceSource};
use serde_json::Value;

use crate::ctx::Ctx;

pub fn record(
    ctx: &mut Ctx,
    artifact: &str,
    origin: &str,
    content: &str,
    source: &str,
    prompt: &str,
) -> Result<Value> {
    let src = parse_source(source)?;
    let id = format!("rec-{}", uuid::Uuid::new_v4());
    let rec = ProvenanceRecord {
        id: id.clone(),
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
    ctx.engines.provenance.add_record(rec);
    Ok(serde_json::json!({
        "id": id,
        "artifact": artifact,
        "source": src.id(),
        "ok": true
    }))
}

pub fn explain(ctx: &Ctx, id: &str) -> Result<Value> {
    let engine = ctx.engines.provenance.clone();
    let engine2 = nexus_cog_provenance::ProvenanceExplainer::new(engine.clone());
    match engine2.explain_record(id) {
        Some(s) => Ok(serde_json::json!({ "id": id, "explanation": s, "found": true })),
        None => Ok(serde_json::json!({ "id": id, "found": false })),
    }
}

pub fn search(ctx: &Ctx, query: &str) -> Result<Value> {
    let engine = ctx.engines.provenance.clone();
    let engine2 = nexus_cog_provenance::ProvenanceQueryEngine::new(engine);
    let r = engine2.search(query);
    let n = r.len();
    Ok(serde_json::json!({ "query": query, "count": n, "results": r }))
}

pub fn snapshot(ctx: &Ctx) -> Result<Value> {
    let snap = ctx.engines.provenance.snapshot();
    Ok(serde_json::to_value(snap)?)
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
