//! Donna Rust core.
//!
//! Sets up local SQLite storage, registers Tauri commands, and runs the desktop app.

mod commands;
mod quick_chat;
mod embedded_server;

pub use donna_core::{error, knowledge, secrets};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            use tauri::Manager;

            // Create the local knowledge-base folder tree on first run.
            let _ = knowledge::ensure_root();

            // ponytail: the desktop is now a thin client of donna-server — it no longer
            // opens the local SQLite DB or warms a model; the server owns all data + logic.
            // Only quick-chat window state and the Cmd+D shortcut live here.
            app.manage(crate::quick_chat::QuickChatState::default());

            // Embedded brain: spawn the bundled donna-server so end users need zero setup.
            app.manage(crate::embedded_server::EmbeddedState::default());
            crate::embedded_server::start(app.handle().clone());

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

            // ponytail: one brain, one scheduler — donna-server runs routines (Task 9);
            // the desktop app no longer starts its own scheduler loop.

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        // Native-only commands. Everything else is served by donna-server over RPC/WS.
        .invoke_handler(tauri::generate_handler![
            commands::quick_chat_context,
            commands::google_set_client,
            commands::google_connect,
            commands::export_google_secrets,
            commands::export_server_bundle,
            commands::project_open_in_editor,
            commands::project_list_files,
            commands::project_read_file,
            commands::project_write_file,
            commands::project_status_report,
            embedded_server::embedded_server_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Donna")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                crate::embedded_server::kill(app);
            }
        });
}
