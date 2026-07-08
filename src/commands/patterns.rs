//! Patterns engine.

use anyhow::Result;
use serde_json::Value;

use crate::ctx::Ctx;

pub fn list(ctx: &Ctx) -> Result<Value> {
    let matches = ctx.engines.patterns.match_code("", "rust");
    let _ = matches;
    // There is no `list` method on PatternMatcher — return empty marker.
    Ok(serde_json::json!({
        "hint": "patterns are matched on demand via `match`; use `nexus-cog patterns match <code>`"
    }))
}

pub fn match_code(ctx: &Ctx, code: &str, language: Option<&str>) -> Result<Value> {
    let lang = language.unwrap_or("rust");
    let matches = ctx.engines.patterns.match_code(code, lang);
    Ok(serde_json::to_value(matches)?)
}

pub fn suggest(ctx: &Ctx, task: &str, language: Option<&str>) -> Result<Value> {
    let lang = language.unwrap_or("rust");
    let p = ctx.engines.patterns.suggest_pattern(task, lang);
    Ok(serde_json::to_value(p)?)
}
