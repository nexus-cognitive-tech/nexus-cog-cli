//! Antifragile verification.
//!
//! Generates adversarial inputs (or edge cases for a supplied code snippet) and
//! lets callers paginate the result set so MCP clients don't have to ship the
//! full generator output over stdio in a single message.

use anyhow::Result;
use nexus_cog_core::antifragile::{AdversarialCategory, AdversarialInput};
use serde_json::{json, Value};

use crate::ctx::Ctx;

/// Maximum number of inputs one adversarial call will ever produce, regardless
/// of the caller-supplied `limit`. Hard ceiling protects the MCP transport.
const HARD_MAX_INPUTS: usize = 500;
const DEFAULT_MAX_INPUTS: usize = 50;

pub fn adversarial(
    ctx: &Ctx,
    target: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
    categories: Option<Vec<String>>,
    include_fuzz: Option<bool>,
) -> Result<Value> {
    let _ = target;
    let requested = limit.unwrap_or(DEFAULT_MAX_INPUTS).min(HARD_MAX_INPUTS);
    let offset = offset.unwrap_or(0);

    let mut inputs = ctx.engines.adversarial.generate();
    if let Some(filter) = categories.as_ref() {
        let parsed = parse_categories(filter)?;
        inputs.retain(|i| parsed.contains(&i.category));
    }
    if !include_fuzz.unwrap_or(false) {
        inputs.retain(|i| !matches!(i.category, AdversarialCategory::Fuzz));
    }

    let total = inputs.len();
    let end = (offset + requested).min(total);
    let start = offset.min(end);
    let page: Vec<&AdversarialInput> = inputs[start..end].iter().collect();

    let items: Vec<Value> = page
        .into_iter()
        .map(|i| {
            json!({
                "category": category_id(i.category),
                "description": i.description,
                "value": i.value,
                "rationale": i.rationale,
                "break_likelihood": i.break_likelihood,
            })
        })
        .collect();

    Ok(json!({
        "total": total,
        "offset": start,
        "limit": requested,
        "returned": items.len(),
        "has_more": end < total,
        "next_offset": if end < total { Some(end) } else { None },
        "inputs": items,
    }))
}

pub fn edge_cases(ctx: &Ctx, code: &str, target: &str) -> Result<Value> {
    let cases = ctx.engines.edge_cases.explore(target, code);
    Ok(serde_json::to_value(cases)?)
}

pub fn robustness(ctx: &Ctx, target: &str, results: Vec<(String, bool)>) -> Result<Value> {
    let n = results.len();
    let broken = results.iter().filter(|(_, b)| *b).count();
    let score = if n == 0 { 1.0 } else { 1.0 - broken as f64 / n as f64 };
    Ok(json!({
        "target": target,
        "total": n,
        "broken": broken,
        "score": score,
    }))
}

fn parse_categories(raw: &[String]) -> Result<Vec<AdversarialCategory>> {
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        let lower = s.to_lowercase();
        let cat = match lower.as_str() {
            "empty" => AdversarialCategory::Empty,
            "boundary" => AdversarialCategory::Boundary,
            "special_characters" | "special-chars" | "specialchars" => AdversarialCategory::SpecialCharacters,
            "large" => AdversarialCategory::Large,
            "malformed" => AdversarialCategory::Malformed,
            "repetition" => AdversarialCategory::Repetition,
            "injection" => AdversarialCategory::Injection,
            "numeric_edge" | "numeric-edge" | "numeric" => AdversarialCategory::NumericEdge,
            "type_confusion" | "type-confusion" => AdversarialCategory::TypeConfusion,
            "concurrency" => AdversarialCategory::Concurrency,
            "fuzz" => AdversarialCategory::Fuzz,
            other => anyhow::bail!("unknown adversarial category: `{other}`"),
        };
        if !out.contains(&cat) {
            out.push(cat);
        }
    }
    Ok(out)
}

fn category_id(c: AdversarialCategory) -> &'static str {
    match c {
        AdversarialCategory::Empty => "empty",
        AdversarialCategory::Boundary => "boundary",
        AdversarialCategory::SpecialCharacters => "special_characters",
        AdversarialCategory::Large => "large",
        AdversarialCategory::Malformed => "malformed",
        AdversarialCategory::Repetition => "repetition",
        AdversarialCategory::Injection => "injection",
        AdversarialCategory::NumericEdge => "numeric_edge",
        AdversarialCategory::TypeConfusion => "type_confusion",
        AdversarialCategory::Concurrency => "concurrency",
        AdversarialCategory::Fuzz => "fuzz",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adversarial_pagination() {
        // We can't easily construct a Ctx here, so exercise the helpers directly.
        let cats = parse_categories(&[
            "empty".into(),
            "INJECTION".into(),
            "unknown".into(),
        ]);
        assert!(cats.is_err());
        let cats = parse_categories(&["empty".into(), "injection".into()]).unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(category_id(AdversarialCategory::Fuzz), "fuzz");
    }
}
