//! Intent engine: declaration, drift tracking, preservation index.

use anyhow::Result;
use nexus_cog_core::common::Severity;
use nexus_cog_core::intent::{Invariant, InvariantOperator, ModuleIntent};
use nexus_cog_intent::IntentDriftEntry;
use serde_json::Value;

use crate::ctx::Ctx;
use Severity as Sev;

pub fn declare(ctx: &mut Ctx, module: &str, purpose: &str) -> Result<Value> {
    let intent = ModuleIntent {
        id: format!("intent-{}", uuid::Uuid::new_v4()),
        module: module.to_string(),
        purpose: purpose.to_string(),
        invariants: Vec::new(),
        tags: Vec::new(),
        author: "nexus-cog-cli".into(),
        declared_at: chrono::Utc::now(),
        last_verified_at: None,
    };
    ctx.engines.intent_storage.declare(intent);
    Ok(serde_json::json!({ "module": module, "ok": true }))
}

pub fn check(ctx: &mut Ctx, module: &str, current_code: &str) -> Result<Value> {
    // Extract trivial observations from the code snippet: identifier tokens,
    // string literals, and numeric literals. This is a heuristic but it's
    // strictly better than the previous stub.
    let observations = extract_observations(current_code);
    match ctx
        .engines
        .intent_checker
        .check(&mut ctx.engines.intent_storage, module, &observations)
    {
        Ok(check) => Ok(serde_json::to_value(check)?),
        Err(e) => Ok(serde_json::json!({
            "module": module,
            "ok": false,
            "reason": e.to_string(),
        })),
    }
}

pub fn drift(ctx: &mut Ctx, module: &str, observation: &str, score: Option<f64>) -> Result<Value> {
    use nexus_cog_core::Severity;
    let score_u32 = (score.unwrap_or(0.5) * 100.0) as u32;
    let entry = IntentDriftEntry {
        module: module.to_string(),
        ipi: score_u32,
        timestamp: chrono::Utc::now(),
        most_severe_drift: observation.to_string(),
        severity: Severity::Warning,
    };
    ctx.engines.drift.record(entry);
    Ok(serde_json::json!({ "ok": true }))
}

pub fn index(ctx: &Ctx) -> Result<Value> {
    let intents = ctx.engines.intent_storage.intents();
    let summary = serde_json::json!({
        "count": intents.len(),
        "modules": intents.iter().map(|i| &i.module).collect::<Vec<_>>(),
    });
    Ok(summary)
}

pub fn declare_with_invariants(
    ctx: &mut Ctx,
    module: &str,
    purpose: &str,
    invariants: Vec<(String, String, String, String, String, Option<&str>)>,
) -> Result<Value> {
    let intent = ModuleIntent {
        id: format!("intent-{}", uuid::Uuid::new_v4()),
        module: module.to_string(),
        purpose: purpose.to_string(),
        invariants: invariants
            .into_iter()
            .map(|(desc, lhs, op, rhs, severity, source)| -> Result<Invariant> {
                Ok(Invariant {
                    id: format!("inv-{}", uuid::Uuid::new_v4()),
                    description: desc,
                    lhs,
                    op: parse_op(&op)?,
                    rhs,
                    severity: parse_severity(severity),
                    holds: true,
                    source: source.map(|s| s.to_string()),
                })
            })
            .collect::<Result<Vec<_>>>()?,
        tags: vec![],
        author: "nexus-cog".into(),
        declared_at: chrono::Utc::now(),
        last_verified_at: None,
    };
    ctx.engines.intent_storage.declare(intent);
    Ok(serde_json::json!({ "module": module, "ok": true }))
}

fn extract_observations(code: &str) -> Vec<(String, String)> {
    // Heuristic: collect (lhs, value) pairs from simple assignments.
    // `let foo = "bar";` → ("foo", "bar")
    // `let n = 42;`      → ("n", "42")
    let mut out = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim().trim_end_matches(';').trim();
        if let Some(rest) = trimmed.strip_prefix("let ") {
            if let Some((lhs, rhs)) = rest.split_once('=') {
                let lhs = lhs.trim().to_string();
                let rhs = rhs.trim().trim_matches('"').to_string();
                if !lhs.is_empty() && !rhs.is_empty() {
                    out.push((lhs, rhs));
                }
            }
        }
    }
    if out.is_empty() {
        // Provide a single dummy observation so the evaluator doesn't short-circuit.
        out.push(("__no_assignment__".into(), code.to_string()));
    }
    out
}

fn parse_op(s: &str) -> Result<InvariantOperator> {
    use InvariantOperator::*;
    Ok(match s.to_lowercase().as_str() {
        "eq" | "==" => Equal,
        "ne" | "!=" => NotEqual,
        "lt" | "<" => Less,
        "gt" | ">" => Greater,
        "le" | "<=" => LessOrEqual,
        "ge" | ">=" => GreaterOrEqual,
        "contains" => Contains,
        "not_contains" | "notcontains" => NotContains,
        other => anyhow::bail!("unknown invariant op: {other}"),
    })
}

fn parse_severity(s: String) -> Sev {
    use Sev::*;
    match s.to_lowercase().as_str() {
        "info" => Info,
        "low" => Low,
        "medium" => Medium,
        "warning" => Warning,
        "high" => High,
        "error" => Error,
        "critical" => Critical,
        _ => Medium,
    }
}
