//! Palace engine subcommands.

use anyhow::Result;
use nexus_cog_core::palace::{MemoryItem, RoomType};
use nexus_cog_palace::recall::{RecallEngine, RecallOptions};
use serde_json::{json, Value};

use crate::ctx::Ctx;

pub fn rooms(ctx: &Ctx) -> Result<Value> {
    let rooms: Vec<Value> = ctx
        .palace
        .rooms()
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "name": r.name,
                "type": r.room_type.id(),
                "importance": r.importance,
                "items": r.items.len(),
                "tags": r.tags,
            })
        })
        .collect();
    Ok(json!({ "rooms": rooms, "count": rooms.len() }))
}

pub fn summary(ctx: &Ctx) -> Result<Value> {
    let s = ctx.palace.summary();
    Ok(json!({
        "total_rooms": s.total_rooms,
        "total_items": s.total_items,
        "total_connections": s.total_connections,
    }))
}

pub fn add_room(ctx: &mut Ctx, name: &str, room_type: Option<&str>) -> Result<Value> {
    let rt = match room_type {
        Some(s) => parse_room_type(s)?,
        None => RoomType::Concept,
    };
    let id = ctx.palace.add_room(name, rt)?;
    ctx.save()?;
    Ok(json!({ "id": id, "name": name, "type": rt.id() }))
}

pub fn add_item(
    ctx: &Ctx,
    room_id: &str,
    key: &str,
    value: &str,
    confidence: Option<f64>,
    tags: Vec<String>,
) -> Result<Value> {
    let conf = confidence.unwrap_or(0.5) as f32;
    let item = MemoryItem {
        tags,
        ..MemoryItem::new(key, value, conf)
    };
    ctx.palace.add_item(room_id, item)?;
    ctx.save()?;
    Ok(json!({ "room": room_id, "key": key, "ok": true }))
}

/// Semantic recall across the palace.
///
/// This wraps [`RecallEngine::recall_bm25`] when the engine is exposed, falling
/// back to the legacy semantic-overlap path otherwise. Filters:
///   * `min_confidence` — drop items below the threshold;
///   * `required_tag`   — drop items that don't carry the tag;
///   * `room_type`      — restrict the candidate set to one room type.
pub fn recall(
    ctx: &Ctx,
    query: &str,
    limit: usize,
    min_confidence: Option<f64>,
    required_tag: Option<&str>,
    room_type: Option<&str>,
) -> Result<Value> {
    let mut opts = RecallOptions::default().with_limit(limit);
    if let Some(c) = min_confidence {
        opts = opts.with_min_confidence(c as f32);
    }
    if let Some(tag) = required_tag {
        opts.required_tag = Some(tag.to_string());
    }
    if let Some(rt) = room_type {
        opts.room_type = Some(parse_room_type(rt)?);
    }
    let rooms = ctx.palace.rooms();
    let results = RecallEngine::new().recall_bm25(query, &rooms, &opts);
    let json_results: Vec<Value> = results
        .into_iter()
        .map(|r| {
            json!({
                "item": r.item,
                "room_id": r.room_id,
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
    let s = strength.unwrap_or(0.5) as f32;
    ctx.palace.connect(from, to, relation, s)?;
    ctx.save()?;
    Ok(json!({ "from": from, "to": to, "relation": relation, "ok": true }))
}

pub fn decay(ctx: &Ctx) -> Result<Value> {
    use nexus_cog_palace::DecayConfig;
    let report = ctx.palace.apply_decay(&DecayConfig::default())?;
    Ok(crate::commands::decay::report_to_value(&report))
}

pub fn export_json(ctx: &mut Ctx, out: &std::path::Path) -> Result<Value> {
    ctx.palace.export_json(out)?;
    Ok(json!({ "path": out.to_string_lossy(), "ok": true }))
}

fn parse_room_type(s: &str) -> Result<RoomType> {
    Ok(match s.to_lowercase().as_str() {
        "concept" => RoomType::Concept,
        "pattern" => RoomType::Pattern,
        "decision" => RoomType::Decision,
        "bug" => RoomType::Bug,
        "learning" => RoomType::Learning,
        "tool" => RoomType::Tool,
        "user" => RoomType::User,
        "project" => RoomType::Project,
        other => anyhow::bail!("unknown room type: {other}"),
    })
}
