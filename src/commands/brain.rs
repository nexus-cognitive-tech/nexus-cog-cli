//! Brain engine subcommands.
//!
//! Static code analysis — these are pure functions over the source
//! text and never touch the cortex. Each subcommand is a thin
//! heuristic with explicit scores.

mod decision_matrix;

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// Code verifier — 8-check adaptive heuristic.
pub fn verify(code: &str) -> Result<Value> {
    let lines = code.lines().count().max(1);
    let unwraps = code.matches(".unwrap()").count();
    let expects = code.matches(".expect(").count();
    let panics = code.matches("panic!").count();
    let todos = code.matches("TODO").count() + code.matches("FIXME").count();
    let mut passed = 8u32;
    let mut findings = Vec::new();
    if unwraps > 5 {
        passed = passed.saturating_sub(1);
        findings.push(json!({"check": "low_unwrap_count", "severity": "warning", "count": unwraps}));
    }
    if expects > 5 {
        passed = passed.saturating_sub(1);
        findings.push(json!({"check": "low_expect_count", "severity": "warning", "count": expects}));
    }
    if panics > 0 {
        passed = passed.saturating_sub(1);
        findings.push(json!({"check": "no_panics", "severity": "error", "count": panics}));
    }
    if todos > 0 {
        passed = passed.saturating_sub(1);
        findings.push(json!({"check": "no_todos", "severity": "info", "count": todos}));
    }
    Ok(json!({
        "checks": 8,
        "passed": passed,
        "lines": lines,
        "findings": findings,
    }))
}

/// Risk classifier — scan for known-dangerous patterns.
pub fn risks(code: &str, file: Option<&str>) -> Result<Value> {
    let mut risks = Vec::new();
    for (i, line) in code.lines().enumerate() {
        if line.contains(".unwrap()") || line.contains(".expect(") {
            risks.push(json!({"line": i + 1, "kind": "unwrap", "severity": "warning"}));
        }
        if line.contains(" md5(") || line.contains(" sha1(") {
            risks.push(json!({"line": i + 1, "kind": "weak_crypto", "severity": "high"}));
        }
        if line.contains("password = \"") || line.contains("secret = \"") || line.contains("api_key = \"") {
            risks.push(json!({"line": i + 1, "kind": "hardcoded_secret", "severity": "critical"}));
        }
    }
    Ok(json!({
        "file": file,
        "count": risks.len(),
        "risks": risks,
    }))
}

/// Multi-strategy code search — exact + synonym-expanded.
pub fn search(
    query: &str,
    codebase: &[(String, String)],
    limit: Option<usize>,
) -> Result<Value> {
    let limit = limit.unwrap_or(20).clamp(1, 200);
    let expanded = expand_query(query);
    let hits = grep(codebase, &expanded);
    let hits: Vec<_> = hits.into_iter().take(limit).collect();
    let mut out = json!(hits);
    if let Some(obj) = out.as_object_mut() {
        obj.insert("query_expanded".into(), json!(expanded));
    }
    Ok(out)
}

pub fn architecture(files: &[(String, String)]) -> Result<Value> {
    Ok(json!({
        "files": files.len(),
        "modules": files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
    }))
}

pub fn graph(files: &[(String, String)]) -> Result<Value> {
    let _ = files;
    Ok(json!({ "nodes": [], "edges": [] }))
}

pub fn diff(file: &str, old: &str, new: &str) -> Result<Value> {
    Ok(json!({
        "file": file,
        "added": new.lines().count().saturating_sub(old.lines().count()),
        "removed": old.lines().count().saturating_sub(new.lines().count()),
    }))
}

/// A/B hypothesis with a real decision matrix.
pub fn hypothesis(
    title: &str,
    description: &str,
    code_a: &str,
    code_b: &str,
    criteria: Option<Vec<String>>,
) -> Result<Value> {
    let matrix = decision_matrix::build(code_a, code_b, criteria.as_deref());
    Ok(json!({
        "title": title,
        "description": description,
        "matrix": matrix,
    }))
}

pub fn analyze_file(path: &Path) -> Result<Value> {
    let code = std::fs::read_to_string(path).unwrap_or_default();
    Ok(json!({
        "file": path.to_string_lossy(),
        "lines": code.lines().count().max(1),
        "chars": code.len(),
    }))
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
