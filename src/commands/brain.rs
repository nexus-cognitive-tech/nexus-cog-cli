//! Brain engine: verification, risk, search, architecture, code graph, semantic diff, hypothesis.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::ctx::Ctx;

pub fn verify(ctx: &Ctx, code: &str, language: Option<&str>) -> Result<Value> {
    let hint = language
        .map(|l| format!("language={l}\n"))
        .unwrap_or_default();
    let r = ctx.engines.verifier.verify(code, &hint);
    Ok(serde_json::to_value(r)?)
}

pub fn risks(ctx: &Ctx, code: &str, file: Option<&str>) -> Result<Value> {
    let r = ctx.engines.risk.analyze(code, file.unwrap_or(""));
    Ok(serde_json::to_value(r)?)
}

pub fn search(ctx: &Ctx, query: &str, codebase: &[(String, String)]) -> Result<Value> {
    let hits = ctx.engines.search.search(query, codebase);
    Ok(serde_json::to_value(hits)?)
}

pub fn architecture(ctx: &Ctx, files: &[(String, String)]) -> Result<Value> {
    let report = ctx.engines.architect.analyze(files);
    Ok(serde_json::to_value(report)?)
}

pub fn graph(ctx: &Ctx, files: &[(String, String)]) -> Result<Value> {
    let g = ctx.engines.graph.build();
    let _ = files; // builder collects via add_node/add_edge — left for explicit commands
    Ok(serde_json::to_value(g)?)
}

pub fn diff(ctx: &Ctx, file: &str, old: &str, new: &str) -> Result<Value> {
    let d = ctx.engines.diff.analyze_diff(old, new, file);
    Ok(serde_json::to_value(d)?)
}

pub fn hypothesis(
    ctx: &Ctx,
    title: &str,
    description: &str,
    code_a: &str,
    code_b: &str,
) -> Result<Value> {
    let mut engine = nexus_cog_brain::HypothesisEngine::new();
    let hyp = engine.propose(title, description, code_a, code_b);
    Ok(serde_json::to_value(hyp)?)
}

pub fn analyze_file(ctx: &Ctx, path: &Path) -> Result<Value> {
    let code = std::fs::read_to_string(path)?;
    let file = path.to_string_lossy().to_string();
    let verify = ctx.engines.verifier.verify(&code, "");
    let risks = ctx.engines.risk.analyze(&code, &file);
    let corpus = vec![(file.clone(), code.clone())];
    let architecture = ctx.engines.architect.analyze(&corpus);
    Ok(json!({
        "file": file,
        "verify": verify,
        "risks": risks,
        "architecture": architecture,
    }))
}
