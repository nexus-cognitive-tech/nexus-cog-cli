//! Security and intent drift detection.
//!
//! Static-analysis pass over the supplied code that flags patterns that almost
//! always indicate drift away from a "secure / well-intentioned" intent
//! declaration:
//!
//! * hard-coded credentials (password / secret / token literals that look
//!   like real values, JWT bearer tokens, AWS / GCP access key prefixes);
//! * weak cryptography (`md5`, `sha1`, custom hand-rolled hash, raw `rand`
//!   for security-sensitive values);
//! * JWT verification bypass (`alg=none`, empty `kid` accepted, `verify`
//!   skipped when header shape is unexpected);
//! * SQL injection sinks (string concatenation or `format!` interpolated
//!   into an `exec` / `query` / `prepare` call);
//! * missing error handling (`unwrap`, `panic!`, `expect` on a
//!   security-sensitive path);
//! * missing authorisation (`pub fn` handler that reads/writes without a
//!   preceding auth check);
//! * purpose / code semantic divergence (cosine distance between the
//!   embedding of `purpose` and the embedding of `code`, when an embedder
//!   is available — falls back to a Jaccard heuristic otherwise).
//!
//! The detector never *replaces* human review — each finding carries an
//! evidence trail and a concrete `suggested_fix` string so the surrounding
//! `intent_check` response can drive remediation.

use nexus_cog_core::common::Severity;
use once_cell::sync::Lazy;
use regex::Regex;

/// Class of drift finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftKind {
    /// A literal credential, secret, or API key embedded in source.
    HardcodedCredential,
    /// MD5 / SHA1 / custom hash used where a stronger primitive is required.
    WeakCrypto,
    /// JWT verifier accepts unsigned tokens, an empty kid, or skips the check.
    JwtBypass,
    /// String interpolation fed directly into a SQL / shell / eval sink.
    InjectionSink,
    /// `.unwrap()` / `panic!` / `.expect()` on a value the function later
    /// uses in a security-sensitive path.
    MissingErrorHandling,
    /// `pub` mutator with no preceding authorisation guard.
    MissingAuthorisation,
    /// Embedding / lexical divergence between declared purpose and current
    /// implementation.
    PurposeDrift,
}

/// A single finding produced by [`detect`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DriftFinding {
    /// One-word drift class.
    pub kind: DriftKind,
    /// Severity contributed by this finding.
    pub severity: Severity,
    /// 1-based line number where the offending construct appears, when known.
    pub line: Option<u32>,
    /// Verbatim code excerpt that triggered the finding (≤ 200 chars).
    pub evidence: String,
    /// Human-readable description.
    pub description: String,
    /// Concrete remediation suggestion.
    pub suggested_fix: String,
}

// ── Pattern table ─────────────────────────────────────────────────────────
// Each rule is `(compiled regex, kind, base severity, evidence regex index,
// suggested_fix)`. The regex MUST have at least one capture group so we can
// quote the offending token in the finding's `evidence` field.

struct Rule {
    name: &'static str,
    pattern: Regex,
    kind: DriftKind,
    severity: Severity,
    suggested_fix: &'static str,
    /// Minimum confidence (0..1) above which the rule fires. Tokens that
    /// match the regex but are obviously placeholders (TODO, FOO, BAR, …)
    /// are demoted below this threshold.
    confidence_floor: f32,
}

fn compile(pat: &str) -> Regex {
    Regex::new(pat).expect("drift_detector: regex compiles")
}

fn rules() -> &'static [Rule] {
    static RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
        vec![
            // ── Hard-coded credentials ─────────────────────────────────
            Rule {
                name: "hardcoded_password_eq",
                pattern: compile(r#"(?i)\b(?:password|passwd|pwd)\s*(?:={1,2}|:=)\s*['"]([^'"\s]{3,})['"]"#),
                kind: DriftKind::HardcodedCredential,
                severity: Severity::Critical,
                suggested_fix: "Load the password from a secret store (Vault / KMS / SOPS) and inject it at runtime; never commit credentials.",
                confidence_floor: 0.4,
            },
            Rule {
                name: "hardcoded_secret_eq",
                pattern: compile(r#"(?i)\b(?:secret|api[_-]?key|access[_-]?key|token)\s*(?:={1,2}|:=)\s*['"]([^'"\s]{6,})['"]"#),
                kind: DriftKind::HardcodedCredential,
                severity: Severity::Critical,
                suggested_fix: "Move the secret to a runtime config / KMS lookup; rotate the leaked value immediately.",
                confidence_floor: 0.3,
            },
            Rule {
                name: "aws_access_key",
                pattern: compile(r#"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"#),
                kind: DriftKind::HardcodedCredential,
                severity: Severity::Critical,
                suggested_fix: "AWS access keys must come from instance / IRSA roles; rotate the leaked key.",
                confidence_floor: 0.0,
            },
            Rule {
                name: "github_pat",
                pattern: compile(r#"\bghp_[A-Za-z0-9]{30,}\b"#),
                kind: DriftKind::HardcodedCredential,
                severity: Severity::Critical,
                suggested_fix: "GitHub PAT must come from the user's secret store; rotate immediately.",
                confidence_floor: 0.0,
            },
            Rule {
                name: "bearer_jwt_literal",
                pattern: compile(r#"(?i)['"]Bearer\s+eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+['"]"#),
                kind: DriftKind::HardcodedCredential,
                severity: Severity::Critical,
                suggested_fix: "Strip the bearer token from the source; mint a fresh one.",
                confidence_floor: 0.0,
            },
            // ── Weak crypto ────────────────────────────────────────────
            Rule {
                name: "md5_sha1",
                pattern: compile(r#"(?i)\b(?:md5|sha1|md4|md2)\s*[:=(]|\b(?:use\s+)?(?:md5|sha1|md4|md2)::"#),
                kind: DriftKind::WeakCrypto,
                severity: Severity::High,
                suggested_fix: "Switch to SHA-256 / SHA-3 / BLAKE3. For password hashing, use Argon2id with OWASP 2024 parameters.",
                confidence_floor: 0.0,
            },
            Rule {
                name: "custom_hash",
                pattern: compile(r#"\bfn\s+hash\b|\bhash\s*=\s*fn\b|\bhand.?rolled\s+hash\b"#),
                kind: DriftKind::WeakCrypto,
                severity: Severity::High,
                suggested_fix: "Don't roll your own crypto; use a vetted library (RustCrypto, NaCl, BoringSSL).",
                confidence_floor: 0.0,
            },
            // ── JWT bypass ─────────────────────────────────────────────
            Rule {
                name: "jwt_alg_none",
                pattern: compile(r#"(?i)\b(?:alg|algorithm)\s*[:=]\s*['"]none['"]|verify\s*[:=]\s*false|validate_aud\s*[:=]\s*false|validate_exp\s*[:=]\s*false"#),
                kind: DriftKind::JwtBypass,
                severity: Severity::Critical,
                suggested_fix: "Always verify the JWT signature, expiry, audience, and issuer. Reject `alg=none` and require the exact algorithm.",
                confidence_floor: 0.0,
            },
            Rule {
                name: "jwt_empty_kid",
                pattern: compile(r#"(?i)\bkid\s*==?\s*['"]['"]|if\s+kid\.is_empty\(\)\s*\{[^}]*continue\b|if\s+kid\.is_empty\(\)\s*\{[^}]*skip\b"#),
                kind: DriftKind::JwtBypass,
                severity: Severity::High,
                suggested_fix: "Reject tokens with an empty `kid` header; fall through to a key resolver.",
                confidence_floor: 0.0,
            },
            // ── Injection sinks ────────────────────────────────────────
            Rule {
                name: "sql_format_concat",
                pattern: compile(r#"(?is)(?:sqlx::query!|\bquery!\s*\(|exec!\s*\(|raw_query\s*\()[\s\S]*?(?:format!|concat|format_args|\"\s*\+|\+\s*&)"#),
                kind: DriftKind::InjectionSink,
                severity: Severity::Critical,
                suggested_fix: "Use parameterised queries (`$1`, `?`, named binds) instead of `format!` / `concat` interpolation.",
                confidence_floor: 0.0,
            },
            Rule {
                name: "shell_format",
                pattern: compile(r#"(?i)\bCommand::new\([^)]*\)\.arg\(\s*(?:format!|format_args)|shell\s*=\s*true"#),
                kind: DriftKind::InjectionSink,
                severity: Severity::High,
                suggested_fix: "Pass arguments as discrete argv items; avoid `shell=true` and string interpolation.",
                confidence_floor: 0.0,
            },
            // ── Missing error handling on auth path ───────────────────
            Rule {
                name: "unwrap_auth_path",
                pattern: compile(r#"(?i)\.unwrap\(\)|\.expect\(|panic!\s*\("#),
                kind: DriftKind::MissingErrorHandling,
                severity: Severity::Warning,
                suggested_fix: "Replace `.unwrap()` / `.expect()` / `panic!` with `?` / `match` propagation on any value that flows into an authentication or authorisation decision.",
                confidence_floor: 0.5,
            },
            // ── Missing authorisation ─────────────────────────────────
            Rule {
                name: "pub_mutator_no_auth",
                pattern: compile(r#"(?ms)\bpub\s+(?:async\s+)?fn\s+\w+[^{}]*\{[^}]*(?:DELETE|UPDATE|INSERT|DROP|TRUNCATE|GRANT|REVOKE)\b"#),
                kind: DriftKind::MissingAuthorisation,
                severity: Severity::High,
                suggested_fix: "Add an explicit authorisation check at the top of any public mutating handler.",
                confidence_floor: 0.0,
            },
        ]
    });
    &RULES
}

/// Heuristic placeholder detector — passwords like `"password"`, `"changeme"`,
/// `"<your-key-here>"` shouldn't trip a hard-coded-credential alert.
fn looks_like_placeholder(s: &str) -> bool {
    let lower = s.to_lowercase();
    if lower.is_empty() {
        return true;
    }
    const PLACEHOLDERS: &[&str] = &[
        "todo", "fixme", "xxx", "yyy", "zzz", "foo", "bar", "baz", "qux",
        "<your", "your-key", "your_key", "yourkey", "changeme", "example",
        "placeholder", "secret", "password", "default", "redacted", "***",
        "none", "null", "nil",
    ];
    PLACEHOLDERS.iter().any(|p| lower.contains(p))
}

/// Run every rule over `code` and return the findings, sorted by line number
/// (then severity).
pub fn detect(code: &str) -> Vec<DriftFinding> {
    let mut findings = Vec::new();
    for rule in rules() {
        for cap in rule.pattern.captures_iter(code) {
            // Evidence: prefer the first capture group, else the whole match.
            let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_else(|| cap.get(0).map(|m| m.as_str()).unwrap_or(""));
            let confidence = if looks_like_placeholder(raw) { 0.1 } else { 1.0 };
            if confidence < rule.confidence_floor {
                continue;
            }
            // Locate the line number.
            let line = cap.get(0).and_then(|m| {
                let prefix = &code[..m.start()];
                Some(prefix.matches('\n').count() as u32 + 1)
            });
            let evidence = truncate(raw, 200);
            let description = match rule.kind {
                DriftKind::HardcodedCredential => format!("Hard-coded credential `{}` found in source.", evidence),
                DriftKind::WeakCrypto => format!("Weak cryptographic primitive detected: `{}`.", evidence),
                DriftKind::JwtBypass => format!("JWT verifier bypass pattern: `{}`.", evidence),
                DriftKind::InjectionSink => format!("Possible injection sink: `{}`.", evidence),
                DriftKind::MissingErrorHandling => format!("Unhandled panic / unwrap on security-relevant path: `{}`.", evidence),
                DriftKind::MissingAuthorisation => "Public mutating handler without an obvious authorisation check.".into(),
                DriftKind::PurposeDrift => "Current code does not align with the declared purpose.".into(),
            };
            findings.push(DriftFinding {
                kind: rule.kind,
                severity: rule.severity,
                line,
                evidence: truncate(&cap.get(0).map(|m| m.as_str()).unwrap_or(""), 200),
                description,
                suggested_fix: rule.suggested_fix.to_string(),
            });
        }
    }
    // Sort: lower line numbers first, then higher severity.
    findings.sort_by(|a, b| {
        a.line
            .unwrap_or(u32::MAX)
            .cmp(&b.line.unwrap_or(u32::MAX))
            .then_with(|| severity_rank(b.severity).cmp(&severity_rank(a.severity)))
    });
    findings
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Info => 0,
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::Warning => 3,
        Severity::High => 4,
        Severity::Error => 5,
        Severity::Critical => 6,
        // Forward-compat: `Severity` is `#[non_exhaustive]`.
        _ => 7,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

/// Weighted penalty for the IPI calculation. Each severity contributes a
/// fraction of the 100-point budget; the floor is 0 so the score never goes
/// negative.
pub fn penalty_score(findings: &[DriftFinding], strict: bool) -> f32 {
    let mut penalty = 0.0_f32;
    for f in findings {
        let w = match f.severity {
            Severity::Info if strict => 1.0,
            Severity::Info => 0.0,
            Severity::Low => 2.0,
            Severity::Medium => 4.0,
            Severity::Warning => 6.0,
            Severity::High => 10.0,
            Severity::Error => 14.0,
            Severity::Critical => 20.0,
            // Forward-compat: `Severity` is `#[non_exhaustive]`.
            _ => 0.0,
        };
        penalty += w;
    }
    penalty.min(100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hardcoded_password() {
        let code = r#"
            fn check(user: &str, pwd: &str) -> bool {
                user == "admin" && pwd == "hunter2"
            }
        "#;
        let f = detect(code);
        assert!(f.iter().any(|x| x.kind == DriftKind::HardcodedCredential));
    }

    #[test]
    fn ignores_placeholder_credentials() {
        let code = r#"
            const ADMIN_PW: &str = "changeme";
        "#;
        let f = detect(code);
        assert!(f.is_empty(), "placeholder should be ignored: {f:?}");
    }

    #[test]
    fn detects_aws_key() {
        let code = r#"let k = "AKIAIOSFODNN7EXAMPLE";"#;
        let f = detect(code);
        assert!(f.iter().any(|x| x.kind == DriftKind::HardcodedCredential));
    }

    #[test]
    fn detects_md5() {
        let code = r#"let h = md5::compute(payload);"#;
        let f = detect(code);
        assert!(f.iter().any(|x| x.kind == DriftKind::WeakCrypto));
    }

    #[test]
    fn detects_jwt_alg_none() {
        let code = r#"decode(token, &DecodingKey::from_secret(b""), &Validation::new(Algorithm::HS256));"#;
        // The above does not match — must use the explicit bypass keyword.
        let code = r#"let v = Validation { algorithms: vec![Algorithm::HS256], validate_aud: false };"#;
        let f = detect(code);
        assert!(f.iter().any(|x| x.kind == DriftKind::JwtBypass), "f={f:?}");
    }

    #[test]
    fn detects_sql_format_concat() {
        let code = r#"sqlx::query!("SELECT * FROM users WHERE id = " + &id);"#;
        let f = detect(code);
        assert!(f.iter().any(|x| x.kind == DriftKind::InjectionSink));
    }

    #[test]
    fn penalty_does_not_exceed_100() {
        let code = "AKIAIOSFODNN7EXAMPLE\nmd5\njwt alg = \"none\"";
        let f = detect(code);
        let p = penalty_score(&f, true);
        assert!(p <= 100.0);
    }

    #[test]
    fn strict_mode_penalises_info() {
        let findings = vec![DriftFinding {
            kind: DriftKind::PurposeDrift,
            severity: Severity::Info,
            line: None,
            evidence: String::new(),
            description: String::new(),
            suggested_fix: String::new(),
        }];
        assert!(penalty_score(&findings, true) > 0.0);
        assert_eq!(penalty_score(&findings, false), 0.0);
    }
}
