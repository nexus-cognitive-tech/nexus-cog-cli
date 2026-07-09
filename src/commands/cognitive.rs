//! Cognitive engine: 6-phase scaffold, thought chains, mirror, response analysis.
//!
//! `think` no longer hands back a free-form prompt and stops — it **executes**
//! the cognitive scaffold against the supplied task/context. If a model
//! `response` is supplied, the scaffold's [`analyze_response`](nexus_cog_cognitive::CognitiveScaffold::analyze_response)
//! runs first to compute real phase-coverage + quality scores. If not, each of
//! the six phases is heuristic-prefilled from the task / context so the caller
//! gets a structured starting point instead of a wall of text.

use anyhow::Result;
use nexus_cog_core::cognitive::{ScaffoldPhase, ScaffoldPrompt};
use nexus_cog_core::thought::{ThoughtNode, ThoughtType};
use nexus_cog_core::Confidence;
use serde_json::{json, Value};

use crate::ctx::Ctx;

pub fn think(ctx: &mut Ctx, task: &str, context: Option<&str>, response: Option<&str>) -> Result<Value> {
    let context = context.unwrap_or("");
    let prompt: ScaffoldPrompt = ctx.engines.cognitive.create_prompt(task, context);

    // ── 1. If a response is provided, analyse it for phase coverage. ──
    let mut phase_presence = None;
    let mut phase_score: f32 = 0.0;
    let mut quality = None;
    let mut suggestions: Vec<String> = Vec::new();
    let mut response_confidence: f32 = 0.0;

    if let Some(resp) = response {
        let analysis = ctx.engines.cognitive.analyze_response(resp);
        phase_presence = Some(analysis.phase_presence);
        phase_score = analysis.phase_score;
        quality = Some(analysis.quality_indicators.clone());
        suggestions = analysis.suggestions.clone();
        response_confidence = analysis.confidence;
    }

    // ── 2. Heuristically pre-fill each phase from the task/context so the
    //       caller has something to start from even without a response. ──
    let phases = prefill_phases(task, context);

    // ── 3. Persist a thought per phase into the chain so it shows up in
    //       subsequent `cognitive_chain_*` / `cognitive_mirror` calls. ──
    let mut added_ids: Vec<String> = Vec::with_capacity(phases.len());
    for (phase, body) in &phases {
        let kind = phase_thought_kind(*phase);
        let id = ctx.engines.thought.add_thought(
            kind,
            format!("[{}] {body}", phase.label()),
            Confidence::new(response_confidence.max(0.6)),
        );
        added_ids.push(id);
    }

    // ── 4. Build the result. ──
    let mut out = json!({
        "task": task,
        "context": context,
        "phases": phases.iter().map(|(p, body)| json!({
            "phase": p.label(),
            "index": p.index(),
            "body": body,
        })).collect::<Vec<_>>(),
        "phase_count": phases.len(),
        "thought_chain_len": ctx.engines.thought.len(),
        "added_thought_ids": added_ids,
        "verification_criteria": prompt.verification_criteria,
    });
    if let Some(pp) = phase_presence {
        out["phase_presence"] = json!(pp);
        out["phase_score"] = json!(phase_score);
    }
    if let Some(q) = quality {
        out["quality_indicators"] = json!(q);
    }
    if !suggestions.is_empty() {
        out["suggestions"] = json!(suggestions);
    }
    if response.is_some() {
        out["response_confidence"] = json!(response_confidence);
    }
    Ok(out)
}

pub fn mirror(ctx: &Ctx, subject: &str, response: &str) -> Result<Value> {
    let r = ctx.engines.mirror.audit_response(subject, response);
    Ok(serde_json::to_value(r)?)
}

pub fn start_chain(ctx: &mut Ctx) -> Result<Value> {
    ctx.engines.thought = nexus_cog_cognitive::ThoughtChain::new();
    Ok(json!({ "chain_started": true, "len": ctx.engines.thought.len() }))
}

pub fn add_thought(
    ctx: &mut Ctx,
    thought_type: &str,
    content: &str,
    confidence: Option<f64>,
) -> Result<Value> {
    let kind = parse_thought_type(thought_type)?;
    let conf = Confidence::new(confidence.unwrap_or(0.8) as f32);
    let id = ctx.engines.thought.add_thought(kind, content, conf);
    Ok(json!({
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

/// Build the six pre-filled phase bodies. Pure deterministic transformation of
/// `task` + `context`; no LLM call. Each body includes explicit constraints /
/// next actions so an agent can pick up where the scaffold left off.
fn prefill_phases(task: &str, context: &str) -> Vec<(ScaffoldPhase, String)> {
    use ScaffoldPhase::*;
    vec![
        (
            Understand,
            format!(
                "Restate the ACTUAL problem behind `{}`. \
                 List explicit constraints (time, budget, compatibility, regulatory) \
                 and define what success looks like as a measurable outcome.",
                task
            ),
        ),
        (
            Analyze,
            format!(
                "Enumerate existing code / patterns / libraries that are relevant. \
                 Identify external dependencies and the failure modes each one introduces. \
                 Note any cross-cutting concern (auth, telemetry, error propagation) that \
                 this task must respect.\n\nContext in scope:\n{}",
                if context.is_empty() { "(no context supplied)" } else { context }
            ),
        ),
        (
            Design,
            "Propose the MINIMAL solution that satisfies the constraints. \
             For each alternative, weigh: complexity, blast radius, testability, \
             reversibility. Pick one and state why over the others in one sentence."
                .into(),
        ),
        (
            Implement,
            "Write code that compiles, handles errors with Result / Option / \
             match, and follows existing patterns. No clever tricks. \
             No TODO / FIXME / HACK markers. Public APIs documented."
                .into(),
        ),
        (
            Verify,
            "Compile, run existing tests, add at least one new test for the \
             happy path and one for the most likely failure mode. \
             Confirm the verification criteria produced by the scaffold."
                .into(),
        ),
        (
            Reflect,
            "Capture one lesson: what was simpler than expected, what was \
             harder? Decide whether anything should be promoted to long-term \
             memory (decision / pattern / error / learning) via `intel_store`."
                .into(),
        ),
    ]
}

fn phase_thought_kind(phase: ScaffoldPhase) -> ThoughtType {
    match phase {
        ScaffoldPhase::Understand => ThoughtType::Problem,
        ScaffoldPhase::Analyze => ThoughtType::Analysis,
        ScaffoldPhase::Design => ThoughtType::Hypothesis,
        ScaffoldPhase::Implement => ThoughtType::Implementation,
        ScaffoldPhase::Verify => ThoughtType::Verification,
        ScaffoldPhase::Reflect => ThoughtType::Reflection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefill_returns_all_six_phases() {
        let phases = prefill_phases("ship the API", "no extra context");
        assert_eq!(phases.len(), 6);
        assert_eq!(phases[0].0, ScaffoldPhase::Understand);
        assert_eq!(phases[5].0, ScaffoldPhase::Reflect);
    }

    #[test]
    fn prefill_embeds_task_and_context() {
        let phases = prefill_phases("kill the bug", "Z-shaped dataset");
        let (_, body) = &phases[0];
        assert!(body.contains("kill the bug"));
        let (_, body) = &phases[1];
        assert!(body.contains("Z-shaped dataset"));
    }
}
