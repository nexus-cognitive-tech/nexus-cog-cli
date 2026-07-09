//! Config subcommands: show, init, add-profile.

use anyhow::Result;
use serde_json::Value;

use crate::config::CliConfig;

pub fn show() -> Result<Value> {
    let cfg = CliConfig::load_default().unwrap_or_default();
    Ok(serde_json::to_value(&cfg)?)
}

pub fn init() -> Result<Value> {
    let path = CliConfig::default_path()?;
    let cfg = CliConfig::default();
    cfg.save(&path)?;
    Ok(serde_json::json!({ "path": path.display().to_string(), "ok": true }))
}

pub fn add_profile(name: &str, db: Option<&str>) -> Result<Value> {
    let path = CliConfig::default_path()?;
    let mut cfg = CliConfig::load_default().unwrap_or_default();
    cfg.profile.insert(
        name.to_string(),
        crate::config::Profile {
            db: db.map(|s| std::path::PathBuf::from(s)),
            format: None,
            embedder_url: None,
            embedder_model: None,
        },
    );
    cfg.save(&path)?;
    Ok(serde_json::json!({ "path": path.display().to_string(), "profile": name, "ok": true }))
}
