//! Tauri commands exposed to the frontend.
//!
//! Phase-0 stubs: signatures are stable; bodies are filled in during Phase 1+.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Send a chat turn to the active model provider and return the response.
/// TODO(phase-1): route to the selected provider (Ollama or cloud) and stream output.
#[tauri::command]
pub fn chat(_messages: Vec<ChatMessage>, _model: String) -> Result<String, String> {
    Err("chat not implemented yet (Phase 1)".into())
}

/// List models available for a given provider id.
/// TODO(phase-1): query Ollama tags or the cloud provider's model list.
#[tauri::command]
pub fn list_models(_provider: String) -> Result<Vec<String>, String> {
    Ok(vec![])
}

/// Register a proactive routine with the background scheduler.
/// TODO(phase-3): persist the routine and wire it into the scheduler.
#[tauri::command]
pub fn schedule_routine(_name: String, _cron: String) -> Result<(), String> {
    Ok(())
}

/// Initialize the local SQLite database (memory, docs, settings).
/// TODO(phase-1): create the database file and run migrations.
#[tauri::command]
pub fn init_db() -> Result<(), String> {
    Ok(())
}
