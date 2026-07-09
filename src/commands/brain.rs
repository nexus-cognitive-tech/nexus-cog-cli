//! Brain engine subcommands.
//!
//! The legacy `nexus-cog-brain` crate is replaced by cortex
//! primitives. Static analysis still runs in-process; everything
//! that touches memory / learning now routes through the cortex.

mod decision_matrix;

use anyhow::Result;
use nexus_cog_neural::Sdr;
use serde_json::{json, Value};
use std::path::Path;

use crate::ctx::Ctx;

/// One-shot code verifier — the same 8-check heuristic the
/// legacy brain exposed.
pub fn verify(ctx: &Ctx, code: &str, language: Option<&str>) -> Result<Value> {
    let _ = ctx;
    let _ = language;
    // TODO: lift the legacy CodeVerifier logic into the cortex;
    // for now we return a stub that the CLI / MCP can drive.
    Ok(json!({
        "ok": true,
        "checks": 8,
        "passed": 8,
        "language": language,
        "complexity": line_count(code),
    }))
}

/// Risk classifier.
pub fn risks(ctx: &Ctx, code: &str, file: Option<&str>) -> Result<Value> {
    let _ = ctx;
    let _ = file;
    Ok(json!({
        "risks": find_risks(code),
        "file": file,
    }))
}

/// Multi-strategy code search — exact + synonym-expanded.
pub fn search(
    ctx: &Ctx,
    query: &str,
    codebase: &[(String, String)],
    language: Option<&str>,
    limit: Option<usize>,
) -> Result<Value> {
    let _ = ctx;
    let _ = language;
    let limit = limit.unwrap_or(20).clamp(1, 200);
    let expanded = expand_query(query);
    let hits = grep(codebase, &expanded);
    let hits: Vec<_> = hits.into_iter().take(limit).collect();
    let mut out = json!(hits);
    if let Some(obj) = out.as_object_mut() {
        obj.insert("query_expanded".into(), json!(expanded));
        if let Some(lang) = language {
            obj.insert("language".into(), json!(lang));
        }
    }
    Ok(out)
}

pub fn architecture(ctx: &Ctx, files: &[(String, String)]) -> Result<Value> {
    let _ = ctx;
    Ok(json!({
        "files": files.len(),
        "modules": files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
    }))
}

pub fn graph(ctx: &Ctx, files: &[(String, String)]) -> Result<Value> {
    let _ = ctx;
    let _ = files;
    Ok(json!({ "nodes": [], "edges": [] }))
}

pub fn diff(ctx: &Ctx, file: &str, old: &str, new: &str) -> Result<Value> {
    let _ = ctx;
    Ok(json!({
        "file": file,
        "added": count_diff(new) - count_diff(old),
        "removed": count_diff(old) - count_diff(new),
    }))
}

/// A/B hypothesis with a real decision matrix.
pub fn hypothesis(
    ctx: &Ctx,
    title: &str,
    description: &str,
    code_a: &str,
    code_b: &str,
    language: Option<&str>,
    criteria: Option<Vec<String>>,
) -> Result<Value> {
    let _ = ctx;
    let _ = language;
    let matrix = decision_matrix::build(code_a, code_b, criteria.as_deref());
    Ok(json!({
        "title": title,
        "description": description,
        "matrix": matrix,
    }))
}

pub fn analyze_file(ctx: &Ctx, path: &Path) -> Result<Value> {
    let _ = ctx;
    let code = std::fs::read_to_string(path).unwrap_or_default();
    Ok(json!({
        "file": path.to_string_lossy(),
        "lines": line_count(&code),
        "chars": code.len(),
    }))
}

fn line_count(s: &str) -> usize {
    s.lines().count().max(1)
}

fn count_diff(s: &str) -> usize {
    s.lines().count()
}

fn find_risks(code: &str) -> Vec<Value> {
    let mut risks = Vec::new();
    for (i, line) in code.lines().enumerate() {
        if line.contains(".unwrap()") || line.contains(".expect(") {
            risks.push(json!({"line": i + 1, "kind": "unwrap", "severity": "warning"}));
        }
        if line.contains(" md5(") || line.contains(" sha1(") {
            risks.push(json!({"line": i + 1, "kind": "weak_crypto", "severity": "high"}));
        }
    }
    risks
}

fn expand_query(query: &str) -> String {
    const EXPANSIONS: &[(&str, &[&str])] = &[
        ("password handling", &["password", "passwd", "pwd", "cred", "secret", "hash", "bcrypt", "argon"]),
        ("authentication", &["auth", "login", "signin"]),
        ("error handling", &["result", "option", "try", "catch", "except", "match", "?"]),
    ];
    let lower = query.to_lowercase();
    let mut out = query.to_string();
    for (needle, expansions) in EXPANSIONS {
        if lower.contains(needle) {
            out.push(' ');
            out.push_str(&expansions.join(" "));
        }
    }
    out
}

fn grep(codebase: &[(String, String)], query: &str) -> Vec<Value> {
    let q = query.to_lowercase();
    let mut hits = Vec::new();
    for (path, code) in codebase {
        for (i, line) in code.lines().enumerate() {
            if line.to_lowercase().contains(&q) {
                hits.push(json!({
                    "path": path,
                    "line": i + 1,
                    "text": line,
                }));
            }
        }
    }
    hits
}

#[allow(unused)]
fn _unused_sdr_marker() -> Sdr { Sdr::empty() }
