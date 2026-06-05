//! Donna Rust core.
//!
//! Hosts the Tauri commands the frontend calls over IPC. These are Phase-0 stubs that
//! define the surface area; real implementations land in later phases (see CONTEXT.md).

mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::chat,
            commands::list_models,
            commands::schedule_routine,
            commands::init_db,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Donna");
}
