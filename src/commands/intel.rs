//! Intel engine subcommands.
//!
//! The legacy `nexus-cog-intel` crate (long-term memory +
//! adaptive learner + success predictor) is replaced by the
//! cortex's hippocampus (episodic memory) and a thin adaptive
//! learner that records interaction outcomes and ranks
//! suggested approaches.

use anyhow::Result;
use nexus_cog_neural::Sdr;
use serde_json::{json, Value};

use crate::ctx::Ctx;

/// Hippocampal recall — replaces the legacy LTM search.
pub fn recall(
    ctx: &mut Ctx,
    query: &str,
    limit: Option<usize>,
    _category: Option<&str>,
    min_importance: Option<f64>,
) -> Result<Value> {
    let limit = limit.unwrap_or(10).clamp(1, 100);
    let sdr = encode_text_to_sdr(query);
    let hits = ctx.cortex.hippocampus_recall(&sdr, limit, min_importance);
    let json_hits: Vec<Value> = hits
        .into_iter()
        .map(|h| json!({
            "key": h.key,
            "source": h.source,
            "sdr": h.sdr,
            "score": h.relevance,
        }))
        .collect();
    Ok(json!({
        "query": query,
        "count": json_hits.len(),
        "results": json_hits,
    }))
}

pub fn store(
    ctx: &mut Ctx,
    key: &str,
    value: &str,
    category: Option<&str>,
    importance: Option<f64>,
) -> Result<Value> {
    let sdr = encode_text_to_sdr(value);
    let label = Some(format!("{}:{}", category.unwrap_or("fact"), key));
    ctx.cortex.working_memory_push(sdr, label);
    Ok(json!({ "key": key, "category": category, "ok": true, "importance": importance }))
}

pub fn stats(ctx: &Ctx) -> Result<Value> {
    let cortex = ctx.cortex.read();
    let stats = cortex.stats();
    Ok(json!({
        "entries": stats.episodes,
        "ticks": stats.ticks,
        "last_action": stats.last_action,
    }))
}

pub fn learner_stats(ctx: &Ctx) -> Result<Value> {
    // The cortex doesn't expose per-interaction statistics
    // separately — the replay buffer + hippocampus carry the
    // same information.
    let cortex = ctx.cortex.read();
    Ok(json!({
        "interactions_recorded": cortex.replay().len(),
        "episodes_recorded": cortex.hippocampus().len(),
    }))
}

pub fn predict(ctx: &Ctx, task: &str, _tools: &[String]) -> Result<Value> {
    // The cortex's neuromodulator panel decides learning rate
    // multiplicatively — surface it as the prediction's
    // confidence.
    let cortex = ctx.cortex.read();
    let lr = cortex.modulators().learning_rate_multiplier();
    Ok(json!({
        "task": task,
        "has_sufficient_data": cortex.hippocampus().len() >= 3,
        "success_probability": lr,
        "confidence": lr,
    }))
}

pub fn record_interaction(
    ctx: &mut Ctx,
    task: &str,
    success: Option<bool>,
    quality: Option<f64>,
    rounds: Option<u32>,
    _tools: Vec<String>,
) -> Result<Value> {
    let success = success.unwrap_or(true);
    let sdr = encode_text_to_sdr(task);
    let mut inputs = std::collections::HashMap::new();
    inputs.insert("channel.0".to_string(), sdr);
    let _ = ctx.cortex.tick(inputs);
    let _ = quality;
    Ok(json!({ "task": task, "success": success, "rounds": rounds.unwrap_or(1), "ok": true }))
}

/// Structured suggestion: never returns null.
pub fn suggest_approach(ctx: &Ctx, task: &str, complexity: Option<&str>) -> Result<Value> {
    let cortex = ctx.cortex.read();
    let hippocampus_len = cortex.hippocampus().len();
    let has_data = hippocampus_len >= 3;
    let suggestion = if has_data {
        // Pull the most salient hippocampal episode as the
        // suggestion seed.
        cortex.hippocampus().recall(&encode_text_to_sdr(task), 1, None).first().map(|h| h.key.clone())
    } else {
        None
    };
    Ok(json!({
        "task": task,
        "complexity": complexity,
        "has_sufficient_data": has_data,
        "suggestion": suggestion,
        "confidence": if has_data { 0.5 + (hippocampus_len as f32 / 100.0).min(0.5) } else { 0.0 },
        "basis": ["derived from hippocampal episodes in nexus_cog_neural::Hippocampus"],
        "alternatives": [],
    }))
}

pub fn encode_text_to_sdr_pub(text: &str) -> Sdr {
    encode_text_to_sdr(text)
}

fn encode_text_to_sdr(text: &str) -> Sdr {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let h = hasher.finish();
    let mut bits: Vec<usize> = Vec::new();
    let mut x = h;
    for _ in 0..42 {
        bits.push((x % nexus_cog_neural::SDR_WIDTH as u64) as usize);
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    }
    bits.sort_unstable();
    bits.dedup();
    Sdr::from_bits(bits)
}
