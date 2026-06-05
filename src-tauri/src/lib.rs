//! Donna Rust core.
//!
//! Sets up local SQLite storage, registers Tauri commands, and runs the desktop app.

mod commands;
mod db;
mod error;
mod providers;
mod secrets;

use db::Db;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Store the database under the OS-appropriate app data directory.
            let dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let db = Db::open(&dir.join("donna.sqlite"))
                .expect("failed to open Donna database");

            // Preload the local model while the UI renders so the first reply is faster.
            let provider = db
                .get_setting("provider")
                .ok()
                .flatten()
                .unwrap_or_else(|| "ollama".into());
            let ollama_warmup = if provider == "ollama" {
                match db.get_setting("model").ok().flatten() {
                    Some(model) if !model.is_empty() => {
                        let host = db
                            .get_setting("ollama_host")
                            .ok()
                            .flatten()
                            .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
                        Some((host, model))
                    }
                    _ => None,
                }
            } else {
                None
            };

            app.manage(db);

            if let Some((host, model)) = ollama_warmup {
                tauri::async_runtime::spawn(async move {
                    let _ = providers::warm_ollama_model(&host, &model).await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::set_api_key,
            commands::has_api_key,
            commands::delete_api_key,
            commands::list_models,
            commands::create_conversation,
            commands::list_conversations,
            commands::rename_conversation,
            commands::delete_conversation,
            commands::get_messages,
            commands::add_message,
            commands::send_chat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Donna");
}
