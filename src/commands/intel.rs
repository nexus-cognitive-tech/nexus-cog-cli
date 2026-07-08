//! Intel engine: long-term memory, adaptive learning, prediction.

use anyhow::Result;
use nexus_cog_core::learner::{Interaction, InteractionContext, TaskComplexity};
use nexus_cog_core::memory::MemoryCategory;
use serde_json::Value;

use crate::ctx::Ctx;

pub fn recall(ctx: &mut Ctx, query: &str) -> Result<Value> {
    let hits = ctx.engines.ltm.search(query, 10);
    Ok(serde_json::to_value(hits)?)
}

pub fn store(
    ctx: &mut Ctx,
    key: &str,
    value: &str,
    category: Option<&str>,
    importance: Option<f64>,
) -> Result<Value> {
    // Category is semantic — refuse to store with an implicit default.
    let category = category
        .ok_or_else(|| anyhow::anyhow!("intel_store requires `category` (decision|pattern|error|learning|preference|context|fact|reference); refusing silent default"))?;
    let cat = parse_category(category)?;
    ctx.engines
        .ltm
        .store(key, value, cat, importance.unwrap_or(0.7) as f32);
    Ok(serde_json::json!({ "key": key, "category": cat.id(), "ok": true }))
}

pub fn stats(ctx: &Ctx) -> Result<Value> {
    let s = ctx.engines.ltm.stats();
    Ok(serde_json::to_value(s)?)
}

pub fn learner_stats(ctx: &Ctx) -> Result<Value> {
    let s = ctx.engines.learner.stats();
    Ok(serde_json::to_value(s)?)
}

pub fn predict(ctx: &Ctx, task: &str, tools: &[String]) -> Result<Value> {
    let p = ctx.engines.predictor.predict(task, tools);
    Ok(serde_json::to_value(p)?)
}

pub fn record_interaction(
    ctx: &mut Ctx,
    task: &str,
    success: Option<bool>,
    quality: Option<f64>,
    rounds: Option<u32>,
    tools: Vec<String>,
) -> Result<Value> {
    let success = success
        .ok_or_else(|| anyhow::anyhow!("intel_record_interaction requires `success` (bool or 'true'/'false'); refusing implicit default"))?;
    let interaction = Interaction {
        id: format!("int-{}", uuid::Uuid::new_v4()),
        task: task.to_string(),
        approach: "nexus-cog-cli".into(),
        tools_used: tools,
        rounds: rounds.unwrap_or(1) as usize,
        success,
        quality_score: quality.unwrap_or(0.7) as f32,
        timestamp: chrono::Utc::now().timestamp(),
        context: InteractionContext {
            complexity: TaskComplexity::Moderate,
            ..Default::default()
        },
        output: None,
        error: None,
    };
    ctx.engines.learner.record_interaction(interaction);
    Ok(serde_json::json!({ "task": task, "success": success, "ok": true }))
}

pub fn suggest_approach(ctx: &Ctx, task: &str, complexity: Option<&str>) -> Result<Value> {
    let comp = parse_complexity(complexity.unwrap_or("medium"))?;
    let s = ctx.engines.learner.suggest_approach(task, &comp);
    Ok(serde_json::to_value(s)?)
}

fn parse_category(s: &str) -> Result<MemoryCategory> {
    use MemoryCategory::*;
    Ok(match s.to_lowercase().as_str() {
        "decision" => Decision,
        "pattern" => Pattern,
        "error" => Error,
        "learning" => Learning,
        "preference" => Preference,
        "context" => Context,
        "fact" => Fact,
        "reference" => Reference,
        other => anyhow::bail!("unknown memory category: {other}"),
    })
}

fn parse_complexity(s: &str) -> Result<TaskComplexity> {
    use TaskComplexity::*;
    Ok(match s.to_lowercase().as_str() {
        "trivial" | "easy" => Simple,
        "low" => Simple,
        "medium" => Moderate,
        "high" => Complex,
        "expert" => Expert,
        other => anyhow::bail!("unknown complexity: {other}"),
    })
}
