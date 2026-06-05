//! Tauri commands exposed to the frontend over IPC.
//!
//! Covers Phase 1: app config, secure key management, model listing, chat history,
//! and streaming chat completions.

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

use crate::db::{Conversation, Db, Message};
use crate::error::{Error, Result};
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
