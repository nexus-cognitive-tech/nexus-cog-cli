//! Brain engine: verification, risk, search, architecture, code graph, semantic diff, hypothesis.
//!
//! `hypothesis` runs a real decision matrix across both approaches — the
//! hypothesis engine's `EstimatedMetrics` are weighted per criterion and the
//! per-criterion winners + an overall winner are reported.
//!
//! `search` expands the query through a synonym map before delegating to
//! `NeuralSearch`, so natural-language prompts like "find the password
//! handling" match code that talks about `pass`, `cred`, `secret`, etc.

mod decision_matrix;

use anyhow::Result;
use nexus_cog_brain::NeuralSearch;
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

/// Multi-strategy search. The query is synonym-expanded (`password` →
/// `pass|pwd|cred|secret|token|hash|bcrypt|argon|scrypt`, …) and then the
/// expanded form is run through `NeuralSearch::search`. `language` is passed
/// through to the verifier-style hints; `limit` is clamped to `[1, 200]`.
pub fn search(
    ctx: &Ctx,
    query: &str,
    codebase: &[(String, String)],
    language: Option<&str>,
    limit: Option<usize>,
) -> Result<Value> {
    let expanded = expand_query(query);
    let limit = limit.unwrap_or(20).clamp(1, 200);
    let hits = ctx.engines.search.search(&expanded, codebase);
    let hits: Vec<_> = hits.into_iter().take(limit).collect();

    // If the expanded query returned nothing useful, fall back to the
    // original query — caller might be using a domain-specific term we don't
    // know about.
    let final_hits = if hits.is_empty() {
        ctx.engines.search.search(query, codebase)
    } else {
        hits
    };
    let mut out = serde_json::to_value(final_hits)?;
    if let Some(obj) = out.as_object_mut() {
        obj.insert("query_expanded".into(), json!(expanded));
        if let Some(lang) = language {
            obj.insert("language".into(), json!(lang));
        }
    }
    Ok(out)
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

/// A/B comparison with a real decision matrix.
pub fn hypothesis(
    ctx: &Ctx,
    title: &str,
    description: &str,
    code_a: &str,
    code_b: &str,
    language: Option<&str>,
    criteria: Option<Vec<String>>,
) -> Result<Value> {
    let mut engine = nexus_cog_brain::HypothesisEngine::new();
    let hyp = engine.propose(title, description, code_a, code_b);

    let matrix = decision_matrix::build(code_a, code_b, criteria.as_deref());
    Ok(json!({
        "id": hyp.id,
        "title": hyp.title,
        "description": hyp.description,
        "status": format!("{:?}", hyp.status),
        "approach_a": hyp.approach_a,
        "approach_b": hyp.approach_b,
        "decision_matrix": matrix,
        "language": language,
        "criteria": criteria,
    }))
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

/// Expand a free-form query through a small synonym map so natural-language
/// prompts (`"find password handling"`) match identifiers we actually use in
/// code (`pass`, `cred`, `secret`, `hash`, `bcrypt` …).
fn expand_query(query: &str) -> String {
    // Order matters: longer phrases first so we don't accidentally re-expand
    // a synonym of a synonym.
    const EXPANSIONS: &[(&str, &[&str])] = &[
        ("password handling", &["password", "passwd", "pwd", "cred", "secret", "hash", "bcrypt", "argon", "scrypt"]),
        ("password", &["passwd", "pwd", "cred", "secret", "hash", "bcrypt", "argon", "scrypt"]),
        ("secret", &["token", "key", "credential", "cred"]),
        ("token", &["bearer", "jwt", "session"]),
        ("authentication", &["auth", "login", "signin", "sign_in"]),
        ("authorisation", &["authorization", "authz", "permission", "role"]),
        ("authorization", &["authorisation", "authz", "permission", "role"]),
        ("sql injection", &["sqli", "concat", "format!", "execute"]),
        ("xss", &["script", "html", "innerhtml", "dangerouslysetinnerhtml"]),
        ("logging", &["log", "tracing", "info!", "warn!", "error!"]),
        ("error handling", &["result", "option", "try", "catch", "except", "match", "?"]),
        ("database", &["db", "sql", "postgres", "mysql", "sqlite", "orm"]),
    ];
    let lower = query.to_lowercase();
    let mut additions: Vec<&str> = Vec::new();
    for (needle, expansions) in EXPANSIONS {
        if lower.contains(needle) {
            additions.extend_from_slice(expansions);
        }
    }
    if additions.is_empty() {
        return query.to_string();
    }
    let mut out = query.to_string();
    out.push(' ');
    out.push_str(&additions.join(" "));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_query_adds_password_synonyms() {
        let e = expand_query("password handling");
        assert!(e.contains("bcrypt"));
        assert!(e.contains("argon"));
        assert!(e.contains("hash"));
    }

    #[test]
    fn expand_query_returns_original_for_unknown_terms() {
        let original = "find widget factory pattern";
        assert_eq!(expand_query(original), original);
    }
}

// Re-export for callers that want to construct a search engine with our
// expanded-query preprocessing.
#[allow(dead_code)]
pub(crate) fn search_engine() -> NeuralSearch {
    NeuralSearch::new()
}
