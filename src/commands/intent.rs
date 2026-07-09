//! Intent engine: declaration, preservation index, security drift detection.

mod drift_detector;

use anyhow::Result;
use nexus_cog_core::common::Severity;
use nexus_cog_core::intent::{Invariant, IntentDrift, InvariantOperator, ModuleIntent};
use serde_json::{json, Value};

use crate::ctx::Ctx;
use crate::commands::intent::drift_detector as detector;

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
    Ok(json!({ "module": module, "ok": true }))
}

/// Check `current_code` against the declared intent for `module`.
///
/// The check now runs the full [`detector`] pass over the snippet and folds
/// every drift finding into:
///   * `drift`          — typed `IntentDrift` records the persistence layer
///                        understands;
///   * `ipi`            — a real `0..=100` score, computed as
///                        `100 - penalty_score(findings, strict)`;
///   * `confidence`     — inversely proportional to the number of findings.
///
/// `strict` treats `Info`-level findings as full violations.
pub fn check(ctx: &mut Ctx, module: &str, current_code: &str, strict: bool) -> Result<Value> {
    let module = module.to_string();
    let findings = detector::detect(current_code);
    let penalty = detector::penalty_score(&findings, strict);
    let ipi = (100.0 - penalty).round().clamp(0.0, 100.0) as u32;

    // Translate detector findings into IntentDrift records so the persistent
    // index stays compatible with the storage schema.
    let drift: Vec<IntentDrift> = findings
        .iter()
        .map(|f| IntentDrift {
            id: format!("drift-{}", uuid::Uuid::new_v4()),
            module: module.clone(),
            invariant_id: format!("{:?}", f.kind),
            description: f.description.clone(),
            severity: f.severity,
            location: f.line.map(|line| nexus_cog_core::common::Range::line(line, 0, 0)),
            suggested_fix: f.suggested_fix.clone(),
        })
        .collect();

    let confidence = match findings.len() {
        0 => 1.0,
        1..=2 => 0.85,
        3..=5 => 0.6,
        6..=10 => 0.35,
        _ => 0.15,
    };

    // Best-effort compatibility path: still call the underlying invariant
    // evaluator so existing invariant declarations (if any) keep contributing
    // to the drift list.
    let observations = extract_observations(current_code);
    let mut engine_drift: Vec<IntentDrift> = Vec::new();
    let mut engine_ipi: Option<u32> = None;
    if let Some(intent) = ctx.engines.intent_storage.intent(&module) {
        if !intent.invariants.is_empty() {
            match ctx.engines.intent_checker.check(
                &mut ctx.engines.intent_storage,
                &module,
                &observations,
            ) {
                Ok(check) => {
                    engine_drift = check.drift;
                    engine_ipi = Some(check.ipi);
                }
                Err(_) => { /* surface only detector findings below */ }
            }
        }
    }

    let mut all_drift = drift;
    all_drift.extend(engine_drift);

    // Combined IPI: take the minimum of detector- and invariant-based scores
    // so a clean detector pass never masks a violated invariant.
    let final_ipi = match engine_ipi {
        Some(other) => ipi.min(other),
        None => ipi,
    };

    Ok(json!({
        "module": module,
        "ipi": final_ipi,
        "confidence": confidence,
        "strict": strict,
        "findings_count": findings.len(),
        "drift": all_drift,
        "findings": findings,
    }))
}

pub fn drift(ctx: &mut Ctx, module: &str, observation: &str, score: Option<f64>) -> Result<Value> {
    // Drift is now derived automatically from intent_check results. This
    // command remains for backward compatibility — it persists a manual
    // annotation through intent_storage instead of through the
    // (removed) IntentDriftTracker.
    use nexus_cog_core::intent::{IntentCheck, IntentDrift};
    use nexus_cog_core::common::Severity;
    let ipi = ((score.unwrap_or(0.5)) * 100.0) as u32;
    let drift = IntentDrift {
        id: format!("drift-{}", uuid::Uuid::new_v4()),
        module: module.to_string(),
        invariant_id: String::new(),
        description: observation.to_string(),
        severity: Severity::Warning,
        location: None,
        suggested_fix: String::new(),
    };
    let check = IntentCheck {
        id: format!("chk-{}", uuid::Uuid::new_v4()),
        module: module.to_string(),
        ipi,
        invariants: Vec::new(),
        drift: vec![drift],
        timestamp: chrono::Utc::now(),
        confidence: nexus_cog_core::common::Confidence::new(0.5),
    };
    ctx.engines.intent_storage.record_check(check);
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

fn parse_severity(s: String) -> Severity {
    use Severity::*;
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
