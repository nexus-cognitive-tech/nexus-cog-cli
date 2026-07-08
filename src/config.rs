//! Configuration loading.
//!
//! Reads `~/.config/nexus-cog/config.toml` and merges CLI flags.
//! Supports multiple named profiles (palaces) so you can switch between
//! projects, agents or namespaces without re-typing flags.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    /// Default profile to use when `--profile` is not given.
    pub default_profile: Option<String>,
    /// Named palace profiles.
    #[serde(default)]
    pub profile: HashMap<String, Profile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    /// Path to the SQLite palace database.
    pub db: Option<PathBuf>,
    /// Palace namespace id inside the DB.
    pub palace: Option<String>,
    /// Default output format.
    pub format: Option<String>,
    /// Optional embedder URL (e.g. `http://localhost:11434` for ollama).
    pub embedder_url: Option<String>,
    /// Embedder model name (e.g. `nomic-embed-text`).
    pub embedder_model: Option<String>,
}

impl CliConfig {
    /// Load config from the standard XDG location.
    pub fn load_default() -> Result<Self> {
        let path = Self::default_path()?;
        if path.exists() {
            Self::load(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Standard config path: `$XDG_CONFIG_HOME/nexus-cog/config.toml`.
    pub fn default_path() -> Result<PathBuf> {
        let dir = directories::ProjectDirs::from("", "", "nexus-cog")
            .context("could not resolve config directory")?;
        Ok(dir.config_dir().join("config.toml"))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse config {}", path.display()))
    }

    /// Resolve a profile by name, falling back to the default profile.
    pub fn resolve<'a>(&'a self, name: Option<&str>) -> Option<&'a Profile> {
        if let Some(n) = name {
            return self.profile.get(n);
        }
        if let Some(d) = &self.default_profile {
            return self.profile.get(d);
        }
        None
    }

    /// Save config to `path`, creating parent dirs.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}
