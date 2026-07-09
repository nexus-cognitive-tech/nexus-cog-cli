//! Intent engine subcommands.
//!
//! Drives the cortex's amygdala (valence tagging) plus a real
//! security drift detector running on every `check` call.

mod drift_detector;

use anyhow::Result;
use nexus_cog_core::intent::{Invariant, InvariantOperator, ModuleIntent};
use nexus_cog_core::IntentDrift;
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

/// Check `current_code` against the cortex's amygdala valence,
/// plus the security drift detector. Returns a real IPI score
/// and the structured list of findings.
pub fn check(
    ctx: &mut Ctx,
    module: &str,
    current_code: &str,
    strict: bool,
) -> Result<Value> {
    let findings = detector::detect(current_code);
    let penalty = detector::penalty_score(&findings, strict);
    let detector_ipi = (100.0 - penalty).round().clamp(0.0, 100.0) as u32;

    // Drive the cortex with the code as input so its amygdala
    // computes valence too — useful when the caller wants both
    // signals (security findings + emotional tone).
    let mut inputs = std::collections::HashMap::new();
    let sdr = crate::commands::common::encode_text_to_sdr(current_code);
    inputs.insert("channel.0".to_string(), sdr);
    let _ = ctx.cortex.tick(inputs);

    let cortex = ctx.cortex.read();
    let modulator = cortex.modulators();
    let amygdala_signal = (modulator.dopamine.level
        + (1.0 - modulator.serotonin.level)
        + modulator.norepinephrine.level)
        / 3.0;
    let final_ipi = ((detector_ipi as f32) * 0.7 + amygdala_signal * 100.0 * 0.3).round() as u32;
    drop(cortex);

    let drift: Vec<IntentDrift> = findings
        .iter()
        .map(|f| IntentDrift {
            id: format!("drift-{}", uuid::Uuid::new_v4()),
            module: module.to_string(),
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

    Ok(json!({
        "module": module,
        "ipi": final_ipi,
        "confidence": confidence,
        "strict": strict,
        "findings_count": findings.len(),
        "drift": drift,
        "findings": findings,
    }))
}

pub fn drift(
    ctx: &mut Ctx,
    module: &str,
    observation: &str,
    drift_score: Option<f64>,
) -> Result<Value> {
    let score = drift_score.unwrap_or(0.5) as f32;
    let intent = ctx.engines.intent_storage.intent(module);
    let stored_drift = intent.and_then(|i| {
        i.invariants
            .iter()
            .find(|inv| inv.description == observation)
            .map(|inv| inv.op.clone())
    });
    Ok(json!({
        "module": module,
        "observation": observation,
        "drift_score": score,
        "matched_invariant": stored_drift,
    }))
}

pub fn index(ctx: &Ctx) -> Result<Value> {
    let intents: Vec<Value> = ctx
        .engines
        .intent_storage
        .intents()
        .into_iter()
        .map(|i| {
            json!({
                "module": i.module,
                "purpose": i.purpose,
                "invariants": i.invariants.len(),
            })
        })
        .collect();
    Ok(json!({ "modules": intents, "count": intents.len() }))
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

#[allow(dead_code)]
fn _suppress_unused(_inv: Invariant, _op: InvariantOperator) {}
