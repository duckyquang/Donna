//! JSON-RPC-ish command dispatch: `POST /rpc/:command` with a JSON body.
//!
//! The body is the same camelCase args object the Tauri UI passes to `invoke(cmd, args)`.
//! Each arm mirrors a `src-tauri` command wrapper: same `ops::` fn, same argument order.
//! Arg keys are the exact camelCase keys the frontend sends (see `src/lib/api.ts`).
//!
//! Excluded (no arm): the streaming pair `send_chat`/`quick_chat_send` (WS-only, Task 9),
//! the native-only commands with no ops fn (`quick_chat_context`, `project_open_in_editor`,
//! `project_list_files`, `project_read_file`, `project_write_file`), and `google_connect`
//! (desktop-native OAuth). `project_status_report` is a hybrid: the desktop walks the local
//! project folder natively, then this arm generates + saves the report from those contents.

use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde_json::{json, Value};
use crate::state::AppState;
use donna_core::ops;

/// Deserialize `key` from the args object, defaulting a missing key to JSON null so
/// `Option<T>` args round-trip. Errors carry the key name for a readable 400 body.
fn arg<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(v.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("bad arg {key}: {e}"))
}

/// Run an ops call, turning both its domain error and any serialization error into a
/// `String` (mapped to a 400 by `handle`). Success serializes the result to JSON verbatim.
macro_rules! ok {
    ($e:expr) => {
        $e.map_err(|x| x.to_string())
            .and_then(|v| serde_json::to_value(v).map_err(|x| x.to_string()))
    };
}

pub async fn handle(
    State(st): State<AppState>,
    Path(cmd): Path<String>,
    Json(a): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match dispatch(&st, &cmd, &a).await {
        Ok(v) => Ok(Json(v)),
        // A sentinel error string flags an unknown command as a 404; everything else is a 400.
        Err(e) if e == UNKNOWN => Err((StatusCode::NOT_FOUND, Json(json!({"error": format!("unknown command {cmd}")})))),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"error": e})))),
    }
}

const UNKNOWN: &str = "\0unknown\0";

async fn dispatch(st: &AppState, cmd: &str, a: &Value) -> Result<Value, String> {
    let db = &st.db;
    let out: Result<Value, String> = match cmd {
        // --- Config ---
        "get_config" => ok!(ops::get_config(db)),
        "basics_status" => ok!(ops::basics_status()),
        "save_config" => ok!(ops::save_config(db, arg(&a, "config")?)),

        // --- Secrets ---
        "set_api_key" => ok!(ops::set_api_key(arg(&a, "provider")?, arg(&a, "key")?)),
        "has_api_key" => ok!(ops::has_api_key(arg(&a, "provider")?)),
        "delete_api_key" => ok!(ops::delete_api_key(arg(&a, "provider")?)),

        // --- Models ---
        "list_models" => ok!(ops::list_models(db, arg(&a, "provider")?).await),

        // --- Conversations & messages ---
        // `title` tolerates a missing key (defaults to empty) so a bare `{}` still creates a row.
        "create_conversation" => ok!(ops::create_conversation(db, arg::<Option<String>>(&a, "title")?.unwrap_or_default())),
        "list_conversations" => ok!(ops::list_conversations(db)),
        "rename_conversation" => ok!(ops::rename_conversation(db, arg(&a, "id")?, arg(&a, "title")?)),
        "delete_conversation" => ok!(ops::delete_conversation(db, arg(&a, "id")?)),
        "get_messages" => ok!(ops::get_messages(db, arg(&a, "conversationId")?)),
        "add_message" => ok!(ops::add_message(db, arg(&a, "conversationId")?, arg(&a, "role")?, arg(&a, "content")?)),

        // --- Knowledge base / Mind Map ---
        "kg_graph" => ok!(ops::kg_graph()),
        "kg_reset" => ok!(ops::kg_reset(db)),
        "kg_save_node" => ok!(ops::kg_save_node(
            db,
            arg(&a, "folder")?,
            arg(&a, "label")?,
            arg(&a, "note")?,
            arg(&a, "nodeType")?,
            arg(&a, "fromFolder")?,
            arg(&a, "fromId")?,
        ).await),
        "kg_delete_node" => ok!(ops::kg_delete_node(arg(&a, "folder")?, arg(&a, "id")?)),
        "kg_node_image" => ok!(ops::kg_node_image(arg(&a, "folder")?, arg(&a, "id")?)),
        "kg_set_node_image" => ok!(ops::kg_set_node_image(arg(&a, "folder")?, arg(&a, "id")?, arg(&a, "sourcePath")?)),
        "kg_remove_node_image" => ok!(ops::kg_remove_node_image(arg(&a, "folder")?, arg(&a, "id")?)),
        "kg_extract" => ok!(ops::kg_extract(db, arg(&a, "conversationId")?).await),
        "kg_reindex_embeddings" => ok!(ops::kg_reindex_embeddings(db).await),

        // --- Integrations ---
        "integrations_status" => ok!(ops::integrations_status()),
        "google_set_client" => ok!(ops::google_set_client(arg(&a, "clientId")?, arg(&a, "clientSecret")?)),
        "google_disconnect" => ok!(ops::google_disconnect()),
        "import_google_secrets" => ok!(ops::import_google_secrets(arg(&a, "client")?, arg(&a, "token")?)),

        // --- Calendar ---
        "calendar_list_events" => ok!(ops::calendar_list_events(arg(&a, "timeMin")?, arg(&a, "timeMax")?).await),
        "calendar_create_event" => ok!(ops::calendar_create_event(arg(&a, "event")?).await),
        "calendar_update_event" => ok!(ops::calendar_update_event(arg(&a, "id")?, arg(&a, "event")?).await),
        "calendar_delete_event" => ok!(ops::calendar_delete_event(arg(&a, "id")?).await),

        // --- Slack ---
        "slack_set_token" => ok!(ops::slack_set_token(arg(&a, "token")?)),
        "slack_disconnect" => ok!(ops::slack_disconnect()),
        "slack_list_channels" => ok!(ops::slack_list_channels().await),
        "slack_send_message" => ok!(ops::slack_send_message(arg(&a, "channel")?, arg(&a, "text")?).await),

        // --- Fathom ---
        "fathom_set_key" => ok!(ops::fathom_set_key(arg(&a, "key")?)),
        "fathom_disconnect" => ok!(ops::fathom_disconnect()),
        "fathom_process_recent_meeting" => ok!(ops::fathom_process_recent_meeting(db).await),

        // --- Routines ---
        "list_routines" => ok!(ops::list_routines(db)),
        "toggle_routine" => ok!(ops::toggle_routine(db, arg(&a, "id")?, arg(&a, "enabled")?)),
        "create_routine" => ok!(ops::create_routine(db, arg(&a, "input")?)),
        "delete_routine" => ok!(ops::delete_routine(db, arg(&a, "id")?)),

        // --- Notifications ---
        "list_notifications" => ok!(ops::list_notifications(db)),
        "mark_notification_read" => ok!(ops::mark_notification_read(db, arg(&a, "id")?)),

        // --- Approvals & trust policies ---
        "approvals_list" => ok!(ops::approvals_list(db)),
        "approvals_pending_for_conversation" => ok!(ops::approvals_pending_for_conversation(db, arg(&a, "conversationId")?)),
        "approval_respond" => ok!(ops::approval_respond(db, arg(&a, "id")?, arg(&a, "approve")?).await),
        "trust_policies_list" => ok!(ops::trust_policies_list(db)),
        "trust_policy_set" => ok!(ops::trust_policy_set(db, arg(&a, "actionKind")?, arg(&a, "mode")?)),

        // --- Docs ---
        "list_docs" => ok!(ops::list_docs(db)),
        "get_doc" => ok!(ops::get_doc(db, arg(&a, "id")?)),
        "delete_doc" => ok!(ops::delete_doc(db, arg(&a, "id")?)),

        // --- Gmail & Drive ---
        "gmail_list_messages" => ok!(ops::gmail_list_messages(arg(&a, "maxResults")?).await),
        "gmail_create_draft" => ok!(ops::gmail_create_draft(arg(&a, "to")?, arg(&a, "subject")?, arg(&a, "body")?).await),
        "drive_list_files" => ok!(ops::drive_list_files(arg(&a, "maxResults")?).await),
        "google_create_doc" => ok!(ops::google_create_doc(arg(&a, "title")?).await),

        // --- GitHub ---
        "github_set_token" => ok!(ops::github_set_token(arg(&a, "token")?)),
        "github_disconnect" => ok!(ops::github_disconnect()),
        "github_list_repos" => ok!(ops::github_list_repos(arg(&a, "maxResults")?).await),
        "github_list_issues" => ok!(ops::github_list_issues(arg(&a, "maxResults")?).await),

        // --- Linear ---
        "linear_set_key" => ok!(ops::linear_set_key(arg(&a, "key")?)),
        "linear_disconnect" => ok!(ops::linear_disconnect()),
        "linear_list_issues" => ok!(ops::linear_list_issues(arg(&a, "maxResults")?).await),

        // --- Notion ---
        "notion_set_token" => ok!(ops::notion_set_token(arg(&a, "token")?)),
        "notion_disconnect" => ok!(ops::notion_disconnect()),
        "notion_search_pages" => ok!(ops::notion_search_pages(arg(&a, "maxResults")?).await),

        // --- Telegram ---
        "telegram_set_credentials" => ok!(ops::telegram_set_credentials(arg(&a, "botToken")?, arg(&a, "chatId")?)),
        "telegram_disconnect" => ok!(ops::telegram_disconnect()),
        "telegram_send_message" => ok!(ops::telegram_send_message(arg(&a, "text")?).await),

        // --- WhatsApp ---
        "whatsapp_set_credentials" => ok!(ops::whatsapp_set_credentials(arg(&a, "accessToken")?, arg(&a, "phoneNumberId")?)),
        "whatsapp_disconnect" => ok!(ops::whatsapp_disconnect()),
        "whatsapp_send_message" => ok!(ops::whatsapp_send_message(arg(&a, "to")?, arg(&a, "text")?).await),
        "whatsapp_set_my_number" => ok!(ops::whatsapp_set_my_number(db, arg(&a, "number")?)),
        "whatsapp_get_my_number" => ok!(ops::whatsapp_get_my_number(db)),

        // --- Projects (DB-side) ---
        "project_list" => ok!(ops::project_list(db).await),
        "project_create" => ok!(ops::project_create(db, arg(&a, "name")?, arg(&a, "template")?, arg(&a, "path")?).await),
        "project_delete" => ok!(ops::project_delete(db, arg(&a, "id")?).await),
        "project_status_report" => ok!(ops::project_status_report(db, arg(&a, "name")?, arg(&a, "template")?, arg(&a, "fileContents")?).await),

        // --- Discord ---
        "discord_set_token" => ok!(ops::discord_set_token(arg(&a, "token")?).await),
        "discord_disconnect" => ok!(ops::discord_disconnect().await),

        // --- News ---
        "news_fetch_latest" => ok!(ops::news_fetch_latest().await),
        "news_list_items" => ok!(ops::news_list_items(arg(&a, "limit")?).await),
        "news_article_summary" => ok!(ops::news_article_summary(db, arg(&a, "url")?).await),

        // --- Reading list ---
        "reading_list_add" => ok!(ops::reading_list_add(db, arg(&a, "url")?, arg(&a, "title")?).await),
        "reading_list_get" => ok!(ops::reading_list_get(db).await),
        "reading_list_summarize" => ok!(ops::reading_list_summarize(db, arg(&a, "id")?).await),
        "reading_list_delete" => ok!(ops::reading_list_delete(db, arg(&a, "id")?).await),

        // --- Focus sessions ---
        "focus_start" => ok!(ops::focus_start(db, arg(&a, "label")?, arg(&a, "duration_min")?).await),
        "focus_end" => ok!(ops::focus_end(db, arg(&a, "id")?).await),
        "focus_active" => ok!(ops::focus_active(db).await),

        // --- Habits ---
        "habit_create" => ok!(ops::habit_create(db, arg(&a, "name")?, arg(&a, "description")?).await),
        "habit_list" => ok!(ops::habit_list(db).await),
        "habit_log" => ok!(ops::habit_log(db, arg(&a, "habit_id")?, arg(&a, "note")?).await),
        "habit_logged_today" => ok!(ops::habit_logged_today(db, arg(&a, "habit_id")?).await),

        // --- Events & suggestions ---
        "recent_events" => ok!(ops::recent_events(db, arg(&a, "limit")?)),
        "suggestions_list" => ok!(ops::suggestions_list(db, arg(&a, "pendingOnly")?)),
        "suggestion_respond" => ok!(ops::suggestion_respond(db, arg(&a, "id")?, arg(&a, "accept")?).await),

        _ => return Err(UNKNOWN.to_string()),
    };
    out
}
