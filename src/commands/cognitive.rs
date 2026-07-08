//! Cognitive engine: 6-phase scaffold, thought chains, mirror, response analysis.

use anyhow::Result;
use nexus_cog_core::thought::{ThoughtNode, ThoughtType};
use nexus_cog_core::Confidence;
use serde_json::Value;

use crate::ctx::Ctx;

pub fn think(ctx: &Ctx, task: &str, context: Option<&str>) -> Result<Value> {
    let prompt = ctx
        .engines
        .cognitive
        .build_task_prompt(task, context.unwrap_or(""));
    Ok(serde_json::json!({
        "task": task,
        "context": context.unwrap_or(""),
        "prompt": prompt,
    }))
}

pub fn mirror(ctx: &Ctx, subject: &str, response: &str) -> Result<Value> {
    let r = ctx.engines.mirror.audit_response(subject, response);
    Ok(serde_json::to_value(r)?)
}

pub fn start_chain(ctx: &mut Ctx) -> Result<Value> {
    ctx.engines.thought = nexus_cog_cognitive::ThoughtChain::new();
    Ok(serde_json::json!({ "chain_started": true, "len": ctx.engines.thought.len() }))
}

pub fn add_thought(ctx: &mut Ctx, thought_type: &str, content: &str, confidence: Option<f64>) -> Result<Value> {
    let kind = parse_thought_type(thought_type)?;
    let conf = Confidence::new(confidence.unwrap_or(0.8) as f32);
    let id = ctx.engines.thought.add_thought(kind, content, conf);
    Ok(serde_json::json!({
        "thought_chain_len": ctx.engines.thought.len(),
        "added_id": id,
    }))
}

pub fn analyze_response(ctx: &Ctx, response: &str) -> Result<Value> {
    let r = ctx.engines.response.analyze(response);
    Ok(serde_json::to_value(r)?)
}

fn parse_thought_type(s: &str) -> Result<ThoughtType> {
    use ThoughtType::*;
    Ok(match s.to_lowercase().as_str() {
        "problem" => Problem,
        "analysis" => Analysis,
        "hypothesis" => Hypothesis,
        "verification" => Verification,
        "reflection" => Reflection,
        "decision" => Decision,
        "branch" => Implementation,
        "question" => Question,
        other => anyhow::bail!("unknown thought_type: {other}"),
    })
}
