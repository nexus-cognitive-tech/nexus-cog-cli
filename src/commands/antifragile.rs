//! Antifragile verification.

use anyhow::Result;
use nexus_cog_core::antifragile::AdversarialInput;
use serde_json::Value;

use crate::ctx::Ctx;

pub fn adversarial(ctx: &Ctx, target: Option<&str>) -> Result<Value> {
    let _ = target;
    let inputs = ctx.engines.adversarial.generate();
    let n = inputs.len();
    let items: Vec<_> = inputs
        .into_iter()
        .map(|i: AdversarialInput| serde_json::json!({
            "category": format!("{:?}", i.category),
            "description": i.description,
            "value": i.value,
            "rationale": i.rationale,
            "break_likelihood": i.break_likelihood,
        }))
        .collect();
    Ok(serde_json::json!({ "count": n, "inputs": items }))
}

pub fn edge_cases(ctx: &Ctx, code: &str, target: &str) -> Result<Value> {
    let cases = ctx.engines.edge_cases.explore(target, code);
    Ok(serde_json::to_value(cases)?)
}

pub fn robustness(ctx: &Ctx, target: &str, results: Vec<(String, bool)>) -> Result<Value> {
    let n = results.len();
    let broken = results.iter().filter(|(_, b)| *b).count();
    let score = if n == 0 { 1.0 } else { 1.0 - broken as f64 / n as f64 };
    Ok(serde_json::json!({
        "target": target,
        "total": n,
        "broken": broken,
        "score": score,
    }))
}
