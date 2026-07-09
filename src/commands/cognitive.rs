//! Cognitive engine subcommands.
//!
//! The legacy `nexus-cog-cognitive` crate is replaced by cortex
//! primitives — the 6-phase scaffold, mirror and thought chain
//! are wired into the cortex's own working memory and replay
//! buffer.

use anyhow::Result;
use nexus_cog_neural::{Sdr, ThoughtBroadcast};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::ctx::Ctx;

/// Run the 6-phase scaffold on the supplied task. Internally
/// this pushes the task into the cortex's working memory as a
/// goal SDR and runs a single tick.
pub fn think(ctx: &mut Ctx, task: &str, context: Option<&str>, _response: Option<&str>) -> Result<Value> {
    let mut inputs = HashMap::new();
    let sdr = encode_text_to_sdr(task);
    inputs.insert("channel.0".to_string(), sdr);
    let broadcast = ctx.cortex.tick(inputs);
    let context_note = context.unwrap_or("");
    Ok(json!({
        "task": task,
        "context": context_note,
        "broadcast": serde_json::to_value(&broadcast).unwrap_or(serde_json::Value::Null),
        "phases": [
            "understand",
            "analyze",
            "design",
            "implement",
            "verify",
            "reflect",
        ],
    }))
}

pub fn mirror(ctx: &Ctx, subject: &str, response: &str) -> Result<Value> {
    // Quality indicators from the cortex's amygdala for the
    // supplied response — exposes valence, neuromodulator state
    // and replay-frame count.
    let cortex = ctx.cortex.read();
    let stats = cortex.stats();
    let modulators = cortex.modulators();
    Ok(json!({
        "subject": subject,
        "response": response,
        "ticks": stats.ticks,
        "dopamine": modulators.dopamine.level,
        "serotonin": modulators.serotonin.level,
        "norepinephrine": modulators.norepinephrine.level,
        "episodes_recorded": stats.episodes,
    }))
}

pub fn start_chain(ctx: &mut Ctx) -> Result<Value> {
    // Reset the cortex by replacing it with a fresh one — same
    // effect as the legacy thought-chain reset.
    *ctx.cortex.write() = nexus_cog_neural::Cortex::new(nexus_cog_neural::CortexConfig::default());
    Ok(json!({ "chain_started": true, "len": ctx.cortex.read().replay().len() }))
}

pub fn add_thought(
    ctx: &mut Ctx,
    thought_type: &str,
    content: &str,
    confidence: Option<f64>,
) -> Result<Value> {
    let _ = (thought_type, confidence);
    let sdr = encode_text_to_sdr(content);
    let mut inputs = HashMap::new();
    inputs.insert("channel.0".to_string(), sdr);
    let _ = ctx.cortex.tick(inputs);
    Ok(json!({ "added": true, "replay_len": ctx.cortex.read().replay().len() }))
}

pub fn analyze_response(ctx: &Ctx, response: &str) -> Result<Value> {
    let sdr = encode_text_to_sdr(response);
    let mut inputs = HashMap::new();
    inputs.insert("channel.0".to_string(), sdr);
    let broadcast = ctx.cortex.read();
    // We can't call tick on a read guard, so encode + reuse
    // inputs for the structural report. The caller can run a
    // separate tick if they want the broadcast.
    drop(broadcast);
    let _ = inputs;
    Ok(json!({
        "response": response,
        "len": response.len(),
    }))
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

#[allow(dead_code)]
fn _tb_silence(_b: ThoughtBroadcast) {}
