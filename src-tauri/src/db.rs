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
        );",
    )?;
    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}
