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
            app.manage(db);
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
