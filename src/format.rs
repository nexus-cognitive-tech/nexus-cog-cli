//! Output formatting: pretty tables, JSON, YAML, plain.
//!
//! All commands return a [`Value`] (JSON-compatible) and the formatter
//! renders it. Colours are auto-disabled when stdout is not a TTY.

use std::io::Write;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::ValueEnum;
use comfy_table::{Cell, Table};
use owo_colors::{OwoColorize, Stream::Stdout, Style};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
    Plain,
}

impl FromStr for OutputFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.to_lowercase().as_str() {
            "table" => Self::Table,
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "plain" | "text" => Self::Plain,
            other => return Err(anyhow!("unknown format: {other}")),
        })
    }
}

pub struct Fmt {
    pub format: OutputFormat,
}

impl Fmt {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Render a JSON-compatible value to `out`.
    pub fn render(&self, value: &Value, out: &mut dyn Write) -> Result<()> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_writer_pretty(&mut *out, value)?;
                writeln!(out)?;
            }
            OutputFormat::Yaml => {
                serde_yaml::to_writer(&mut *out, value)?;
                writeln!(out)?;
            }
            OutputFormat::Plain => {
                write_plain(value, out)?;
                writeln!(out)?;
            }
            OutputFormat::Table => {
                let mut t = Table::new();
                t.load_preset(comfy_table::presets::UTF8_FULL);
                if let Value::Object(map) = value {
                    if map.len() == 1 && let Some(Value::Array(items)) = map.values().next() {
                        render_table_from_array(&mut t, items, out);
                    } else {
                        render_object_as_rows(&mut t, map, out);
                    }
                } else if let Value::Array(items) = value {
                    render_table_from_array(&mut t, items, out);
                } else {
                    writeln!(out, "{}", value)?;
                }
            }
        }
        Ok(())
    }
}

fn render_table_from_array(t: &mut Table, items: &[Value], out: &mut dyn Write) {
    if items.is_empty() {
        writeln!(out, "(empty)").ok();
        return;
    }
    let headers = collect_keys(items);
    t.set_header(headers.iter().map(|h| h.as_str()).collect::<Vec<_>>());
    for item in items {
        if let Value::Object(map) = item {
            let row: Vec<Cell> = headers
                .iter()
                .map(|k| {
                    let v = map.get(k).cloned().unwrap_or(Value::Null);
                    cell(&v)
                })
                .collect();
            t.add_row(row);
        }
    }
    writeln!(out, "{t}").ok();
}

fn render_object_as_rows(t: &mut Table, map: &serde_json::Map<String, Value>, out: &mut dyn Write) {
    t.set_header(vec!["key", "value"]);
    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();
    for k in keys {
        t.add_row(vec![k.clone().into(), cell(&map[k])]);
    }
    writeln!(out, "{t}").ok();
}

fn cell(v: &Value) -> Cell {
    let s = match v {
        Value::Null => "—".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    };
    if is_terminal::is_terminal(std::io::stdout()) {
        let s = if s.len() > 80 {
            format!("{}…", &s[..79])
        } else {
            s
        };
        Cell::from(s.if_supports_color(Stdout, |t| t.dimmed()))
    } else {
        Cell::from(if s.len() > 80 { format!("{}…", &s[..79]) } else { s })
    }
}

fn write_plain(v: &Value, out: &mut dyn Write) -> Result<()> {
    fn rec(v: &Value, out: &mut dyn Write, indent: usize) -> Result<()> {
        match v {
            Value::Object(map) => {
                for (k, vv) in map {
                    match vv {
                        Value::Object(_) | Value::Array(_) => {
                            writeln!(out, "{:indent$}{}:", "", k, indent = indent)?;
                            rec(vv, out, indent + 2)?;
                        }
                        _ => writeln!(out, "{:indent$}{}: {}", "", k, vv, indent = indent)?,
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    rec(item, out, indent + 2)?;
                    writeln!(out)?;
                }
            }
            _ => writeln!(out, "{:indent$}{}", "", v, indent = indent)?,
        }
        Ok(())
    }
    rec(v, out, 0)?;
    Ok(())
}

fn collect_keys(items: &[Value]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for item in items {
        if let Value::Object(map) = item {
            for k in map.keys() {
                if !keys.contains(k) {
                    keys.push(k.clone());
                }
            }
        }
    }
    keys.sort();
    keys
}

// silence warning on unused style import in minimal builds
#[allow(dead_code)]
fn _style() -> Style {
    Style::new()
}
