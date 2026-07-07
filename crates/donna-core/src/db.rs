//! Local SQLite persistence for Donna.
//!
//! Stores app settings, conversations, and chat messages on-device. Secrets (API keys)
//! are NOT stored here — they live in the OS keychain (see `secrets.rs`).

use std::sync::Mutex;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Wraps the SQLite connection behind a mutex so it can live in Tauri managed state.
pub struct Db(pub Mutex<Connection>);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Routine {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub builtin_id: Option<String>,
    pub schedule_type: String,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub day_of_week: Option<i32>,
    pub minutes_before: Option<i32>,
    pub prompt: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub action: Option<String>,
    pub doc_id: Option<i64>,
    pub read: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub template: String,
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Doc {
    pub id: i64,
    pub title: String,
    pub source: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReadingListItem {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub summary: Option<String>,
    pub tags: Option<String>,
    pub read: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FocusSession {
    pub id: i64,
    pub label: String,
    pub duration_min: i32,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Habit {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrustPolicy {
    pub action_kind: String,
    pub mode: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Approval {
    pub id: i64,
    pub conversation_id: i64,
    pub tool: String,
    pub args_json: String,
    pub summary: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Reminder {
    pub id: i64,
    pub text: String,
    pub due_at: String,
    pub fired: bool,
    pub created_at: String,
}

impl Db {
    /// Open (or create) the database at `path` and run migrations.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        migrate(&conn)?;
        Ok(Db(Mutex::new(conn)))
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn create_conversation(&self, title: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO conversations (title, created_at) VALUES (?1, ?2)",
            [title, &now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at FROM conversations ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn rename_conversation(&self, id: i64, title: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET title = ?1 WHERE id = ?2",
            rusqlite::params![title, id],
        )?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM messages WHERE conversation_id = ?1", [id])?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Wipe every conversation and message — used when the user resets Donna's knowledge.
    pub fn delete_all_conversations(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM messages", [])?;
        conn.execute("DELETE FROM conversations", [])?;
        Ok(())
    }

    pub fn add_message(&self, conversation_id: i64, role: &str, content: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![conversation_id, role, content, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_messages(&self, conversation_id: i64) -> Result<Vec<Message>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, created_at
             FROM messages WHERE conversation_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([conversation_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    // --- Routines ------------------------------------------------------------

    pub fn seed_builtin_routines(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        let builtins: &[(&str, &str, &str, i32, i32, Option<i32>, Option<i32>, &str)] = &[
            (
                "morning_briefing",
                "Morning Briefing",
                "daily",
                8,
                0,
                None,
                None,
                "Prepare a concise morning briefing: today's calendar, priorities, and anything Donna should flag.",
            ),
            (
                "relationship_reconnect",
                "Relationship Reconnect",
                "weekly",
                9,
                0,
                Some(0),
                None,
                "Review people in the knowledge base the user has not mentioned recently and suggest warm reconnection nudges.",
            ),
            (
                "meeting_briefing",
                "Meeting Briefing",
                "before_meeting",
                0,
                0,
                None,
                Some(30),
                "Prepare a short briefing for an upcoming meeting: context on attendees, related knowledge, and suggested talking points.",
            ),
            (
                "post_meeting_debrief",
                "Post-Meeting Debrief",
                "after_meeting",
                0,
                5,
                None,
                Some(10),
                "After a meeting ends, pull the Fathom summary and create action items, follow-ups, and a knowledge base update.",
            ),
            (
                "tech_news",
                "Daily Tech News",
                "daily",
                9,
                0,
                None,
                None,
                "Fetch and summarize today's top AI and tech stories. Include key trends, notable releases, and anything relevant to the user's work and interests.",
            ),
            (
                "weekly_review",
                "Weekly Review",
                "weekly",
                20,
                0,
                Some(6),
                None,
                "Generate a comprehensive weekly review: what was accomplished, any open loops or unfinished tasks, upcoming week priorities, and who to reconnect with.",
            ),
            (
                "end_of_day_journal",
                "End-of-Day Journal",
                "daily",
                18,
                0,
                None,
                None,
                "Ask three reflection questions: (1) What went well today? (2) What was challenging? (3) What's the top priority for tomorrow? Synthesize the answers and save insights to the knowledge base.",
            ),
        ];
        for (builtin_id, name, schedule_type, hour, minute, day_of_week, minutes_before, prompt) in
            builtins
        {
            conn.execute(
                "INSERT OR IGNORE INTO routines
                 (name, kind, builtin_id, schedule_type, hour, minute, day_of_week, minutes_before, prompt, enabled)
                 VALUES (?1, 'builtin', ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
                rusqlite::params![
                    name,
                    builtin_id,
                    schedule_type,
                    hour,
                    minute,
                    day_of_week,
                    minutes_before,
                    prompt,
                ],
            )?;
        }
        Ok(())
    }

    fn row_to_routine(row: &rusqlite::Row<'_>) -> rusqlite::Result<Routine> {
        Ok(Routine {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            builtin_id: row.get(3)?,
            schedule_type: row.get(4)?,
            hour: row.get(5)?,
            minute: row.get(6)?,
            day_of_week: row.get(7)?,
            minutes_before: row.get(8)?,
            prompt: row.get(9)?,
            enabled: row.get::<_, i32>(10)? != 0,
            last_run_at: row.get(11)?,
        })
    }

    pub fn list_routines(&self) -> Result<Vec<Routine>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, builtin_id, schedule_type, hour, minute, day_of_week,
                    minutes_before, prompt, enabled, last_run_at
             FROM routines ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_routine)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_routine(&self, id: i64) -> Result<Option<Routine>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, builtin_id, schedule_type, hour, minute, day_of_week,
                    minutes_before, prompt, enabled, last_run_at
             FROM routines WHERE id = ?1",
        )?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_routine(&row)?))
        } else {
            Ok(None)
        }
    }

    pub fn create_routine(
        &self,
        name: &str,
        schedule_type: &str,
        hour: Option<i32>,
        minute: Option<i32>,
        day_of_week: Option<i32>,
        minutes_before: Option<i32>,
        prompt: Option<&str>,
    ) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO routines
             (name, kind, schedule_type, hour, minute, day_of_week, minutes_before, prompt, enabled)
             VALUES (?1, 'custom', ?2, ?3, ?4, ?5, ?6, ?7, 1)",
            rusqlite::params![
                name,
                schedule_type,
                hour,
                minute,
                day_of_week,
                minutes_before,
                prompt,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn toggle_routine(&self, id: i64, enabled: bool) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE routines SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i32, id],
        )?;
        Ok(())
    }

    pub fn delete_routine(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM routine_runs WHERE routine_id = ?1", [id])?;
        conn.execute("DELETE FROM routines WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn mark_routine_run(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE routines SET last_run_at = ?1 WHERE id = ?2",
            rusqlite::params![now_iso(), id],
        )?;
        Ok(())
    }

    pub fn record_routine_dedupe(&self, routine_id: i64, dedupe_key: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO routine_runs (routine_id, dedupe_key, run_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![routine_id, dedupe_key, now_iso()],
        )?;
        Ok(())
    }

    pub fn has_routine_dedupe(&self, routine_id: i64, dedupe_key: &str) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT 1 FROM routine_runs WHERE routine_id = ?1 AND dedupe_key = ?2")?;
        let mut rows = stmt.query(rusqlite::params![routine_id, dedupe_key])?;
        Ok(rows.next()?.is_some())
    }

    // --- Notifications -------------------------------------------------------

    pub fn insert_notification(
        &self,
        title: &str,
        body: &str,
        action: Option<&str>,
        doc_id: Option<i64>,
    ) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO notifications (title, body, action, doc_id, read, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            rusqlite::params![title, body, action, doc_id, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_notifications(&self) -> Result<Vec<Notification>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, body, action, doc_id, read, created_at
             FROM notifications ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Notification {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                action: row.get(3)?,
                doc_id: row.get(4)?,
                read: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn mark_notification_read(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE notifications SET read = 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    // --- Docs ----------------------------------------------------------------

    pub fn create_doc(&self, title: &str, source: &str, content: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO docs (title, source, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![title, source, content, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_doc(&self, id: i64) -> Result<Option<Doc>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, source, content, created_at, updated_at FROM docs WHERE id = ?1",
        )?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Doc {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_docs(&self) -> Result<Vec<Doc>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, source, content, created_at, updated_at
             FROM docs ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Doc {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn update_doc(&self, id: i64, title: &str, content: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE docs SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![title, content, now_iso(), id],
        )?;
        Ok(())
    }

    pub fn delete_doc(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM docs WHERE id = ?1", [id])?;
        Ok(())
    }

    // --- Knowledge embeddings -----------------------------------------------

    pub fn upsert_embedding(&self, node_key: &str, vector: &[f32]) -> Result<()> {
        let json = serde_json::to_string(vector)?;
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO kg_embeddings (node_key, vector) VALUES (?1, ?2)
             ON CONFLICT(node_key) DO UPDATE SET vector = excluded.vector",
            rusqlite::params![node_key, json],
        )?;
        Ok(())
    }

    pub fn list_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT node_key, vector FROM kg_embeddings")?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let json: String = row.get(1)?;
            let vector: Vec<f32> = serde_json::from_str(&json).unwrap_or_default();
            Ok((key, vector))
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_embedding(&self, node_key: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM kg_embeddings WHERE node_key = ?1", [node_key])?;
        Ok(())
    }

    pub fn clear_embeddings(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM kg_embeddings", [])?;
        Ok(())
    }

    // --- Projects ------------------------------------------------------------

    pub fn create_project(&self, name: &str, template: &str, path: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO projects (name, template, path, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, template, path, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, template, path, created_at FROM projects ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                template: row.get(2)?,
                path: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_project(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
        Ok(())
    }

    // --- Reading list ------------------------------------------------------------

    pub fn reading_list_add(&self, url: &str, title: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO reading_list (url, title, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![url, title, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn reading_list_get(&self) -> Result<Vec<ReadingListItem>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, title, summary, tags, read, created_at FROM reading_list ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ReadingListItem {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                summary: row.get(3)?,
                tags: row.get(4)?,
                read: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn reading_list_update_summary(&self, id: i64, summary: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE reading_list SET summary = ?1, read = 1 WHERE id = ?2", rusqlite::params![summary, id])?;
        Ok(())
    }

    pub fn reading_list_delete(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM reading_list WHERE id = ?1", [id])?;
        Ok(())
    }

    // --- Focus sessions ----------------------------------------------------------

    pub fn focus_start(&self, label: &str, duration_min: i32) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO focus_sessions (label, duration_min, started_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![label, duration_min, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn focus_end(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE focus_sessions SET ended_at = ?1 WHERE id = ?2", rusqlite::params![now_iso(), id])?;
        Ok(())
    }

    pub fn focus_active(&self) -> Result<Option<FocusSession>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, label, duration_min, started_at, ended_at FROM focus_sessions WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(FocusSession {
                id: row.get(0)?,
                label: row.get(1)?,
                duration_min: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
            }));
        }
        Ok(None)
    }

    // --- Habits ------------------------------------------------------------------

    pub fn habit_create(&self, name: &str, description: Option<&str>) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO habits (name, description, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, description, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn habit_list(&self) -> Result<Vec<Habit>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, enabled, created_at FROM habits WHERE enabled = 1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Habit {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                enabled: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn habit_log(&self, habit_id: i64, note: Option<&str>) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO habit_logs (habit_id, logged_at, note) VALUES (?1, ?2, ?3)",
            rusqlite::params![habit_id, now_iso(), note],
        )?;
        Ok(())
    }

    pub fn habit_logged_today(&self, habit_id: i64) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM habit_logs WHERE habit_id = ?1 AND logged_at LIKE ?2",
            rusqlite::params![habit_id, format!("{today}%")],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // --- Trust policies ------------------------------------------------------

    pub fn get_trust_policy(&self, action_kind: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT mode FROM trust_policies WHERE action_kind = ?1")?;
        let mut rows = stmt.query([action_kind])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_trust_policy(&self, action_kind: &str, mode: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO trust_policies (action_kind, mode, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(action_kind) DO UPDATE SET mode = excluded.mode, updated_at = excluded.updated_at",
            rusqlite::params![action_kind, mode, now_iso()],
        )?;
        Ok(())
    }

    pub fn list_trust_policies(&self) -> Result<Vec<TrustPolicy>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT action_kind, mode, updated_at FROM trust_policies ORDER BY action_kind ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TrustPolicy {
                action_kind: row.get(0)?,
                mode: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    // --- Approvals -------------------------------------------------------------

    pub fn insert_approval(
        &self,
        conversation_id: i64,
        tool: &str,
        args_json: &str,
        summary: &str,
    ) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO approvals (conversation_id, tool, args_json, summary, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
            rusqlite::params![conversation_id, tool, args_json, summary, now_iso()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn row_to_approval(row: &rusqlite::Row) -> rusqlite::Result<Approval> {
        Ok(Approval {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            tool: row.get(2)?,
            args_json: row.get(3)?,
            summary: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            resolved_at: row.get(7)?,
        })
    }

    pub fn get_approval(&self, id: i64) -> Result<Option<Approval>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, tool, args_json, summary, status, created_at, resolved_at
             FROM approvals WHERE id = ?1",
        )?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_approval(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_approvals(&self, pending_only: bool) -> Result<Vec<Approval>> {
        let conn = self.0.lock().unwrap();
        let sql = if pending_only {
            "SELECT id, conversation_id, tool, args_json, summary, status, created_at, resolved_at
             FROM approvals WHERE status = 'pending' ORDER BY id DESC"
        } else {
            "SELECT id, conversation_id, tool, args_json, summary, status, created_at, resolved_at
             FROM approvals ORDER BY id DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], Self::row_to_approval)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn list_pending_approvals_for_conversation(&self, conversation_id: i64) -> Result<Vec<Approval>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, tool, args_json, summary, status, created_at, resolved_at
             FROM approvals WHERE conversation_id = ?1 AND status = 'pending' ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([conversation_id], Self::row_to_approval)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// The existing pending approval for this exact (conversation, tool, args), if any —
    /// used to dedupe a model that re-issues an identical `Ask` call before the user
    /// has resolved the first request.
    pub fn find_pending_approval(
        &self,
        conversation_id: i64,
        tool: &str,
        args_json: &str,
    ) -> Result<Option<Approval>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, tool, args_json, summary, status, created_at, resolved_at
             FROM approvals
             WHERE conversation_id = ?1 AND tool = ?2 AND args_json = ?3 AND status = 'pending'
             ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(rusqlite::params![conversation_id, tool, args_json])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_approval(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn resolve_approval(&self, id: i64, status: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE approvals SET status = ?1, resolved_at = ?2 WHERE id = ?3 AND status = 'pending'",
            rusqlite::params![status, now_iso(), id],
        )?;
        Ok(())
    }

    /// Newest-first streak of `approved` resolutions for `tool`, stopping at the
    /// first `rejected` (or the first non-approved status). Consumed by the
    /// Phase 4 trust engine to decide when to promote ask -> auto.
    pub fn count_consecutive_approvals(&self, tool: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT status FROM approvals
             WHERE tool = ?1 AND status IN ('approved','rejected')
             ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([tool], |row| row.get::<_, String>(0))?;
        let mut count = 0;
        for status in rows {
            if status? == "approved" {
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }

    // --- Reminders -------------------------------------------------------------

    pub fn insert_reminder(&self, text: &str, due_at: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO reminders (text, due_at, fired, created_at) VALUES (?1, ?2, 0, ?3)",
            rusqlite::params![text, due_at, now_iso()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Contract: both `due_at` (as stored) and `now_iso` must be UTC-normalized
    /// RFC3339 (`+00:00`) — the comparison below is a lexicographic string compare,
    /// which is only correct when both sides share the same fixed-width UTC offset.
    /// Writers normalize: `ops::remember` before insert, and the scheduler sweep
    /// before calling this function.
    pub fn due_unfired_reminders(&self, now_iso: &str) -> Result<Vec<Reminder>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, text, due_at, fired, created_at FROM reminders
             WHERE fired = 0 AND due_at <= ?1 ORDER BY due_at ASC",
        )?;
        let rows = stmt.query_map([now_iso], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                text: row.get(1)?,
                due_at: row.get(2)?,
                fired: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn mark_reminder_fired(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE reminders SET fired = 1 WHERE id = ?1", [id])?;
        Ok(())
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS conversations (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            title      TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            role            TEXT NOT NULL,
            content         TEXT NOT NULL,
            created_at      TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conversation
            ON messages(conversation_id);
        CREATE TABLE IF NOT EXISTS routines (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            kind            TEXT NOT NULL DEFAULT 'custom',
            builtin_id      TEXT,
            schedule_type   TEXT NOT NULL,
            hour            INTEGER,
            minute          INTEGER,
            day_of_week     INTEGER,
            minutes_before  INTEGER,
            prompt          TEXT,
            enabled         INTEGER NOT NULL DEFAULT 1,
            last_run_at     TEXT
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_routines_builtin_id
            ON routines(builtin_id) WHERE builtin_id IS NOT NULL;
        CREATE TABLE IF NOT EXISTS notifications (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            title       TEXT NOT NULL,
            body        TEXT NOT NULL,
            action      TEXT,
            doc_id      INTEGER,
            read        INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS docs (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            title       TEXT NOT NULL,
            source      TEXT NOT NULL,
            content     TEXT NOT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS routine_runs (
            routine_id  INTEGER NOT NULL,
            dedupe_key  TEXT NOT NULL,
            run_at      TEXT NOT NULL,
            PRIMARY KEY (routine_id, dedupe_key)
        );
        CREATE TABLE IF NOT EXISTS kg_embeddings (
            node_key TEXT PRIMARY KEY,
            vector   TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS projects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            template    TEXT NOT NULL,
            path        TEXT NOT NULL,
            created_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS reading_list (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            url         TEXT NOT NULL,
            title       TEXT NOT NULL,
            summary     TEXT,
            tags        TEXT,
            read        INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS focus_sessions (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            label       TEXT NOT NULL,
            duration_min INTEGER NOT NULL DEFAULT 25,
            started_at  TEXT NOT NULL,
            ended_at    TEXT
        );
        CREATE TABLE IF NOT EXISTS habits (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            description TEXT,
            enabled     INTEGER NOT NULL DEFAULT 1,
            created_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS habit_logs (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            habit_id    INTEGER NOT NULL,
            logged_at   TEXT NOT NULL,
            note        TEXT
        );
        CREATE TABLE IF NOT EXISTS trust_policies (
            action_kind TEXT PRIMARY KEY,
            mode        TEXT NOT NULL CHECK (mode IN ('ask','auto')),
            updated_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS approvals (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            tool            TEXT NOT NULL,
            args_json       TEXT NOT NULL,
            summary         TEXT NOT NULL,
            status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','expired')),
            created_at      TEXT NOT NULL,
            resolved_at     TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_approvals_status ON approvals(status);
        CREATE TABLE IF NOT EXISTS reminders (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            text        TEXT NOT NULL,
            due_at      TEXT NOT NULL,
            fired       INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        let dir = std::env::temp_dir().join(format!(
            "donna-db-test-{}-{}",
            std::process::id(),
            now_iso().replace([':', '.'], "-")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Db::open(&dir.join("t.sqlite")).unwrap()
    }

    #[test]
    fn trust_policy_default_absent_then_upsert() {
        let db = test_db();
        assert_eq!(db.get_trust_policy("slack_send_message").unwrap(), None);
        db.set_trust_policy("slack_send_message", "auto").unwrap();
        assert_eq!(db.get_trust_policy("slack_send_message").unwrap(), Some("auto".into()));
        db.set_trust_policy("slack_send_message", "ask").unwrap(); // upsert
        assert_eq!(db.list_trust_policies().unwrap().len(), 1);
    }

    #[test]
    fn approval_lifecycle() {
        let db = test_db();
        let id = db.insert_approval(1, "whatsapp_send_message", r#"{"to":"+1","text":"hi"}"#, "Send WhatsApp to +1").unwrap();
        assert_eq!(db.get_approval(id).unwrap().unwrap().status, "pending");
        assert_eq!(db.list_pending_approvals_for_conversation(1).unwrap().len(), 1);
        db.resolve_approval(id, "approved").unwrap();
        let a = db.get_approval(id).unwrap().unwrap();
        assert_eq!(a.status, "approved");
        assert!(a.resolved_at.is_some());
        assert!(db.list_pending_approvals_for_conversation(1).unwrap().is_empty());
        // resolving again must not clobber
        db.resolve_approval(id, "rejected").unwrap();
        assert_eq!(db.get_approval(id).unwrap().unwrap().status, "approved");
    }

    #[test]
    fn reminders_due_and_fired() {
        let db = test_db();
        db.insert_reminder("stretch", "2026-01-01T00:00:00Z").unwrap();
        let due = db.due_unfired_reminders("2026-01-02T00:00:00Z").unwrap();
        assert_eq!(due.len(), 1);
        db.mark_reminder_fired(due[0].id).unwrap();
        assert!(db.due_unfired_reminders("2026-01-02T00:00:00Z").unwrap().is_empty());
        // not yet due
        db.insert_reminder("later", "2027-01-01T00:00:00Z").unwrap();
        assert!(db.due_unfired_reminders("2026-01-02T00:00:00Z").unwrap().is_empty());
    }

    #[tokio::test]
    async fn remember_normalizes_offsets_to_utc() {
        let db = test_db();
        // ops::remember with a +07:00 due_at that is ALREADY PAST in UTC (= 02:00Z)
        crate::ops::remember(&db, "tea".into(), "2026-01-01T09:00:00+07:00".into())
            .await
            .unwrap();
        let due = db.due_unfired_reminders("2026-01-01T03:00:00+00:00").unwrap();
        assert_eq!(due.len(), 1, "overdue +07:00 reminder must be found by a UTC now");
        assert_eq!(due[0].due_at, "2026-01-01T02:00:00+00:00");

        // and one NOT yet due (= 16:00Z)
        crate::ops::remember(&db, "later".into(), "2026-01-01T23:00:00+07:00".into())
            .await
            .unwrap();
        assert_eq!(db.due_unfired_reminders("2026-01-01T03:00:00+00:00").unwrap().len(), 1);
    }

    #[test]
    fn count_consecutive_approvals_stops_at_first_rejected() {
        let db = test_db();
        let a = db.insert_approval(1, "slack_send_message", "{}", "s").unwrap();
        db.resolve_approval(a, "rejected").unwrap();
        let b = db.insert_approval(1, "slack_send_message", "{}", "s").unwrap();
        db.resolve_approval(b, "approved").unwrap();
        let c = db.insert_approval(1, "slack_send_message", "{}", "s").unwrap();
        db.resolve_approval(c, "approved").unwrap();
        let d = db.insert_approval(1, "slack_send_message", "{}", "s").unwrap();
        db.resolve_approval(d, "approved").unwrap();
        // newest-first: d, c, b approved, then a rejected -> streak of 3
        assert_eq!(db.count_consecutive_approvals("slack_send_message").unwrap(), 3);
    }
}
