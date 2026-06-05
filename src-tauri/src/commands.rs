//! Tauri commands exposed to the frontend over IPC.
//!
//! Covers Phase 1: app config, secure key management, model listing, chat history,
//! and streaming chat completions.

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

use crate::db::{Conversation, Db, KgEdge, KgNode, Message};
use crate::error::{Error, Result};
use crate::integrations::{self, google, slack};
use crate::providers::{self, ChatTurn};
use crate::secrets;

const DONNA_SYSTEM_PROMPT: &str = "You are Donna, a warm, sharp, and proactive personal \
assistant who is private and runs locally on the user's own device. You learn about the \
user over time, help them think, draft, and stay organized, and you are concise and \
practical. When the user tells you to remember something about themselves or their \
routines, acknowledge it clearly.";

// --- Config -----------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub provider: String,
    pub model: String,
    pub ollama_host: String,
    pub onboarded: bool,
}

fn load_config(db: &Db) -> Result<AppConfig> {
    Ok(AppConfig {
        provider: db.get_setting("provider")?.unwrap_or_else(|| "ollama".into()),
        model: db.get_setting("model")?.unwrap_or_default(),
        ollama_host: db
            .get_setting("ollama_host")?
            .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into()),
        onboarded: db.get_setting("onboarded")?.as_deref() == Some("true"),
    })
}

#[tauri::command]
pub fn get_config(db: State<Db>) -> Result<AppConfig> {
    load_config(&db)
}

fn spawn_ollama_warmup(host: String, model: String) {
    if model.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _ = providers::warm_ollama_model(&host, &model).await;
    });
}

#[tauri::command]
pub fn save_config(db: State<Db>, config: AppConfig) -> Result<()> {
    db.set_setting("provider", &config.provider)?;
    db.set_setting("model", &config.model)?;
    db.set_setting("ollama_host", &config.ollama_host)?;
    db.set_setting("onboarded", if config.onboarded { "true" } else { "false" })?;
    if config.provider == "ollama" {
        spawn_ollama_warmup(config.ollama_host, config.model);
    }
    Ok(())
}

// --- Secrets ----------------------------------------------------------------

#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<()> {
    secrets::set_api_key(&provider, &key)
}

#[tauri::command]
pub fn has_api_key(provider: String) -> Result<bool> {
    secrets::has_api_key(&provider)
}

#[tauri::command]
pub fn delete_api_key(provider: String) -> Result<()> {
    secrets::delete_api_key(&provider)
}

// --- Models -----------------------------------------------------------------

#[tauri::command]
pub async fn list_models(db: State<'_, Db>, provider: String) -> Result<Vec<String>> {
    let ollama_host = db
        .get_setting("ollama_host")?
        .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider)?;
    providers::list_models(&provider, api_key, &ollama_host).await
}

// --- Conversations & messages ----------------------------------------------

#[tauri::command]
pub fn create_conversation(db: State<Db>, title: String) -> Result<i64> {
    db.create_conversation(&title)
}

#[tauri::command]
pub fn list_conversations(db: State<Db>) -> Result<Vec<Conversation>> {
    db.list_conversations()
}

#[tauri::command]
pub fn rename_conversation(db: State<Db>, id: i64, title: String) -> Result<()> {
    db.rename_conversation(id, &title)
}

#[tauri::command]
pub fn delete_conversation(db: State<Db>, id: i64) -> Result<()> {
    db.delete_conversation(id)
}

#[tauri::command]
pub fn get_messages(db: State<Db>, conversation_id: i64) -> Result<Vec<Message>> {
    db.get_messages(conversation_id)
}

#[tauri::command]
pub fn add_message(
    db: State<Db>,
    conversation_id: i64,
    role: String,
    content: String,
) -> Result<i64> {
    db.add_message(conversation_id, &role, &content)
}

// --- Streaming chat ---------------------------------------------------------

/// Events streamed to the frontend over a Tauri channel during a chat completion.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatEvent {
    Token { content: String },
    Done { message_id: i64 },
    Error { message: String },
}

/// Generate an assistant reply for a conversation, streaming tokens to the frontend
/// and persisting the final assistant message.
#[tauri::command]
pub async fn send_chat(
    db: State<'_, Db>,
    conversation_id: i64,
    on_event: Channel<ChatEvent>,
) -> Result<()> {
    let config = load_config(&db)?;
    if config.model.is_empty() {
        let _ = on_event.send(ChatEvent::Error {
            message: "No model selected. Pick one in Settings.".into(),
        });
        return Ok(());
    }

    let api_key = secrets::get_api_key(&config.provider)?;

    // Build the prompt: system persona + full conversation history.
    let mut turns: Vec<ChatTurn> = vec![ChatTurn {
        role: "system".into(),
        content: DONNA_SYSTEM_PROMPT.into(),
    }];
    for m in db.get_messages(conversation_id)? {
        turns.push(ChatTurn {
            role: m.role,
            content: m.content,
        });
    }

    let mut answer = String::new();
    let result = providers::stream_chat(
        &config.provider,
        &config.model,
        api_key,
        &config.ollama_host,
        &turns,
        |token| {
            answer.push_str(token);
            let _ = on_event.send(ChatEvent::Token {
                content: token.to_string(),
            });
        },
    )
    .await;

    match result {
        Ok(()) => {
            let id = db.add_message(conversation_id, "assistant", &answer)?;
            let _ = on_event.send(ChatEvent::Done { message_id: id });
            Ok(())
        }
        Err(e) => {
            let _ = on_event.send(ChatEvent::Error {
                message: e.to_string(),
            });
            Err(Error::Provider(e.to_string()))
        }
    }
}

// --- Knowledge graph / Mind Map --------------------------------------------

#[derive(Serialize)]
pub struct KgGraph {
    pub nodes: Vec<KgNode>,
    pub edges: Vec<KgEdge>,
}

#[tauri::command]
pub fn kg_graph(db: State<Db>) -> Result<KgGraph> {
    Ok(KgGraph {
        nodes: db.list_nodes()?,
        edges: db.list_edges()?,
    })
}

const EXTRACT_PROMPT: &str = "You are building a personal knowledge graph about the \
user from the conversation below. Extract durable facts about the user: people, \
projects, preferences, routines, places, health, and topics they care about. Ignore \
trivia and one-off chit-chat.\n\nReturn ONLY valid JSON (no prose, no code fences) with \
this exact shape:\n{\n  \"nodes\": [{\"id\": \"kebab-case-stable-id\", \"label\": \
\"Short Name\", \"group\": \"People|Projects|Preferences|Routines|Places|Health|Topics\", \
\"note\": \"one or two sentence note about why this matters to the user\"}],\n  \
\"edges\": [{\"source\": \"node-id\", \"target\": \"node-id\"}]\n}\nReuse the exact ids of \
existing nodes when referring to the same thing so they are updated, not duplicated. If \
there is nothing worth saving, return {\"nodes\":[],\"edges\":[]}.";

/// Extract knowledge from a conversation and merge it into the graph. Best-effort:
/// returns the number of nodes upserted. Safe to call after each chat turn.
#[tauri::command]
pub async fn kg_extract(db: State<'_, Db>, conversation_id: i64) -> Result<usize> {
    let config = load_config(&db)?;
    if config.model.is_empty() {
        return Ok(0);
    }
    let api_key = secrets::get_api_key(&config.provider)?;

    // Transcript of the conversation.
    let transcript: String = db
        .get_messages(conversation_id)?
        .iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");
    if transcript.trim().is_empty() {
        return Ok(0);
    }

    // Existing nodes, so the model reuses ids instead of duplicating.
    let existing: String = db
        .list_nodes()?
        .iter()
        .map(|n| format!("- {} ({}) [{}]", n.id, n.label, n.group))
        .collect::<Vec<_>>()
        .join("\n");

    let user_content = format!(
        "Existing nodes:\n{}\n\nConversation:\n{}",
        if existing.is_empty() { "(none)" } else { &existing },
        transcript
    );

    let turns = vec![
        ChatTurn {
            role: "system".into(),
            content: EXTRACT_PROMPT.into(),
        },
        ChatTurn {
            role: "user".into(),
            content: user_content,
        },
    ];

    let raw = providers::complete(
        &config.provider,
        &config.model,
        api_key,
        &config.ollama_host,
        &turns,
    )
    .await?;

    let parsed = match extract_json(&raw) {
        Some(v) => v,
        None => return Ok(0),
    };

    let mut count = 0usize;
    if let Some(nodes) = parsed.get("nodes").and_then(|n| n.as_array()) {
        for node in nodes {
            let label = node.get("label").and_then(|v| v.as_str()).unwrap_or("");
            if label.trim().is_empty() {
                continue;
            }
            let id = node
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(slugify)
                .unwrap_or_else(|| slugify(label));
            let group = node
                .get("group")
                .and_then(|v| v.as_str())
                .unwrap_or("Topics");
            let note = node.get("note").and_then(|v| v.as_str()).unwrap_or("");
            db.upsert_node(&id, label, group, note)?;
            count += 1;
        }
    }
    if let Some(edges) = parsed.get("edges").and_then(|e| e.as_array()) {
        for edge in edges {
            let source = edge.get("source").and_then(|v| v.as_str()).map(slugify);
            let target = edge.get("target").and_then(|v| v.as_str()).map(slugify);
            if let (Some(s), Some(t)) = (source, target) {
                if !s.is_empty() && !t.is_empty() && s != t {
                    db.add_edge(&s, &t)?;
                }
            }
        }
    }

    Ok(count)
}

/// Pull the first JSON object out of a possibly noisy model response.
fn extract_json(text: &str) -> Option<serde_json::Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

/// Normalize a label/id into a stable kebab-case identifier.
fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

// --- Integrations ----------------------------------------------------------

#[tauri::command]
pub fn integrations_status() -> Result<Vec<integrations::IntegrationStatus>> {
    integrations::status()
}

#[tauri::command]
pub fn google_set_client(client_id: String, client_secret: String) -> Result<()> {
    google::set_client(&client_id, &client_secret)
}

#[tauri::command]
pub async fn google_connect() -> Result<()> {
    google::connect().await
}

#[tauri::command]
pub fn google_disconnect() -> Result<()> {
    google::disconnect()
}

#[tauri::command]
pub async fn calendar_list_events(
    time_min: String,
    time_max: String,
) -> Result<Vec<google::CalendarEvent>> {
    google::list_events(&time_min, &time_max).await
}

#[tauri::command]
pub async fn calendar_create_event(
    event: google::CalendarEvent,
) -> Result<google::CalendarEvent> {
    google::create_event(&event).await
}

#[tauri::command]
pub async fn calendar_update_event(
    id: String,
    event: google::CalendarEvent,
) -> Result<google::CalendarEvent> {
    google::update_event(&id, &event).await
}

#[tauri::command]
pub async fn calendar_delete_event(id: String) -> Result<()> {
    google::delete_event(&id).await
}

#[tauri::command]
pub fn slack_set_token(token: String) -> Result<()> {
    slack::set_token(&token)
}

#[tauri::command]
pub fn slack_disconnect() -> Result<()> {
    slack::disconnect()
}

#[tauri::command]
pub async fn slack_list_channels() -> Result<Vec<slack::SlackChannel>> {
    slack::list_channels().await
}

#[tauri::command]
pub async fn slack_send_message(channel: String, text: String) -> Result<()> {
    slack::send_message(&channel, &text).await
}

#[tauri::command]
pub fn fathom_set_key(key: String) -> Result<()> {
    integrations::fathom::set_key(&key)
}

#[tauri::command]
pub fn fathom_disconnect() -> Result<()> {
    integrations::fathom::disconnect()
}
