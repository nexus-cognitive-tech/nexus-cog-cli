//! Provenance engine.
//!
//! `record` always computes SHA-256 over the supplied `content` and stores it
//! in `content_hash` (so callers cannot accidentally inject empty hashes). When
//! a `parent` ID is provided a `DerivedFrom` edge is added so the lineage graph
//! stays connected.
//!
//! `explain` accepts three `match_mode`s:
//! * `exact` — full record ID only (the historical behaviour);
//! * `prefix` — any unique record whose ID starts with the supplied needle
//!   (handy for chat UIs that truncate UUIDs);
//! * `fuzzy`  — `prefix`, plus a substring search across `artifact` / `origin`
//!   / `content` if no ID match was found.

use anyhow::{Context, Result};
use nexus_cog_core::provenance::{ProvenanceEdgeType, ProvenanceRecord, ProvenanceSource};
use nexus_cog_provenance::ProvenanceExplainer;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::ctx::Ctx;

pub fn record(
    ctx: &mut Ctx,
    artifact: &str,
    origin: &str,
    content: &str,
    source: &str,
    prompt: &str,
    parent: Option<&str>,
    agent: Option<&str>,
    confidence: Option<f64>,
) -> Result<Value> {
    let src = parse_source(source)?;
    let id = format!("rec-{}", uuid::Uuid::new_v4());
    let hash = sha256_hex(content);

    let rec = ProvenanceRecord {
        id: id.clone(),
        artifact: artifact.to_string(),
        source: src,
        origin: origin.to_string(),
        parent: parent.map(String::from),
        children: vec![],
        prompt: prompt.to_string(),
        content: content.to_string(),
        content_hash: hash,
        timestamp: chrono::Utc::now().timestamp(),
        agent: agent.unwrap_or("nexus-cog-cli").to_string(),
        location: None,
        confidence: nexus_cog_core::Confidence::new(confidence.unwrap_or(1.0) as f32),
        metadata: Default::default(),
    };
    ctx.engines.provenance.add_record(rec);

    let mut edge_added = false;
    if let Some(pid) = parent {
        edge_added = ctx
            .engines
            .provenance
            .add_edge_by_id(pid, &id, ProvenanceEdgeType::DerivedFrom)
            .context("linking provenance parent edge")?;
    }

    Ok(json!({
        "id": id,
        "artifact": artifact,
        "source": src.id(),
        "content_hash": format!("sha256:{}", ctx.engines.provenance.get(&id).map(|r| r.content_hash.clone()).unwrap_or_default()),
        "parent_linked": edge_added,
        "ok": true,
    }))
}

pub fn explain(ctx: &Ctx, id: &str, match_mode: Option<&str>) -> Result<Value> {
    let mode = match match_mode.map(str::to_ascii_lowercase).as_deref() {
        Some("exact") => MatchMode::Exact,
        Some("fuzzy") => MatchMode::Fuzzy,
        // Default is prefix: gracefully handle short IDs.
        _ => MatchMode::Prefix,
    };
    let engine = ctx.engines.provenance.clone();
    let explainer = ProvenanceExplainer::new(engine.clone());

    let resolved = resolve_record(&engine, id, mode);
    match resolved {
        Some((record, how)) => Ok(json!({
            "id": record.id,
            "artifact": record.artifact,
            "found": true,
            "match": how,
            "explanation": explainer.format_record_human(&record),
            "lineage": explainer.explain_chain(&record.id),
        })),
        None => Ok(json!({
            "id": id,
            "found": false,
            "match": "none",
            "hint": "no record matched; try `provenance_search` or pass a different match_mode",
        })),
    }
}

pub fn search(ctx: &Ctx, query: &str) -> Result<Value> {
    let engine = ctx.engines.provenance.clone();
    let engine2 = nexus_cog_provenance::ProvenanceQueryEngine::new(engine);
    let r = engine2.search(query);
    let n = r.len();
    Ok(json!({ "query": query, "count": n, "results": r }))
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

#[derive(Debug, Clone, Copy)]
enum MatchMode {
    Exact,
    Prefix,
    Fuzzy,
}

/// Resolve a record from the engine according to the requested mode.
fn resolve_record(
    engine: &nexus_cog_provenance::ProvenanceGraphEngine,
    needle: &str,
    mode: MatchMode,
) -> Option<(ProvenanceRecord, &'static str)> {
    // 1) Exact.
    if let Some(r) = engine.get(needle) {
        return Some((r, "exact"));
    }
    // 2) Prefix.
    if matches!(mode, MatchMode::Prefix | MatchMode::Fuzzy) {
        if let Some(r) = unique_prefix_match(engine, needle) {
            return Some((r, "prefix"));
        }
    }
    // 3) Fuzzy: substring across artifact/origin/content.
    if matches!(mode, MatchMode::Fuzzy) {
        let needle_lower = needle.to_lowercase();
        let mut hits: Vec<ProvenanceRecord> = engine
            .records()
            .into_iter()
            .filter(|r| {
                r.artifact.to_lowercase().contains(&needle_lower)
                    || r.origin.to_lowercase().contains(&needle_lower)
                    || r.content.to_lowercase().contains(&needle_lower)
                    || r.prompt.to_lowercase().contains(&needle_lower)
            })
            .collect();
        if hits.len() == 1 {
            return Some((hits.remove(0), "fuzzy_unique"));
        }
        if !hits.is_empty() {
            // Return the most recent (highest timestamp) but flag ambiguity.
            hits.sort_by_key(|r| std::cmp::Reverse(r.timestamp));
            return Some((hits.remove(0), "fuzzy_ambiguous"));
        }
    }
    None
}

fn unique_prefix_match(
    engine: &nexus_cog_provenance::ProvenanceGraphEngine,
    prefix: &str,
) -> Option<ProvenanceRecord> {
    let mut hits = Vec::new();
    for r in engine.records() {
        if r.id.starts_with(prefix) {
            hits.push(r);
        }
    }
    if hits.len() == 1 {
        Some(hits.remove(0))
    } else {
        None
    }
}

fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_deterministic() {
        assert_eq!(
            sha256_hex("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn sha256_differs_per_input() {
        assert_ne!(sha256_hex("a"), sha256_hex("b"));
    }
}
