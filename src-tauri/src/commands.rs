//! Tauri commands exposed to the frontend over IPC.
//!
//! These are thin wrappers over `donna_core::ops`: they adapt Tauri's `State<Db>` and
//! streaming `Channel` to plain args + a callback, then map core errors to strings for
//! the IPC boundary. All real logic lives in `donna_core::ops`. Purely-native commands
//! (screen capture, editor launch, project file I/O, status reports) stay whole here.

use tauri::ipc::Channel;
use tauri::State;

use crate::db::Db;
use crate::integrations::google;

use donna_core::ops;

/// Wrappers surface errors to the frontend as plain strings across the IPC boundary.
type Result<T> = std::result::Result<T, String>;

// Re-exports so the frontend-facing types keep their old `commands::` paths and the
// generated IPC bindings still resolve.
pub use ops::{
    AppConfig, ChatEvent, CreateRoutineInput, GraphNode, GraphResponse, ProjectFile,
};

// --- Config -----------------------------------------------------------------

#[tauri::command]
pub fn get_config(db: State<Db>) -> Result<AppConfig> {
    ops::get_config(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn basics_status() -> Result<Vec<crate::knowledge::BasicFieldStatus>> {
    ops::basics_status().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(db: State<Db>, config: AppConfig) -> Result<()> {
    ops::save_config(&db, config).map_err(|e| e.to_string())
}

// --- Secrets ----------------------------------------------------------------

#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<()> {
    ops::set_api_key(provider, key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn has_api_key(provider: String) -> Result<bool> {
    ops::has_api_key(provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_api_key(provider: String) -> Result<()> {
    ops::delete_api_key(provider).map_err(|e| e.to_string())
}

// --- Models -----------------------------------------------------------------

#[tauri::command]
pub async fn list_models(db: State<'_, Db>, provider: String) -> Result<Vec<String>> {
    ops::list_models(&db, provider).await.map_err(|e| e.to_string())
}

// --- Conversations & messages ----------------------------------------------

#[tauri::command]
pub fn create_conversation(db: State<Db>, title: String) -> Result<i64> {
    ops::create_conversation(&db, title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_conversations(db: State<Db>) -> Result<Vec<crate::db::Conversation>> {
    ops::list_conversations(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_conversation(db: State<Db>, id: i64, title: String) -> Result<()> {
    ops::rename_conversation(&db, id, title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_conversation(db: State<Db>, id: i64) -> Result<()> {
    ops::delete_conversation(&db, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_messages(db: State<Db>, conversation_id: i64) -> Result<Vec<crate::db::Message>> {
    ops::get_messages(&db, conversation_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_message(
    db: State<Db>,
    conversation_id: i64,
    role: String,
    content: String,
) -> Result<i64> {
    ops::add_message(&db, conversation_id, role, content).map_err(|e| e.to_string())
}

// --- Streaming chat ---------------------------------------------------------

#[tauri::command]
pub async fn send_chat(
    db: State<'_, Db>,
    conversation_id: i64,
    on_event: Channel<ChatEvent>,
) -> Result<()> {
    ops::send_chat(&db, conversation_id, &move |ev| {
        let _ = on_event.send(ev);
    })
    .await
    .map_err(|e| e.to_string())
}

// --- Knowledge base / Mind Map ---------------------------------------------

#[tauri::command]
pub fn kg_graph() -> Result<GraphResponse> {
    ops::kg_graph().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kg_reset(db: State<Db>) -> Result<()> {
    ops::kg_reset(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kg_save_node(
    db: State<'_, Db>,
    folder: Vec<String>,
    label: String,
    note: String,
    node_type: String,
    from_folder: Option<Vec<String>>,
    from_id: Option<String>,
) -> Result<GraphNode> {
    ops::kg_save_node(&db, folder, label, note, node_type, from_folder, from_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kg_delete_node(folder: Vec<String>, id: String) -> Result<()> {
    ops::kg_delete_node(folder, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kg_node_image(folder: Vec<String>, id: String) -> Result<Option<String>> {
    ops::kg_node_image(folder, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kg_set_node_image(folder: Vec<String>, id: String, source_path: String) -> Result<()> {
    ops::kg_set_node_image(folder, id, source_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kg_remove_node_image(folder: Vec<String>, id: String) -> Result<()> {
    ops::kg_remove_node_image(folder, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kg_extract(db: State<'_, Db>, conversation_id: i64) -> Result<usize> {
    ops::kg_extract(&db, conversation_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kg_reindex_embeddings(db: State<'_, Db>) -> Result<usize> {
    ops::kg_reindex_embeddings(&db).await.map_err(|e| e.to_string())
}

// --- Integrations ----------------------------------------------------------

#[tauri::command]
pub fn integrations_status() -> Result<Vec<crate::integrations::IntegrationStatus>> {
    ops::integrations_status().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn google_set_client(client_id: String, client_secret: String) -> Result<()> {
    ops::google_set_client(client_id, client_secret).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn google_connect() -> Result<()> {
    ops::google_connect().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn google_disconnect() -> Result<()> {
    ops::google_disconnect().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn calendar_list_events(
    time_min: String,
    time_max: String,
) -> Result<Vec<google::CalendarEvent>> {
    ops::calendar_list_events(time_min, time_max).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn calendar_create_event(
    event: google::CalendarEvent,
) -> Result<google::CalendarEvent> {
    ops::calendar_create_event(event).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn calendar_update_event(
    id: String,
    event: google::CalendarEvent,
) -> Result<google::CalendarEvent> {
    ops::calendar_update_event(id, event).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn calendar_delete_event(id: String) -> Result<()> {
    ops::calendar_delete_event(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn slack_set_token(token: String) -> Result<()> {
    ops::slack_set_token(token).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn slack_disconnect() -> Result<()> {
    ops::slack_disconnect().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn slack_list_channels() -> Result<Vec<crate::integrations::slack::SlackChannel>> {
    ops::slack_list_channels().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn slack_send_message(channel: String, text: String) -> Result<()> {
    ops::slack_send_message(channel, text).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fathom_set_key(key: String) -> Result<()> {
    ops::fathom_set_key(key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fathom_disconnect() -> Result<()> {
    ops::fathom_disconnect().map_err(|e| e.to_string())
}

// --- Routines ---------------------------------------------------------------

#[tauri::command]
pub fn list_routines(db: State<Db>) -> Result<Vec<crate::db::Routine>> {
    ops::list_routines(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_routine(db: State<Db>, id: i64, enabled: bool) -> Result<()> {
    ops::toggle_routine(&db, id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_routine(db: State<Db>, input: CreateRoutineInput) -> Result<i64> {
    ops::create_routine(&db, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_routine(db: State<Db>, id: i64) -> Result<()> {
    ops::delete_routine(&db, id).map_err(|e| e.to_string())
}

// --- Notifications ----------------------------------------------------------

#[tauri::command]
pub fn list_notifications(db: State<Db>) -> Result<Vec<crate::db::Notification>> {
    ops::list_notifications(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_notification_read(db: State<Db>, id: i64) -> Result<()> {
    ops::mark_notification_read(&db, id).map_err(|e| e.to_string())
}

// --- Docs -------------------------------------------------------------------

#[tauri::command]
pub fn list_docs(db: State<Db>) -> Result<Vec<crate::db::Doc>> {
    ops::list_docs(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_doc(db: State<Db>, id: i64) -> Result<crate::db::Doc> {
    ops::get_doc(&db, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_doc(db: State<Db>, id: i64) -> Result<()> {
    ops::delete_doc(&db, id).map_err(|e| e.to_string())
}

// --- Gmail ------------------------------------------------------------------

#[tauri::command]
pub async fn gmail_list_messages(max_results: u32) -> Result<Vec<google::GmailMessage>> {
    ops::gmail_list_messages(max_results).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn google_create_doc(title: String) -> Result<String> {
    ops::google_create_doc(title).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn gmail_create_draft(to: String, subject: String, body: String) -> Result<String> {
    ops::gmail_create_draft(to, subject, body).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn drive_list_files(max_results: u32) -> Result<Vec<google::DriveFile>> {
    ops::drive_list_files(max_results).await.map_err(|e| e.to_string())
}

// --- GitHub -----------------------------------------------------------------

#[tauri::command]
pub fn github_set_token(token: String) -> Result<()> {
    ops::github_set_token(token).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn github_disconnect() -> Result<()> {
    ops::github_disconnect().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn github_list_repos(max_results: u32) -> Result<Vec<crate::integrations::github::GitHubRepo>> {
    ops::github_list_repos(max_results).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn github_list_issues(max_results: u32) -> Result<Vec<crate::integrations::github::GitHubIssue>> {
    ops::github_list_issues(max_results).await.map_err(|e| e.to_string())
}

// --- Linear -----------------------------------------------------------------

#[tauri::command]
pub fn linear_set_key(key: String) -> Result<()> {
    ops::linear_set_key(key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn linear_disconnect() -> Result<()> {
    ops::linear_disconnect().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn linear_list_issues(max_results: u32) -> Result<Vec<crate::integrations::linear::LinearIssue>> {
    ops::linear_list_issues(max_results).await.map_err(|e| e.to_string())
}

// --- Notion -----------------------------------------------------------------

#[tauri::command]
pub fn notion_set_token(token: String) -> Result<()> {
    ops::notion_set_token(token).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notion_disconnect() -> Result<()> {
    ops::notion_disconnect().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn notion_search_pages(max_results: u32) -> Result<Vec<crate::integrations::notion::NotionPage>> {
    ops::notion_search_pages(max_results).await.map_err(|e| e.to_string())
}

// --- Telegram ---------------------------------------------------------------

#[tauri::command]
pub fn telegram_set_credentials(bot_token: String, chat_id: String) -> Result<()> {
    ops::telegram_set_credentials(bot_token, chat_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn telegram_disconnect() -> Result<()> {
    ops::telegram_disconnect().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn telegram_send_message(text: String) -> Result<()> {
    ops::telegram_send_message(text).await.map_err(|e| e.to_string())
}

// --- WhatsApp ---------------------------------------------------------------

#[tauri::command]
pub fn whatsapp_set_credentials(access_token: String, phone_number_id: String) -> Result<()> {
    ops::whatsapp_set_credentials(access_token, phone_number_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn whatsapp_disconnect() -> Result<()> {
    ops::whatsapp_disconnect().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn whatsapp_send_message(to: String, text: String) -> Result<()> {
    ops::whatsapp_send_message(to, text).await.map_err(|e| e.to_string())
}

// --- Projects (DB-side) ------------------------------------------------------

#[tauri::command]
pub async fn project_list(db: State<'_, Db>) -> Result<Vec<crate::db::Project>> {
    ops::project_list(&db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn project_create(
    db: State<'_, Db>,
    name: String,
    template: String,
    path: String,
) -> Result<crate::db::Project> {
    ops::project_create(&db, name, template, path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn project_delete(db: State<'_, Db>, id: i64) -> Result<()> {
    ops::project_delete(&db, id).await.map_err(|e| e.to_string())
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
pub async fn project_list_files(project_id: i64, db: State<'_, Db>) -> Result<Vec<ProjectFile>> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Ok(vec![]);
    };
    let root = std::path::Path::new(&project.path);
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
pub async fn project_read_file(project_id: i64, path: String, db: State<'_, Db>) -> Result<String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Err("Project not found".to_string());
    };
    let full_path = std::path::Path::new(&project.path).join(&path);
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn project_write_file(project_id: i64, path: String, content: String, db: State<'_, Db>) -> Result<()> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Err("Project not found".to_string());
    };
    let full_path = std::path::Path::new(&project.path).join(&path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&full_path, content).map_err(|e| e.to_string())
}

// --- Discord -----------------------------------------------------------------

#[tauri::command]
pub async fn discord_set_token(token: String) -> Result<()> {
    ops::discord_set_token(token).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn discord_disconnect() -> Result<()> {
    ops::discord_disconnect().await.map_err(|e| e.to_string())
}

// --- Fathom post-meeting processing -----------------------------------------

#[tauri::command]
pub async fn fathom_process_recent_meeting(db: State<'_, Db>) -> Result<String> {
    ops::fathom_process_recent_meeting(&db).await.map_err(|e| e.to_string())
}

// --- News --------------------------------------------------------------------

#[tauri::command]
pub async fn news_fetch_latest() -> Result<String> {
    ops::news_fetch_latest().await.map_err(|e| e.to_string())
}

// --- Reading list ------------------------------------------------------------

#[tauri::command]
pub async fn reading_list_add(db: State<'_, Db>, url: String, title: String) -> Result<crate::db::ReadingListItem> {
    ops::reading_list_add(&db, url, title).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reading_list_get(db: State<'_, Db>) -> Result<Vec<crate::db::ReadingListItem>> {
    ops::reading_list_get(&db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reading_list_summarize(db: State<'_, Db>, id: i64) -> Result<String> {
    ops::reading_list_summarize(&db, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reading_list_delete(db: State<'_, Db>, id: i64) -> Result<()> {
    ops::reading_list_delete(&db, id).await.map_err(|e| e.to_string())
}

// --- Focus sessions ----------------------------------------------------------

#[tauri::command]
pub async fn focus_start(db: State<'_, Db>, label: String, duration_min: i32) -> Result<crate::db::FocusSession> {
    ops::focus_start(&db, label, duration_min).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn focus_end(db: State<'_, Db>, id: i64) -> Result<()> {
    ops::focus_end(&db, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn focus_active(db: State<'_, Db>) -> Result<Option<crate::db::FocusSession>> {
    ops::focus_active(&db).await.map_err(|e| e.to_string())
}

// --- Habits ------------------------------------------------------------------

#[tauri::command]
pub async fn habit_create(db: State<'_, Db>, name: String, description: Option<String>) -> Result<crate::db::Habit> {
    ops::habit_create(&db, name, description).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn habit_list(db: State<'_, Db>) -> Result<Vec<crate::db::Habit>> {
    ops::habit_list(&db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn habit_log(db: State<'_, Db>, habit_id: i64, note: Option<String>) -> Result<()> {
    ops::habit_log(&db, habit_id, note).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn habit_logged_today(db: State<'_, Db>, habit_id: i64) -> Result<bool> {
    ops::habit_logged_today(&db, habit_id).await.map_err(|e| e.to_string())
}

// --- Project status report (native-only: filesystem walk) --------------------

#[tauri::command]
pub async fn project_status_report(db: State<'_, Db>, project_id: i64) -> Result<String> {
    use crate::providers::{self, ChatTurn};
    use crate::secrets;

    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Err("Project not found".to_string());
    };
    let provider = db.get_setting("provider").map_err(|e| e.to_string())?.unwrap_or_else(|| "ollama".into());
    let model = db.get_setting("model").map_err(|e| e.to_string())?.unwrap_or_default();
    let ollama_host = db.get_setting("ollama_host").map_err(|e| e.to_string())?.unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider).map_err(|e| e.to_string())?;

    // Collect file contents for context
    let root = std::path::Path::new(&project.path);
    let mut file_contents = String::new();
    for entry in walkdir_shallow(root) {
        let content = std::fs::read_to_string(&entry).unwrap_or_default();
        if !content.is_empty() {
            file_contents.push_str(&format!("\n### {}\n{content}", entry.display()));
        }
    }

    let turns = vec![
        ChatTurn { role: "system".into(), content: "Generate a concise project status report with: current status, what's done, what's in progress, blockers, and next steps. Use Markdown.".into() },
        ChatTurn { role: "user".into(), content: format!("Project: {}\nTemplate: {}\n\nFiles:\n{file_contents}", project.name, project.template) },
    ];
    let report = providers::complete(&provider, &model, api_key, &ollama_host, &turns).await.map_err(|e| e.to_string())?;
    let doc_id = crate::docs::create(&db, &format!("Status Report: {}", project.name), "project_status", &report).map_err(|e| e.to_string())?;
    db.insert_notification(
        &format!("Status report: {}", project.name),
        "Project status report is ready in Docs.",
        Some("open_doc"),
        Some(doc_id),
    ).map_err(|e| e.to_string())?;
    Ok(report)
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

#[tauri::command]
pub async fn quick_chat_send(
    db: State<'_, Db>,
    message: String,
    app_name: String,
    on_event: Channel<ChatEvent>,
) -> Result<()> {
    ops::quick_chat_send(&db, message, app_name, &move |ev| {
        let _ = on_event.send(ev);
    })
    .await
    .map_err(|e| e.to_string())
}

// --- News items (structured) --------------------------------------------------

#[tauri::command]
pub async fn news_list_items(
    limit: Option<usize>,
) -> Result<Vec<crate::integrations::news::NewsItem>> {
    ops::news_list_items(limit).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn news_article_summary(db: State<'_, Db>, url: String) -> Result<String> {
    ops::news_article_summary(&db, url).await.map_err(|e| e.to_string())
}
