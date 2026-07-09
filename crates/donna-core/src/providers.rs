//! Model providers: a provider-agnostic streaming chat + model listing layer.
//!
//! Supports local models via Ollama and cloud models via OpenAI and Anthropic. Each
//! provider streams tokens through a caller-supplied callback so the command layer can
//! forward them over a Tauri `Channel` to the UI. Google is recognized but not yet
//! implemented (Phase 1 ships at least one cloud provider per the roadmap).

use std::sync::OnceLock;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

/// Keep the model loaded in Ollama between messages (avoids cold-start reloads).
const OLLAMA_KEEP_ALIVE: &str = "30m";
/// Smaller context windows prefill faster than model defaults (often 8k–32k).
const OLLAMA_NUM_CTX: u32 = 4096;
/// Cap generation length so short replies stay snappy.
const OLLAMA_NUM_PREDICT: i32 = 1024;

pub(crate) fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

fn ollama_chat_options() -> serde_json::Value {
    serde_json::json!({
        "num_ctx": OLLAMA_NUM_CTX,
        "num_predict": OLLAMA_NUM_PREDICT,
    })
}

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

/// Run a chat completion and return the full text (collects the stream). Used for
/// non-interactive tasks like knowledge extraction.
pub async fn complete(
    provider: &str,
    model: &str,
    api_key: Option<String>,
    ollama_host: &str,
    messages: &[ChatTurn],
) -> Result<String> {
    let mut out = String::new();
    stream_chat(provider, model, api_key, ollama_host, messages, |t| {
        out.push_str(t)
    })
    .await?;
    Ok(out)
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
    let resp = http_client().get(url).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(
            "Could not reach Ollama. Is it installed and running?".into(),
        ));
    }
    let tags: OllamaTags = resp.json().await?;
    Ok(tags.models.into_iter().map(|m| m.name).collect())
}

/// Load the model into Ollama memory so the first chat message is not stuck waiting
/// on a cold start. Safe to call repeatedly; runs in the background on app launch.
pub async fn warm_ollama_model(host: &str, model: &str) -> Result<()> {
    if model.is_empty() {
        return Ok(());
    }
    let url = format!("{}/api/generate", host.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "prompt": ".",
        "stream": false,
        "keep_alive": OLLAMA_KEEP_ALIVE,
        "options": {
            "num_ctx": 512,
            "num_predict": 1,
        },
    });
    let resp = http_client().post(url).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Could not warm up Ollama model ({}).",
            resp.status()
        )));
    }
    Ok(())
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
        "keep_alive": OLLAMA_KEEP_ALIVE,
        "options": ollama_chat_options(),
    });
    let resp = http_client().post(url).json(&body).send().await?;
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

/// Generate an embedding vector via Ollama's `/api/embeddings` endpoint.
pub async fn embed_ollama(host: &str, model: &str, text: &str) -> Result<Vec<f32>> {
    if model.is_empty() || text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{}/api/embeddings", host.trim_end_matches('/'));
    let body = serde_json::json!({ "model": model, "prompt": text });
    let resp = http_client().post(url).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Ollama embeddings error ({}). Is {} pulled?",
            resp.status(),
            model
        )));
    }
    let v: serde_json::Value = resp.json().await?;
    let embedding = v
        .get("embedding")
        .and_then(|e| e.as_array())
        .ok_or_else(|| Error::Provider("unexpected Ollama embeddings response".into()))?;
    Ok(embedding
        .iter()
        .filter_map(|n| n.as_f64().map(|f| f as f32))
        .collect())
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
    let resp = http_client()
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

// --- OpenAI agent step (tool-calling) ----------------------------------------
//
// Parallel entry point for the Phase 2 agent loop. Kept separate from
// `stream_chat`/`consume_sse`/`ChatTurn` because those serve plain chat and their
// `extract` contract is text-only — bending it to also carry tool-call deltas and
// usage would leak agent concerns into every existing provider path.

/// A message in the OpenAI agent loop. Unlike `ChatTurn`, this can carry assistant
/// tool-call requests or tool-result turns. Optional fields are omitted (not
/// serialized as `null`) so assistant turns without content, and non-tool turns,
/// match the exact OpenAI wire shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurn {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// A tool call as it appears on an outgoing assistant `AgentTurn`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallOut {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// A tool call assembled from streamed deltas, ready for the agent loop to execute.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// One step of the agent loop: any text OpenAI produced, any tool calls it wants
/// to make, and the token usage for this step.
#[derive(Debug, Clone, Default)]
pub struct AgentStep {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub total_tokens: u64,
}

/// In-progress tool call, keyed by its `index` in the `tool_calls` delta array.
/// `id`/`name` arrive on the first fragment for that index; `arguments` accumulates
/// as plain string concatenation across fragments (it's a JSON string being typed
/// out incrementally, so fragments can split mid-token or mid-escape).
#[derive(Debug, Clone, Default)]
pub struct PartialToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Fold one `choices[0].delta.tool_calls[]` element into the accumulator.
fn accumulate_tool_delta(acc: &mut Vec<PartialToolCall>, delta: &serde_json::Value) {
    let Some(index) = delta.get("index").and_then(|i| i.as_u64()) else {
        return;
    };
    let index = index as usize;
    if acc.len() <= index {
        acc.resize(index + 1, PartialToolCall::default());
    }
    let entry = &mut acc[index];
    if let Some(id) = delta.get("id").and_then(|v| v.as_str()) {
        entry.id.push_str(id);
    }
    if let Some(function) = delta.get("function") {
        if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
            entry.name.push_str(name);
        }
        if let Some(args) = function.get("arguments").and_then(|v| v.as_str()) {
            entry.arguments.push_str(args);
        }
    }
}

/// Turn the accumulator into the final `ToolCall`s, dropping any entry whose name
/// never arrived (e.g. a stray/incomplete index).
fn finish_tool_calls(acc: Vec<PartialToolCall>) -> Vec<ToolCall> {
    acc.into_iter()
        .filter(|p| !p.name.is_empty())
        .map(|p| ToolCall {
            id: p.id,
            name: p.name,
            arguments: p.arguments,
        })
        .collect()
}

/// Run one step of the OpenAI tool-calling agent loop: stream a chat completion,
/// forwarding text deltas to `on_token`, and return the accumulated content, any
/// tool calls the model wants to make, and token usage for this step.
pub async fn openai_agent_step(
    api_key: &str,
    model: &str,
    messages: &[AgentTurn],
    tools: &serde_json::Value,
    on_token: &mut (impl FnMut(&str) + Send),
) -> Result<AgentStep> {
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "tools": tools,
        "stream": true,
        "stream_options": {"include_usage": true},
    });
    let resp = http_client()
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
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

    let mut content = String::new();
    let mut tool_acc: Vec<PartialToolCall> = Vec::new();
    let mut total_tokens = 0u64;

    // Bespoke SSE loop (mirrors `consume_sse`'s chunk-buffering: bytes can arrive
    // split mid-line, so we accumulate into `buf` and only drain complete lines).
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find('\n') {
            let line: String = buf.drain(..=idx).collect();
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };

            if let Some(usage) = v.get("usage").filter(|u| !u.is_null()) {
                if let Some(t) = usage.get("total_tokens").and_then(|t| t.as_u64()) {
                    total_tokens = t;
                }
            }

            let Some(choice) = v.get("choices").and_then(|c| c.get(0)) else {
                continue;
            };
            let Some(delta) = choice.get("delta") else {
                continue;
            };
            if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
                if !text.is_empty() {
                    on_token(text);
                    content.push_str(text);
                }
            }
            if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                for call in calls {
                    accumulate_tool_delta(&mut tool_acc, call);
                }
            }
        }
    }

    Ok(AgentStep {
        content,
        tool_calls: finish_tool_calls(tool_acc),
        total_tokens,
    })
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
    let resp = http_client()
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

    let resp = http_client()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_delta_accumulation_assembles_fragmented_calls() {
        let mut acc = Vec::new();
        for chunk in [
            r##"{"index":0,"id":"call_a","type":"function","function":{"name":"slack_send_message","arguments":""}}"##,
            r##"{"index":0,"function":{"arguments":"{\"channel\":"}}"##,
            r##"{"index":0,"function":{"arguments":"\"#general\",\"text\":\"hi\"}"}}"##,
            r##"{"index":1,"id":"call_b","type":"function","function":{"name":"list_docs","arguments":"{}"}}"##,
        ] {
            accumulate_tool_delta(&mut acc, &serde_json::from_str(chunk).unwrap());
        }
        let calls = finish_tool_calls(acc);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "slack_send_message");
        assert_eq!(calls[0].arguments, r##"{"channel":"#general","text":"hi"}"##);
        assert_eq!(calls[1].name, "list_docs");
    }

    #[test]
    fn agent_turn_serializes_openai_shapes() {
        let assistant = AgentTurn {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(vec![ToolCallOut {
                id: "call_a".into(),
                kind: "function".into(),
                function: FunctionCall {
                    name: "list_docs".into(),
                    arguments: "{}".into(),
                },
            }]),
            tool_call_id: None,
        };
        let v = serde_json::to_value(&assistant).unwrap();
        assert!(v.get("content").is_none());
        assert_eq!(v["tool_calls"][0]["function"]["name"], "list_docs");
        let tool = AgentTurn {
            role: "tool".into(),
            content: Some("[]".into()),
            tool_calls: None,
            tool_call_id: Some("call_a".into()),
        };
        let v = serde_json::to_value(&tool).unwrap();
        assert_eq!(v["tool_call_id"], "call_a");
        assert!(v.get("tool_calls").is_none());
    }
}
