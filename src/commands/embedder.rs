//! Embedder install / info (HTTP only — no actual model execution).

use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum EmbedderKind {
    Ollama,
    Openai,
    Noop,
}

pub fn info(kind: EmbedderKind) -> Result<Value> {
    Ok(match kind {
        EmbedderKind::Ollama => serde_json::json!({
            "kind": "ollama",
            "url": "http://localhost:11434",
            "models": ["nomic-embed-text", "mxbai-embed-large", "all-minilm"],
            "status": "not connected",
        }),
        EmbedderKind::Openai => serde_json::json!({
            "kind": "openai",
            "url": "https://api.openai.com/v1",
            "models": ["text-embedding-3-small", "text-embedding-3-large"],
            "status": "not connected",
        }),
        EmbedderKind::Noop => serde_json::json!({
            "kind": "noop",
            "status": "active (no embeddings stored)",
        }),
    })
}
