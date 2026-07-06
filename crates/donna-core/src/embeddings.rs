//! Ollama/OpenAI embedding index for knowledge-graph semantic retrieval.

use crate::db::Db;
use crate::error::{Error, Result};
use crate::knowledge::{self, KbNode};
use crate::providers;
use crate::secrets;

pub const DEFAULT_EMBED_MODEL: &str = "nomic-embed-text";
pub const DEFAULT_OPENAI_EMBED_MODEL: &str = "text-embedding-3-small";

pub fn node_key(node: &KbNode) -> String {
    format!("{}/{}", node.folder.join("/"), node.id)
}

fn node_text(node: &KbNode) -> String {
    format!(
        "{} {} {}",
        node.label,
        node.note,
        node.folder.join(" ")
    )
}

/// True when the currently configured provider can produce embeddings: Ollama
/// (always, assuming it's reachable) or OpenAI with an API key on file.
pub fn backend_available(db: &Db) -> bool {
    match db.get_setting("provider").ok().flatten().as_deref() {
        Some("openai") => secrets::has_api_key("openai").unwrap_or(false),
        _ => true,
    }
}

/// Embed `text` using the provider configured on `db`, routing to Ollama or OpenAI.
pub async fn embed(db: &Db, host: &str, model: &str, text: &str) -> Result<Vec<f32>> {
    let provider = db
        .get_setting("provider")?
        .unwrap_or_else(|| "ollama".into());
    match provider.as_str() {
        "ollama" => providers::embed_ollama(host, model, text).await,
        "openai" => {
            let key = secrets::get_api_key("openai")?
                .ok_or_else(|| Error::MissingApiKey("openai".into()))?;
            let model = db
                .get_setting("embed_model")?
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| DEFAULT_OPENAI_EMBED_MODEL.into());
            openai_embed(&key, &model, text).await
        }
        other => Err(Error::UnsupportedProvider(other.into())),
    }
}

pub async fn index_node(db: &Db, host: &str, model: &str, node: &KbNode) -> Result<()> {
    let vector = embed(db, host, model, &node_text(node)).await?;
    db.upsert_embedding(&node_key(node), &vector)
}

pub async fn reindex_all(db: &Db, host: &str, model: &str) -> Result<usize> {
    let graph = knowledge::graph()?;
    let mut count = 0usize;
    for node in &graph.nodes {
        if index_node(db, host, model, node).await.is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

fn openai_embed_body(input: &str, model: &str) -> serde_json::Value {
    serde_json::json!({ "model": model, "input": input })
}

/// Generate an embedding vector via OpenAI's `/v1/embeddings` endpoint.
async fn openai_embed(api_key: &str, model: &str, input: &str) -> Result<Vec<f32>> {
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    let body = openai_embed_body(input, model);
    let resp = reqwest::Client::new()
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!(
            "OpenAI embeddings error ({status}): {detail}"
        )));
    }
    let v: serde_json::Value = resp.json().await?;
    let embedding = v
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("embedding"))
        .and_then(|e| e.as_array())
        .ok_or_else(|| Error::Provider("unexpected OpenAI embeddings response".into()))?;
    Ok(embedding
        .iter()
        .filter_map(|n| n.as_f64().map(|f| f as f32))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_request_body_shape() {
        let body = openai_embed_body("hello", "text-embedding-3-small");
        assert_eq!(body["model"], "text-embedding-3-small");
        assert_eq!(body["input"], "hello");
    }
}
