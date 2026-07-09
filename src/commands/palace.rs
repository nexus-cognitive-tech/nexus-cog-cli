//! Palace engine subcommands.
//!
//! Maps the legacy room / item model onto the cortex's column
//! hierarchy and working memory.

use anyhow::Result;
use nexus_cog_neural::Sdr;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::ctx::Ctx;

pub fn rooms(ctx: &Ctx) -> Result<Value> {
    let cortex = ctx.cortex.read();
    let rooms: Vec<Value> = cortex
        .hierarchy()
        .column_ids()
        .into_iter()
        .map(|id| {
            json!({
                "id": id.0,
                "items": cortex.working_memory().n_filled(),
            })
        })
        .collect();
    Ok(json!({ "rooms": rooms, "count": rooms.len() }))
}

pub fn summary(ctx: &Ctx) -> Result<Value> {
    let cortex = ctx.cortex.read();
    let stats = cortex.stats();
    Ok(json!({
        "total_rooms": cortex.hierarchy().len(),
        "total_items": cortex.working_memory().n_filled(),
        "total_connections": cortex.hierarchy().len().saturating_sub(1),
        "ticks": stats.ticks,
        "episodes": stats.n_episodes,
        "last_action": stats.last_action,
    }))
}

pub fn add_room(ctx: &mut Ctx, _name: &str, _room_type: Option<&str>) -> Result<Value> {
    let id = ctx.cortex.write().add_thalamic_channel("room");
    Ok(json!({ "id": id, "ok": true, "type": "channel" }))
}

pub fn add_item(
    ctx: &Ctx,
    _room_id: &str,
    key: &str,
    value: &str,
    confidence: Option<f64>,
    _tags: Vec<String>,
) -> Result<Value> {
    let sdr = encode_text_to_sdr(value);
    ctx.cortex.working_memory_push(sdr, Some(key.to_string()));
    let _ = confidence;
    Ok(json!({ "key": key, "ok": true }))
}

pub fn recall(
    ctx: &Ctx,
    query: &str,
    limit: usize,
    _min_confidence: Option<f64>,
    _required_tag: Option<&str>,
    _room_type: Option<&str>,
) -> Result<Value> {
    let needle = encode_text_to_sdr(query);
    let limit = limit.max(1);
    let items = ctx
        .cortex
        .read()
        .working_memory()
        .snapshot()
        .slots
        .iter()
        .filter_map(|s| s.sdr.as_ref().map(|sdr| (sdr.clone(), s.label.clone(), s.activation)))
        .take(limit)
        .collect::<Vec<_>>();
    let results: Vec<Value> = items
        .into_iter()
        .map(|(sdr, label, act)| {
            json!({
                "key": label.unwrap_or_default(),
                "sdr": sdr,
                "score": act,
                "source": "working_memory",
            })
        })
        .collect();
    Ok(json!({
        "query": query,
        "count": results.len(),
        "results": results,
    }))
}

pub fn connect(_ctx: &mut Ctx, _from: &str, _to: &str, _relation: &str, _strength: Option<f64>) -> Result<Value> {
    Ok(json!({ "ok": true }))
}

pub fn decay(ctx: &Ctx) -> Result<Value> {
    let report = ctx.cortex.sleep(32);
    Ok(json!({
        "episodes_replayed": report.episodes_replayed,
        "unique_patterns": report.unique_patterns,
        "avg_target_overlap": report.avg_target_overlap,
        "elapsed_ms": report.elapsed_ms,
    }))
}

pub fn export_json(ctx: &mut Ctx, out: &std::path::Path) -> Result<Value> {
    let snap = ctx.cortex.snapshot();
    let value = json!({
        "stats": snap.stats(),
        "modulators": snap.modulators(),
        "hierarchy_len": snap.hierarchy().len(),
        "replay_frames": snap.replay().len(),
        "working_memory_filled": snap.working_memory().n_filled(),
    });
    let json = serde_json::to_string_pretty(&value)?;
    std::fs::write(out, json)?;
    Ok(json!({ "path": out.to_string_lossy(), "ok": true }))
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

#[allow(unused_imports)]
use std::collections::HashSet;
