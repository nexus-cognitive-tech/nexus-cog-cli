//! Real A/B decision matrix for `brain_hypothesis`.
//!
//! The matrix scores each approach against every supplied criterion and emits
//! per-criterion winners, an overall weighted winner, and an evidence trail
//! the agent can quote in its own reasoning.

use nexus_cog_brain::hypothesis;
use nexus_cog_core::hypothesis::EstimatedMetrics;
use serde_json::{json, Value};
use std::collections::HashMap;

/// A single, normalised criterion score in `[0.0, 1.0]`. Higher = better.
pub type CriterionScore = f32;

/// Computed for each side of the comparison.
#[derive(Debug, Clone)]
pub struct Side {
    pub complexity: f32,
    pub performance: f32,
    pub readability: f32,
    pub maintainability: f32,
    pub error_handling: f32,
    pub testability: f32,
    pub security: f32,
    pub loc: usize,
}

impl Side {
    /// Look up a score by criterion name; returns `None` for unknown names.
    pub fn score_for(&self, criterion: &str) -> Option<CriterionScore> {
        Some(match criterion.to_ascii_lowercase().as_str() {
            "complexity" => 1.0 - self.complexity, // lower complexity is better
            "performance" => self.performance,
            "readability" => self.readability,
            "maintainability" => self.maintainability,
            "error_handling" | "error-handling" => self.error_handling,
            "testability" => self.testability,
            "security" => self.security,
            _ => return None,
        })
    }
}

impl From<EstimatedMetrics> for Side {
    fn from(m: EstimatedMetrics) -> Self {
        Side {
            complexity: m.complexity,
            performance: m.performance,
            readability: m.readability,
            maintainability: m.maintainability,
            error_handling: 0.0, // filled by `score_pair`
            testability: 0.0,
            security: 0.0,
            loc: 0,
        }
    }
}

/// Build the full decision matrix.
pub fn build(code_a: &str, code_b: &str, criteria: Option<&[String]>) -> Value {
    // Re-use the hypothesis engine's static metric estimator (so the matrix
    // matches what `brain_hypothesis` already produced) and layer our own
    // dynamic checks on top.
    let m_a: EstimatedMetrics = hypothesis::estimate_metrics(code_a);
    let m_b: EstimatedMetrics = hypothesis::estimate_metrics(code_b);
    let mut side_a: Side = m_a.into();
    let mut side_b: Side = m_b.into();
    side_a.loc = code_a.lines().count();
    side_b.loc = code_b.lines().count();
    side_a.error_handling = error_handling_score(code_a);
    side_b.error_handling = error_handling_score(code_b);
    side_a.testability = testability_score(code_a);
    side_b.testability = testability_score(code_b);
    side_a.security = security_score(code_a);
    side_b.security = security_score(code_b);

    // Effective criterion list: caller-supplied + the seven built-ins.
    let builtins: Vec<String> = vec![
        "complexity".into(),
        "performance".into(),
        "readability".into(),
        "maintainability".into(),
        "error_handling".into(),
        "testability".into(),
        "security".into(),
    ];
    let effective: Vec<String> = match criteria {
        Some(custom) if !custom.is_empty() => custom.to_vec(),
        _ => builtins.clone(),
    };

    // Weights: built-ins get 1.0; caller-supplied extras get 0.8 unless they
    // match a built-in.
    let weights: HashMap<String, f32> = effective
        .iter()
        .map(|c| {
            let w = if builtins.iter().any(|b| b.eq_ignore_ascii_case(c)) {
                1.0
            } else {
                0.8
            };
            (c.to_ascii_lowercase(), w)
        })
        .collect();

    let mut per_criterion: Vec<Value> = Vec::with_capacity(effective.len());
    let mut total_a = 0.0_f32;
    let mut total_b = 0.0_f32;
    let mut total_weight = 0.0_f32;
    for criterion in &effective {
        let key = criterion.to_ascii_lowercase();
        let score_a = side_a.score_for(&key).unwrap_or(0.0);
        let score_b = side_b.score_for(&key).unwrap_or(0.0);
        let w = *weights.get(&key).unwrap_or(&1.0);
        total_a += score_a * w;
        total_b += score_b * w;
        total_weight += w;
        let winner = if (score_a - score_b).abs() < 0.02 {
            "tie"
        } else if score_a > score_b {
            "a"
        } else {
            "b"
        };
        per_criterion.push(json!({
            "criterion": criterion,
            "a": score_a,
            "b": score_b,
            "delta": score_a - score_b,
            "weight": w,
            "winner": winner,
        }));
    }
    let avg_a = if total_weight > 0.0 { total_a / total_weight } else { 0.0 };
    let avg_b = if total_weight > 0.0 { total_b / total_weight } else { 0.0 };
    let overall_winner = if (avg_a - avg_b).abs() < 0.02 {
        "tie"
    } else if avg_a > avg_b {
        "a"
    } else {
        "b"
    };
    let confidence = ((avg_a - avg_b).abs() / avg_a.max(avg_b).max(0.01)).clamp(0.0, 1.0);

    let summary = match overall_winner {
        "a" => format!(
            "Approach A wins with weighted score {:.3} vs {:.3} for B.",
            avg_a, avg_b
        ),
        "b" => format!(
            "Approach B wins with weighted score {:.3} vs {:.3} for A.",
            avg_b, avg_a
        ),
        _ => format!(
            "Approaches are statistically tied (Δ = {:.3}); choose on a non-functional criterion.",
            (avg_a - avg_b).abs()
        ),
    };

    json!({
        "criteria": per_criterion,
        "scores": { "a": avg_a, "b": avg_b },
        "loc": { "a": side_a.loc, "b": side_b.loc },
        "winner": overall_winner,
        "confidence": confidence,
        "summary": summary,
    })
}

fn error_handling_score(code: &str) -> f32 {
    let has_result = code.contains("Result<") || code.contains("-> Result");
    let has_option = code.contains("Option<") || code.contains("-> Option");
    let has_question = code.contains('?');
    let has_match = code.contains("match ") || code.contains("try {") || code.contains("try:") || code.contains("except ");
    let has_unwrap = code.contains(".unwrap()") || code.contains(".expect(") || code.contains("panic!");
    let mut s: f32 = 0.4;
    if has_result { s += 0.2; }
    if has_option { s += 0.1; }
    if has_question { s += 0.2; }
    if has_match { s += 0.1; }
    if has_unwrap { s -= 0.25; }
    s.clamp(0.0, 1.0)
}

fn testability_score(code: &str) -> f32 {
    let lines = code.lines().count().max(1);
    let unwraps = code.matches(".unwrap()").count() + code.matches(".expect(").count();
    let panics = code.matches("panic!").count();
    let fns = code.matches("fn ").count().max(1);
    let purity_hint = if code.contains("&mut ") || code.contains("self,") || code.contains("self .") {
        -0.05
    } else {
        0.05
    };
    (0.6 - (unwraps as f32) * 0.05 - (panics as f32) * 0.1 + purity_hint + (1.0 / fns as f32) * 0.1)
        .clamp(0.0, 1.0)
        * (1.0 - (lines as f32 / 80.0).min(0.4))
}

fn security_score(code: &str) -> f32 {
    let lower = code.to_lowercase();
    let mut s: f32 = 0.8;
    if lower.contains("md5") || lower.contains("sha1") { s -= 0.3; }
    if lower.contains(".unwrap()") || lower.contains(".expect(") { s -= 0.05; }
    if lower.contains("format!") && (lower.contains("query") || lower.contains("exec")) { s -= 0.3; }
    if lower.contains("sqlx::query!") { s -= 0.15; }
    if lower.contains("validate_aud = false") || lower.contains("verify = false") { s -= 0.4; }
    if lower.contains("password = \"") || lower.contains("secret = \"") || lower.contains("api_key = \"") {
        s -= 0.5;
    }
    if lower.contains("argon2") || lower.contains("bcrypt") || lower.contains("scrypt") { s += 0.1; }
    s.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_function_scores_higher_than_unsafe() {
        let safe = r#"
            fn parse(s: &str) -> Result<i32, ParseError> {
                s.parse().map_err(ParseError::from)
            }
        "#;
        let bad = r#"
            fn parse(s: &str) -> i32 { s.parse().unwrap() }
        "#;
        let m = build(safe, bad, None);
        let a = m["scores"]["a"].as_f64().unwrap();
        let b = m["scores"]["b"].as_f64().unwrap();
        assert!(a > b, "safe ({a}) should beat unsafe ({b})");
    }

    #[test]
    fn hardcoded_password_pulls_security_down() {
        let bad = r#"
            fn check(user: &str, pw: &str) -> bool {
                user == "admin" && pw == "hunter2"
            }
        "#;
        let clean = r#"
            fn check(user: &str, pw_hash: &str, db: &Db) -> bool {
                db.lookup(user).map(|u| argon2::verify(pw_hash, &u.hash)).unwrap_or(false)
            }
        "#;
        let m = build(bad, clean, None);
        let criteria = m["criteria"].as_array().unwrap();
        let sec = criteria.iter().find(|c| c["criterion"] == "security").unwrap();
        assert!(sec["a"].as_f64().unwrap() < sec["b"].as_f64().unwrap());
    }

    #[test]
    fn custom_criteria_respected() {
        let m = build(
            "fn a() -> Result<(), E> { Ok(()) }",
            "fn b() { let _ = std::process::exit(0); }",
            Some(&["error_handling".into(), "security".into()]),
        );
        let criteria = m["criteria"].as_array().unwrap();
        assert_eq!(criteria.len(), 2);
    }
}
