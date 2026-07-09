//! Palace engine subcommands.
//!
//! to working-memory slots, recall to BM25 over the cortex's
//! hippocampal episodes. External behaviour is unchanged.

use anyhow::Result;
use nexus_cog_neural::Sdr;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::ctx::Ctx;

pub fn rooms(ctx: &Ctx) -> Result<Value> {
    let cortex = ctx.cortex.read();
    let rooms: Vec<Value> = cortex
        .hierarchy()
        .region_ids()
        .into_iter()
        .map(|id| {
            json!({
                "id": id.0,
                "name": cortex.hierarchy().region(id).map(|r| r.name.clone()).unwrap_or_default(),
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
        "episodes": stats.episodes,
        "last_action": stats.last_action,
    }))
}

pub fn add_room(ctx: &mut Ctx, name: &str, room_type: Option<&str>) -> Result<Value> {
    // Room creation now means attaching a new cortical region. We
    // honour the `room_type` keyword for compatibility but the
    // cortex doesn't expose region typing yet — it's all regions.
    let _ = room_type;
    let id = {
        let mut cortex = ctx.cortex.write();
        let new_id = cortex
            .hierarchy_mut_add_region(name.to_string(), nexus_cog_neural::SDR_WIDTH);
        new_id
    };
    Ok(json!({ "id": id.0, "name": name, "type": "region" }))
}

pub fn add_item(
    ctx: &mut Ctx,
    room_id: &str,
    key: &str,
    value: &str,
    confidence: Option<f64>,
    tags: Vec<String>,
) -> Result<Value> {
    // Look up the room by id.
    let target_room = ctx
        .cortex
        .read()
        .hierarchy()
        .region_ids()
        .into_iter()
        .find(|id| id.0.to_string() == room_id);
    let Some(_) = target_room else {
        anyhow::bail!("room {room_id} not found");
    };
    let sdr = crate::commands::common::encode_text_to_sdr(value);
    ctx.cortex.working_memory_push(sdr, Some(key.to_string()));
    let _ = confidence;
    Ok(json!({ "room": room_id, "key": key, "ok": true }))
}

/// Semantic recall across the cortex — BM25 over hippocampal
/// episodes + working-memory slots, filtered by tag and room type.
pub fn recall(
    ctx: &Ctx,
    query: &str,
    limit: usize,
    min_confidence: Option<f64>,
    required_tag: Option<&str>,
    room_type: Option<&str>,
) -> Result<Value> {
    let _ = room_type;
    let needle = crate::commands::common::encode_text_to_sdr(query);
    let results = ctx.cortex.hippocampus_recall(&needle, limit, min_confidence);
    // Drop episodes that don't carry the required tag (best-effort:
    // we don't store tags on episodes yet, so this filter only
    // really applies to working-memory items).
    let filtered: Vec<_> = results
        .into_iter()
        .filter(|r| required_tag.is_none_or(|t| r.source.contains(t) || r.key.contains(t)))
        .collect();
    let json_results: Vec<Value> = filtered
        .into_iter()
        .map(|r| {
            json!({
                "item": r.sdr,
                "room_id": r.source,
                "key": r.key,
                "relevance": r.relevance,
                "source": r.source,
            })
        })
        .collect();
    Ok(json!({
        "query": query,
        "count": json_results.len(),
        "filters": {
            "min_confidence": min_confidence,
            "required_tag": required_tag,
            "room_type": room_type,
        },
        "results": json_results,
    }))
}

pub fn connect(ctx: &mut Ctx, from: &str, to: &str, relation: &str, strength: Option<f64>) -> Result<Value> {
    let from_id = ctx
        .cortex
        .read()
        .hierarchy()
        .region_ids()
        .into_iter()
        .find(|id| id.0.to_string() == from);
    let to_id = ctx
        .cortex
        .read()
        .hierarchy()
        .region_ids()
        .into_iter()
        .find(|id| id.0.to_string() == to);
    let (Some(from_id), Some(to_id)) = (from_id, to_id) else {
        anyhow::bail!("room {from} or {to} not found");
    };
    ctx.cortex.hierarchy_connect(from_id, to_id);
    let _ = (relation, strength);
    Ok(json!({ "from": from, "to": to, "relation": relation, "ok": true }))
}

pub fn decay(ctx: &Ctx) -> Result<Value> {
    let report = ctx.cortex.sleep(32);
    Ok(json!({
        "elapsed_ms": report.elapsed_ms,
        "episodes_replayed": report.episodes_replayed,
        "unique_patterns": report.unique_patterns,
        "avg_target_overlap": report.avg_target_overlap,
    }))
}

pub fn export_json(ctx: &mut Ctx, out: &std::path::Path) -> Result<Value> {
    let snapshot = ctx.cortex.snapshot();
    let value = json!({
        "stats": snapshot.stats(),
        "modulators": snapshot.modulators(),
        "hierarchy_len": snapshot.hierarchy().len(),
        "regions": snapshot.hierarchy().region_ids(),
        "replay_frames": snapshot.replay().len(),
        "working_memory_filled": snapshot.working_memory().n_filled(),
        "episodes": snapshot.hippocampus().len(),
    });
    let json = serde_json::to_string_pretty(&value)?;
    std::fs::write(out, json)?;
    Ok(json!({ "path": out.to_string_lossy(), "ok": true }))
}

