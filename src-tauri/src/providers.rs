//! Model providers: a provider-agnostic streaming chat + model listing layer.
//!
//! Supports local models via Ollama and cloud models via OpenAI and Anthropic. Each
//! provider streams tokens through a caller-supplied callback so the command layer can
//! forward them over a Tauri `Channel` to the UI. Google is recognized but not yet
//! implemented (Phase 1 ships at least one cloud provider per the roadmap).

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

/// Stream a chat completion, invoking `on_token` for each text chunk received.
pub async fn stream_chat(
    provider: &str,
    model: &str,
    api_key: Option<String>,
    ollama_host: &str,
    messages: &[ChatTurn],
    mut on_token: impl FnMut(&str),
) -> Result<()> {
    match provider {
        "ollama" => stream_ollama(ollama_host, model, messages, &mut on_token).await,
        "openai" => {
            let key = api_key.ok_or_else(|| Error::MissingApiKey("openai".into()))?;
            stream_openai(&key, model, messages, &mut on_token).await
        }
        "anthropic" => {
            let key = api_key.ok_or_else(|| Error::MissingApiKey("anthropic".into()))?;
            stream_anthropic(&key, model, messages, &mut on_token).await
        }
        other => Err(Error::UnsupportedProvider(other.into())),
    }
}

/// List the models available for a provider.
pub async fn list_models(
    provider: &str,
    api_key: Option<String>,
    ollama_host: &str,
) -> Result<Vec<String>> {
    match provider {
        "ollama" => list_ollama_models(ollama_host).await,
        "openai" => match api_key {
            Some(key) => list_openai_models(&key).await,
            None => Ok(default_openai_models()),
        },
        "anthropic" => Ok(default_anthropic_models()),
        "google" => Ok(default_google_models()),
        other => Err(Error::UnsupportedProvider(other.into())),
    }
}

// --- Ollama -----------------------------------------------------------------

#[derive(Deserialize)]
struct OllamaTags {
    models: Vec<OllamaTag>,
}
#[derive(Deserialize)]
struct OllamaTag {
    name: String,
}

async fn list_ollama_models(host: &str) -> Result<Vec<String>> {
    let url = format!("{}/api/tags", host.trim_end_matches('/'));
    let resp = reqwest::get(url).await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(
            "Could not reach Ollama. Is it installed and running?".into(),
        ));
    }
    let tags: OllamaTags = resp.json().await?;
    Ok(tags.models.into_iter().map(|m| m.name).collect())
}

async fn stream_ollama(
    host: &str,
    model: &str,
    messages: &[ChatTurn],
    on_token: &mut impl FnMut(&str),
) -> Result<()> {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    let resp = reqwest::Client::new().post(url).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Ollama returned an error ({}).",
            resp.status()
        )));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find('\n') {
            let line: String = buf.drain(..=idx).collect();
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(text) = v
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                {
                    if !text.is_empty() {
                        on_token(text);
                    }
                }
            }
        }
    }
    Ok(())
}

// --- OpenAI -----------------------------------------------------------------

#[derive(Deserialize)]
struct OpenAiModels {
    data: Vec<OpenAiModel>,
}
#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

fn default_openai_models() -> Vec<String> {
    vec![
        "gpt-4o".into(),
        "gpt-4o-mini".into(),
        "gpt-4.1".into(),
        "gpt-4.1-mini".into(),
    ]
}

async fn list_openai_models(key: &str) -> Result<Vec<String>> {
    let resp = reqwest::Client::new()
        .get("https://api.openai.com/v1/models")
        .bearer_auth(key)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Ok(default_openai_models());
    }
    let models: OpenAiModels = resp.json().await?;
    let mut ids: Vec<String> = models
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| id.starts_with("gpt"))
        .collect();
    ids.sort();
    if ids.is_empty() {
        ids = default_openai_models();
    }
    Ok(ids)
}

async fn stream_openai(
    key: &str,
    model: &str,
    messages: &[ChatTurn],
    on_token: &mut impl FnMut(&str),
) -> Result<()> {
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    let resp = reqwest::Client::new()
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(key)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!(
            "OpenAI error ({status}): {detail}"
        )));
    }

    consume_sse(resp, on_token, |data| {
        let v: serde_json::Value = serde_json::from_str(data).ok()?;
        v.get("choices")?
            .get(0)?
            .get("delta")?
            .get("content")?
            .as_str()
            .map(|s| s.to_string())
    })
    .await
}

// --- Anthropic --------------------------------------------------------------

fn default_anthropic_models() -> Vec<String> {
    vec![
        "claude-3-5-sonnet-latest".into(),
        "claude-3-5-haiku-latest".into(),
        "claude-3-opus-latest".into(),
    ]
}

fn default_google_models() -> Vec<String> {
    vec![
        "gemini-1.5-pro".into(),
        "gemini-1.5-flash".into(),
        "gemini-2.0-flash".into(),
    ]
}

async fn stream_anthropic(
    key: &str,
    model: &str,
    messages: &[ChatTurn],
    on_token: &mut impl FnMut(&str),
) -> Result<()> {
    // Anthropic wants system prompts as a top-level field, not in `messages`.
    let system: String = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n\n");
    let turns: Vec<&ChatTurn> = messages.iter().filter(|m| m.role != "system").collect();

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "stream": true,
        "messages": turns,
    });
    if !system.is_empty() {
        body["system"] = serde_json::Value::String(system);
    }

    let resp = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!(
            "Anthropic error ({status}): {detail}"
        )));
    }

    consume_sse(resp, on_token, |data| {
        let v: serde_json::Value = serde_json::from_str(data).ok()?;
        if v.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
            return v
                .get("delta")?
                .get("text")?
                .as_str()
                .map(|s| s.to_string());
        }
        None
    })
    .await
}

// --- Shared SSE handling ----------------------------------------------------

/// Consume a Server-Sent Events stream, extracting text from each `data:` line via
/// `extract`, and forwarding non-empty results to `on_token`.
async fn consume_sse(
    resp: reqwest::Response,
    on_token: &mut impl FnMut(&str),
    extract: impl Fn(&str) -> Option<String>,
) -> Result<()> {
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find('\n') {
            let line: String = buf.drain(..=idx).collect();
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                if let Some(text) = extract(data) {
                    if !text.is_empty() {
                        on_token(&text);
                    }
                }
            }
        }
    }
    Ok(())
}
