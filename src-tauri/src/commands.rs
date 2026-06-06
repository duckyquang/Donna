//! Tauri commands exposed to the frontend over IPC.
//!
//! Covers Phase 1: app config, secure key management, model listing, chat history,
//! and streaming chat completions.

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

use crate::db::{Conversation, Db, Message};
use crate::error::{Error, Result};
use crate::integrations::{self, google, slack};
use crate::knowledge;
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

// --- Knowledge base / Mind Map ---------------------------------------------
//
// The knowledge base is a folder tree on disk (see `knowledge.rs`). The Mind Map UI
// consumes a flat node list where each node's `group` is its folder path, so categories
// and sub-folder branches render as clusters.

#[derive(Serialize)]
pub struct GraphNode {
    /// Globally-unique id: folder path + file id.
    pub id: String,
    pub label: String,
    /// Folder path joined with " / " — drives clustering/branching in the UI.
    pub group: String,
    pub note: String,
    pub updated_at: String,
    /// Raw folder path components, for editing/moving the node.
    pub folder: Vec<String>,
    /// File id (slug) within the folder.
    pub file_id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub has_image: bool,
}

#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<serde_json::Value>,
}

fn to_graph_node(n: knowledge::KbNode) -> GraphNode {
    GraphNode {
        id: format!("{}/{}", n.folder.join("/"), n.id),
        label: n.label,
        group: n.folder.join(" / "),
        note: n.note,
        updated_at: n.updated,
        folder: n.folder,
        file_id: n.id,
        node_type: n.node_type,
        has_image: n.has_image,
    }
}

#[tauri::command]
pub fn kg_graph() -> Result<GraphResponse> {
    let g = knowledge::graph()?;
    Ok(GraphResponse {
        nodes: g.nodes.into_iter().map(to_graph_node).collect(),
        edges: vec![],
    })
}

#[tauri::command]
pub fn kg_reset() -> Result<()> {
    knowledge::reset()
}

#[tauri::command]
pub fn kg_save_node(
    folder: Vec<String>,
    label: String,
    note: String,
    node_type: String,
    from_folder: Option<Vec<String>>,
    from_id: Option<String>,
) -> Result<GraphNode> {
    let node = knowledge::save_node(
        &folder,
        &label,
        &note,
        &node_type,
        from_folder.as_deref(),
        from_id.as_deref(),
    )?;
    Ok(to_graph_node(node))
}

#[tauri::command]
pub fn kg_delete_node(folder: Vec<String>, id: String) -> Result<()> {
    knowledge::delete_node(&folder, &id)
}

#[tauri::command]
pub fn kg_node_image(folder: Vec<String>, id: String) -> Result<Option<String>> {
    knowledge::node_image(&folder, &id)
}

#[tauri::command]
pub fn kg_set_node_image(folder: Vec<String>, id: String, source_path: String) -> Result<()> {
    knowledge::set_node_image(&folder, &id, &source_path)
}

#[tauri::command]
pub fn kg_remove_node_image(folder: Vec<String>, id: String) -> Result<()> {
    knowledge::remove_node_image(&folder, &id)
}

const CURATION_PROMPT: &str = "You are Donna's memory curator. Decide whether the \
conversation below contains durable, useful knowledge about the USER that is worth \
remembering long-term. Save ONLY things specifically about this user: facts about their \
life, work, or study; their routines; their stated preferences; explicit feedback they \
give Donna; and important people or projects in their life. DO NOT save general world \
knowledge, your own answers, or transient/trivial chit-chat. It is good and normal to \
save nothing.\n\nFor each thing worth keeping, choose a category (a folder) and \
optionally a sub-category (a branch), write a short label, classify the type \
(info|routine|feedback|preference|person|project), and write a 1-2 sentence note in your \
own words so you can recall and use it later. Reuse an existing category when it fits.\n\n\
Return ONLY valid JSON (no prose, no code fences):\n{\"memories\":[{\"category\":\
\"About You\",\"subcategory\":\"\",\"label\":\"Short label\",\"type\":\"info\",\"note\":\
\"What to remember and why it matters.\"}]}\nIf nothing qualifies, return \
{\"memories\":[]}.";

/// Ask Donna to decide what (if anything) from a conversation is worth remembering, then
/// write those memories into the folder-based knowledge base. Returns the count saved.
#[tauri::command]
pub async fn kg_extract(db: State<'_, Db>, conversation_id: i64) -> Result<usize> {
    let config = load_config(&db)?;
    if config.model.is_empty() {
        return Ok(0);
    }
    let api_key = secrets::get_api_key(&config.provider)?;

    let transcript: String = db
        .get_messages(conversation_id)?
        .iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");
    if transcript.trim().is_empty() {
        return Ok(0);
    }

    let categories = knowledge::categories()?.join(", ");
    let user_content = format!(
        "Existing categories: {}\n\nConversation:\n{}",
        if categories.is_empty() { "(none yet)".into() } else { categories },
        transcript
    );

    let turns = vec![
        ChatTurn {
            role: "system".into(),
            content: CURATION_PROMPT.into(),
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
    if let Some(memories) = parsed.get("memories").and_then(|m| m.as_array()) {
        for mem in memories {
            let label = mem.get("label").and_then(|v| v.as_str()).unwrap_or("").trim();
            if label.is_empty() {
                continue;
            }
            let category = mem
                .get("category")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("About You");
            let note = mem.get("note").and_then(|v| v.as_str()).unwrap_or("");
            let node_type = mem.get("type").and_then(|v| v.as_str()).unwrap_or("info");

            let mut folder = vec![category.to_string()];
            if let Some(sub) = mem
                .get("subcategory")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                folder.push(sub.to_string());
            }

            knowledge::save_node(&folder, label, note, node_type, None, None)?;
            count += 1;
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
