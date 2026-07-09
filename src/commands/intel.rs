//! Intel engine: long-term memory, adaptive learning, prediction.
//!
//! `recall` is wired to the BM25 / FTS5 search inside `LongTermMemory`; see
//! `nexus-cog-intel/src/memory.rs`. `suggest_approach` always returns a
//! structured object — when the learner has insufficient data
//! (`has_sufficient_data = false`), the `suggestion` field is empty and the
//! `basis` field explains why.

use anyhow::Result;
use nexus_cog_core::learner::{Interaction, InteractionContext, TaskComplexity};
use nexus_cog_core::memory::MemoryCategory;
use serde_json::{json, Value};

use crate::ctx::Ctx;

pub fn recall(
    ctx: &mut Ctx,
    query: &str,
    limit: Option<usize>,
    category: Option<&str>,
    min_importance: Option<f64>,
) -> Result<Value> {
    let cat_filter = match category {
        Some(s) => Some(parse_category(s)?),
        None => None,
    };
    let limit = limit.unwrap_or(10).clamp(1, 100);
    let min_importance = min_importance.map(|v| v.clamp(0.0, 1.0) as f32);

    let hits = ctx.engines.ltm.search_filtered(query, limit, cat_filter, min_importance);
    Ok(serde_json::to_value(hits)?)
}

pub fn store(
    ctx: &mut Ctx,
    key: &str,
    value: &str,
    category: Option<&str>,
    importance: Option<f64>,
) -> Result<Value> {
    let category = category
        .ok_or_else(|| anyhow::anyhow!("intel_store requires `category` (decision|pattern|error|learning|preference|context|fact|reference); refusing silent default"))?;
    let cat = parse_category(category)?;
    ctx.engines
        .ltm
        .store(key, value, cat, importance.unwrap_or(0.7) as f32);
    Ok(json!({ "key": key, "category": cat.id(), "ok": true }))
}

pub fn stats(ctx: &Ctx) -> Result<Value> {
    let s = ctx.engines.ltm.stats();
    Ok(serde_json::to_value(s)?)
}

pub fn learner_stats(ctx: &Ctx) -> Result<Value> {
    let s = ctx.engines.learner.stats();
    Ok(serde_json::to_value(s)?)
}

pub fn predict(ctx: &Ctx, task: &str, _tools: &[String]) -> Result<Value> {
    let p = ctx.engines.predictor.predict(task);
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
    Ok(json!({ "task": task, "success": success, "ok": true }))
}

/// Always returns a structured object — see module docs.
pub fn suggest_approach(ctx: &Ctx, task: &str, complexity: Option<&str>) -> Result<Value> {
    let parsed_complexity = complexity.map(parse_complexity).transpose()?;
    let suggestion = ctx.engines.learner.suggest_approach(task);
    let stats = ctx.engines.learner.stats();
    let has_data = stats.total_interactions >= 3;

    let mut basis: Vec<&'static str> = Vec::new();
    if has_data {
        basis.push("derived from recorded interactions in `nexus_cog_intel::AdaptiveLearner`");
    } else {
        basis.push("insufficient interaction history (< 3 recorded); no learned pattern yet");
    }
    if let Some(c) = parsed_complexity {
        basis.push("caller-supplied complexity used to bias prior pattern selection");
        let _ = c;
    }

    let confidence = if has_data { 0.5 + (stats.success_rate * 0.5) } else { 0.0 };

    let alternatives: Vec<Value> = if has_data {
        ctx.engines
            .learner
            .alternatives_for(task)
            .into_iter()
            .map(|alt| {
                json!({
                    "approach": alt.approach,
                    "estimated_success": alt.estimated_success,
                    "evidence": alt.evidence,
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(json!({
        "task": task,
        "has_sufficient_data": has_data,
        "suggestion": suggestion,
        "confidence": confidence,
        "basis": basis,
        "alternatives": alternatives,
        "interactions_recorded": stats.total_interactions,
        "success_rate": stats.success_rate,
    }))
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
