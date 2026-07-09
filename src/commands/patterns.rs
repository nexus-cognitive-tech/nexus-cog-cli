//! Patterns engine.

use anyhow::Result;
use serde_json::Value;

use crate::ctx::Ctx;

pub fn list(ctx: &Ctx) -> Result<Value> {
    let patterns = ctx.engines.patterns.patterns();
    Ok(serde_json::json!({
        "count": patterns.len(),
        "patterns": patterns,
    }))
}

pub fn match_code(ctx: &Ctx, code: &str) -> Result<Value> {
    let matches = ctx.engines.patterns.match_code(code, "rust");
    Ok(serde_json::to_value(matches)?)
}

pub fn suggest(ctx: &Ctx, task: &str) -> Result<Value> {
    let p = ctx.engines.patterns.suggest_pattern(task, "rust");
    match p {
        Some(pat) => Ok(serde_json::to_value(pat)?),
        None => Ok(serde_json::json!({
            "suggestion": null,
            "hint": "no pattern matched; provide more task context or call patterns_list to see available patterns",
        })),
    }
}
