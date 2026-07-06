//! Donna Rust core.
//!
//! Sets up local SQLite storage, registers Tauri commands, and runs the desktop app.

mod commands;
mod quick_chat;
mod scheduler;

pub use donna_core::{db, docs, embeddings, error, integrations, knowledge, oauth, providers, retrieval, secrets};

use db::Db;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // Create the local knowledge-base folder tree on first run.
            let _ = knowledge::ensure_root();

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

            // Quick-chat state
            app.manage(crate::quick_chat::QuickChatState::default());

            // Register Cmd+D global shortcut for the quick-chat overlay
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                let handle = app.handle().clone();
                app.global_shortcut().on_shortcut("CmdOrCtrl+D", move |_app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let handle = handle.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            // Capture screen context in background thread
                            let ctx = crate::quick_chat::capture_context();
                            if let Some(state) = handle.try_state::<crate::quick_chat::QuickChatState>() {
                                *state.ctx.lock().unwrap() = ctx;
                            }
                            let _ = crate::quick_chat::open_quick_chat_window(&handle);
                        });
                    }
                })?;
            }

            if let Some((host, model)) = ollama_warmup {
                tauri::async_runtime::spawn(async move {
                    let _ = providers::warm_ollama_model(&host, &model).await;
                });
            }

            scheduler::run_loop(app.handle().clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::basics_status,
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
            commands::kg_graph,
            commands::kg_extract,
            commands::kg_reset,
            commands::kg_save_node,
            commands::kg_delete_node,
            commands::kg_node_image,
            commands::kg_set_node_image,
            commands::kg_remove_node_image,
            commands::kg_reindex_embeddings,
            commands::integrations_status,
            commands::google_set_client,
            commands::google_connect,
            commands::google_disconnect,
            commands::calendar_list_events,
            commands::calendar_create_event,
            commands::calendar_update_event,
            commands::calendar_delete_event,
            commands::slack_set_token,
            commands::slack_disconnect,
            commands::slack_list_channels,
            commands::slack_send_message,
            commands::fathom_set_key,
            commands::fathom_disconnect,
            commands::list_routines,
            commands::toggle_routine,
            commands::create_routine,
            commands::delete_routine,
            commands::list_notifications,
            commands::mark_notification_read,
            commands::list_docs,
            commands::get_doc,
            commands::delete_doc,
            commands::gmail_list_messages,
            commands::google_create_doc,
            commands::gmail_create_draft,
            commands::drive_list_files,
            commands::github_set_token,
            commands::github_disconnect,
            commands::github_list_repos,
            commands::github_list_issues,
            commands::linear_set_key,
            commands::linear_disconnect,
            commands::linear_list_issues,
            commands::notion_set_token,
            commands::notion_disconnect,
            commands::notion_search_pages,
            commands::telegram_set_credentials,
            commands::telegram_disconnect,
            commands::telegram_send_message,
            commands::whatsapp_set_credentials,
            commands::whatsapp_disconnect,
            commands::whatsapp_send_message,
            commands::project_list,
            commands::project_create,
            commands::project_delete,
            commands::project_open_in_editor,
            commands::project_list_files,
            commands::project_read_file,
            commands::project_write_file,
            commands::discord_set_token,
            commands::discord_disconnect,
            commands::fathom_process_recent_meeting,
            commands::news_fetch_latest,
            commands::reading_list_add,
            commands::reading_list_get,
            commands::reading_list_summarize,
            commands::reading_list_delete,
            commands::focus_start,
            commands::focus_end,
            commands::focus_active,
            commands::habit_create,
            commands::habit_list,
            commands::habit_log,
            commands::habit_logged_today,
            commands::project_status_report,
            commands::quick_chat_context,
            commands::quick_chat_send,
            commands::news_list_items,
            commands::news_article_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Donna");
}
