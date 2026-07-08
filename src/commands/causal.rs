//! Causal engine: forward, backward, counterfactual, pre-mortem, blast radius, graph ops.

use anyhow::Result;
use nexus_cog_core::causal::{CausalEdge, CausalEdgeType, CausalNode, CausalNodeType};
use nexus_cog_core::Confidence;
use serde_json::{json, Value};

use crate::ctx::Ctx;

pub fn add_node(
    ctx: &mut Ctx,
    id: &str,
    name: &str,
    kind: Option<&str>,
    description: Option<&str>,
) -> Result<Value> {
    let node_type = match kind {
        Some(s) => parse_node_type(s)?,
        None => CausalNodeType::CodeEntity,
    };
    let existed = ctx.engines.causal.node(id).is_some();
    ctx.engines.causal.add_node(CausalNode {
        id: id.to_string(),
        node_type,
        name: name.to_string(),
        description: description.unwrap_or("").to_string(),
        file: None,
        line: None,
        confidence: Confidence::new(1.0),
        tags: vec![],
    });
    Ok(format_dump(&ctx.engines.causal, &format!("{} node `{id}`", if existed { "updated" } else { "added" })))
}

pub fn add_edge(ctx: &mut Ctx, from: &str, to: &str, kind: Option<&str>, strength: Option<f64>) -> Result<Value> {
    let edge_type = match kind {
        Some(s) => parse_edge_type(s)?,
        None => CausalEdgeType::Causes,
    };
    let s = strength.unwrap_or(0.5) as f32;
    let added = ctx.engines.causal.add_edge(CausalEdge {
        from: from.to_string(),
        to: to.to_string(),
        edge_type,
        strength: s,
        confidence: Confidence::new(1.0),
        evidence: vec![],
    })?;
    if !added {
        anyhow::bail!("causal edge {from}->{to} rejected: missing endpoint or self-loop rejected");
    }
    Ok(format_dump(&ctx.engines.causal, &format!("added edge `{from}` -> `{to}`")))
}

pub fn forward(ctx: &Ctx, entity: &str) -> Result<Value> {
    let engine = ctx.engines.causal.clone();
    let mut reasoner = nexus_cog_causal::ForwardReasoner::new(engine);
    let impact = reasoner.impact_of(entity);
    Ok(json!({ "entity": entity, "impact": impact }))
}

pub fn backward(ctx: &Ctx, entity: &str) -> Result<Value> {
    let engine = ctx.engines.causal.clone();
    let mut reasoner = nexus_cog_causal::BackwardReasoner::new(engine);
    let impact = reasoner.causes_of(entity);
    Ok(json!({ "entity": entity, "impact": impact }))
}

pub fn counterfactual(ctx: &Ctx, entity: &str) -> Result<Value> {
    let engine = ctx.engines.causal.clone();
    let mut engine2 = nexus_cog_causal::CounterfactualReasoner::new(engine);
    let alts = engine2.propose_counterfactuals(entity);
    Ok(serde_json::to_value(alts)?)
}

pub fn pre_mortem(ctx: &Ctx, entity: &str) -> Result<Value> {
    let engine = ctx.engines.causal.clone();
    let mut engine2 = nexus_cog_causal::PreMortemEngine::new(engine);
    let report = engine2.run(entity);
    Ok(serde_json::to_value(report)?)
}

pub fn blast(ctx: &Ctx, entity: &str) -> Result<Value> {
    let engine = ctx.engines.causal.clone();
    let mut engine2 = nexus_cog_causal::BlastRadiusCalculator::new(engine);
    let r = engine2.compute(entity);
    Ok(serde_json::to_value(r)?)
}

pub fn dump(ctx: &Ctx) -> Result<Value> {
    Ok(format_dump(&ctx.engines.causal, "dump"))
}

fn format_dump(engine: &nexus_cog_causal::CausalGraphEngine, op: &str) -> Value {
    let nodes = engine.nodes();
    let edges = engine.edges();
    let mut by_type: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for n in &nodes {
        *by_type.entry(format!("{:?}", n.node_type)).or_insert(0) += 1;
    }
    json!({
        "op": op,
        "node_count": nodes.len(),
        "edge_count": edges.len(),
        "nodes": nodes,
        "edges": edges,
        "by_type": by_type,
    })
}

fn parse_node_type(s: &str) -> Result<CausalNodeType> {
    use CausalNodeType::*;
    Ok(match s.to_lowercase().as_str() {
        "code" | "code_entity" | "code-entity" => CodeEntity,
        "behavior" => Behavior,
        "feature" => Feature,
        "invariant" => Invariant,
        "assumption" => Assumption,
        "decision" => Decision,
        "constraint" => Constraint,
        "bug" => Bug,
        "external" | "external_dep" | "external-dep" => ExternalDep,
        other => anyhow::bail!("unknown causal node type: {other}"),
    })
}

fn parse_edge_type(s: &str) -> Result<CausalEdgeType> {
    use CausalEdgeType::*;
    Ok(match s.to_lowercase().as_str() {
        "causes" => Causes,
        "enables" => Enables,
        "prevents" => Prevents,
        "mitigates" => Mitigates,
        "correlates" => Correlates,
        other => anyhow::bail!("unknown causal edge type: {other}"),
    })
}
