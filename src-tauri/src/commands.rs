//! Native-only Tauri commands.
//!
//! Since Task 10, the desktop is a thin client of `donna-server`: all data/logic commands
//! are routed to the server over RPC/WS (see `src/lib/api.ts`). Only commands that MUST run
//! in-process on this machine live here — screen capture, the Google OAuth loopback flow,
//! launching an editor, project file I/O, and pushing local keychain secrets to the server.
//! None of these open the local SQLite DB; the server owns the one brain / one DB.

use donna_core::{bundle, ops};
use std::collections::BTreeMap;

/// Wrappers surface errors to the frontend as plain strings across the IPC boundary.
type Result<T> = std::result::Result<T, String>;

// Re-export so the frontend-facing type keeps its old `commands::` path.
pub use ops::ProjectFile;

// --- Google OAuth (desktop-native loopback flow) -----------------------------

#[tauri::command]
pub async fn google_connect() -> Result<()> {
    ops::google_connect().await.map_err(|e| e.to_string())
}

/// Read the Google client + OAuth token from the local keychain so they can be pushed
/// to the server (which then makes Google API calls and refreshes tokens server-side).
#[tauri::command]
pub fn export_google_secrets() -> Result<GoogleSecrets> {
    use crate::secrets;
    let client = secrets::get_secret("google_client")
        .map_err(|e| e.to_string())?
        .ok_or("no google client configured")?;
    let token = secrets::get_secret("oauth:google")
        .map_err(|e| e.to_string())?
        .ok_or("not connected to Google")?;
    Ok(GoogleSecrets { client, token })
}

#[derive(serde::Serialize)]
pub struct GoogleSecrets {
    pub client: String,
    pub token: String,
}

/// Every keychain key the old (pre-Task-10) desktop app could have written. Used only
/// by `export_server_bundle` to migrate a pre-existing install onto donna-server — new
/// keys must be added here too, or that integration silently won't migrate.
const KNOWN_SECRET_KEYS: &[&str] = &[
    "api_key:openai",
    "api_key:anthropic",
    "api_key:google",
    "api_key:linear",
    "api_key:fathom",
    "google_client",
    "oauth:google",
    "token:slack",
    "token:github",
    "token:notion",
    "token:telegram",
    "chat_id:telegram",
    "token:whatsapp",
    "phone_id:whatsapp",
    "discord_bot_token",
];

/// Export the old desktop app's local data (SQLite DB, knowledge base, keychain
/// secrets) into a single tar.gz for `donna-server import`. Reads files by path and
/// the keychain directly — does not open a DB connection.
#[tauri::command]
pub async fn export_server_bundle(app: tauri::AppHandle, dest_dir: String) -> Result<String> {
    use crate::secrets;
    use tauri::Manager;

    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("donna.sqlite");
    let kb_dir = donna_core::knowledge::kb_root();

    let mut secrets_map = BTreeMap::new();
    for key in KNOWN_SECRET_KEYS {
        if let Some(value) = secrets::get_secret(key).map_err(|e| e.to_string())? {
            secrets_map.insert(key.to_string(), value);
        }
    }

    let out_path = std::path::Path::new(&dest_dir).join("donna-export.tar.gz");
    bundle::write_bundle(&out_path, &db_path, &kb_dir, &secrets_map).map_err(|e| e.to_string())?;
    Ok(out_path.to_string_lossy().to_string())
}

// --- Projects (native-only: filesystem / editor) -----------------------------

#[tauri::command]
pub async fn project_open_in_editor(path: String) -> Result<()> {
    // Try VS Code first, then Cursor, then system default
    let editors = ["cursor", "code", "zed"];
    for editor in &editors {
        if std::process::Command::new(editor).arg(&path).spawn().is_ok() {
            return Ok(());
        }
    }
    open::that(&path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn project_list_files(project_path: String) -> Result<Vec<ProjectFile>> {
    let root = std::path::Path::new(&project_path);
    let mut files = Vec::new();
    collect_files(root, root, &mut files, 0);
    Ok(files)
}

fn collect_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<ProjectFile>,
    depth: usize,
) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| {
        let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(false);
        (is_file as u8, e.file_name())
    });
    for entry in entries {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name != ".gitignore" {
            continue;
        }
        let rel = entry_path.strip_prefix(root).unwrap_or(&entry_path);
        let is_dir = entry_path.is_dir();
        out.push(ProjectFile {
            name: name.clone(),
            path: rel.to_string_lossy().to_string(),
            is_dir,
        });
        if is_dir {
            collect_files(root, &entry_path, out, depth + 1);
        }
    }
}

#[tauri::command]
pub async fn project_read_file(project_path: String, path: String) -> Result<String> {
    let full_path = std::path::Path::new(&project_path).join(&path);
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn project_write_file(project_path: String, path: String, content: String) -> Result<()> {
    let full_path = std::path::Path::new(&project_path).join(&path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&full_path, content).map_err(|e| e.to_string())
}

/// Native half of the status report: walk the project folder and return its readable file
/// contents. The server generates the report + saves it (it holds the provider config + DB).
#[tauri::command]
pub async fn project_status_report(project_path: String) -> Result<String> {
    let root = std::path::Path::new(&project_path);
    let mut file_contents = String::new();
    for entry in walkdir_shallow(root) {
        let content = std::fs::read_to_string(&entry).unwrap_or_default();
        if !content.is_empty() {
            file_contents.push_str(&format!("\n### {}\n{content}", entry.display()));
        }
    }
    Ok(file_contents)
}

fn walkdir_shallow(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else { return files; };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') { continue; }
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if matches!(ext.to_str(), Some("md" | "txt" | "rs" | "ts" | "tsx" | "py" | "js" | "toml")) {
                    files.push(path);
                }
            }
        }
    }
    files
}

// --- Quick Chat --------------------------------------------------------------

#[tauri::command]
pub fn quick_chat_context(
    state: tauri::State<'_, crate::quick_chat::QuickChatState>,
) -> crate::quick_chat::QuickChatContext {
    state.ctx.lock().unwrap().clone()
}
