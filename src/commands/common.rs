//! Helpers shared across commands.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::ctx::Ctx;
use crate::format::{Fmt, OutputFormat};

/// Render a single value to stdout in the configured format.
pub fn print(ctx: &Ctx, value: Value) -> Result<()> {
    let fmt = Fmt::new(ctx_format(ctx));
    let mut out = std::io::stdout();
    fmt.render(&value, &mut out)
        .context("render output")?;
    Ok(())
}

/// Resolve the output format from CLI / config.
pub fn ctx_format(ctx: &Ctx) -> OutputFormat {
    OutputFormat::Table // override with config later
}

/// Parse a JSON string into a Value; empty strings return `Value::Null`.
pub fn parse_input(s: &str) -> Result<Value> {
    if s.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(s).context("parse JSON input")
}

/// Read all of stdin into a single Value (array of lines or parsed JSON).
pub fn read_stdin() -> Result<Value> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    // Try as JSON first; fall back to plain text (single string).
    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => Ok(v),
        Err(_) => Ok(Value::String(trimmed.to_string())),
    }
}

/// Confirm a destructive action unless `--yes` is set.
pub fn confirm(question: &str, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    use std::io::Write;
    print!("{question} [y/N] ");
    std::io::stdout().flush()?;
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    if s.trim().eq_ignore_ascii_case("y") {
        Ok(())
    } else {
        Err(anyhow::anyhow!("aborted"))
    }
}

#[allow(dead_code)]
pub fn write_str(p: &Path, s: &str) -> Result<()> {
    std::fs::write(p, s).context("write file")?;
    Ok(())
}
