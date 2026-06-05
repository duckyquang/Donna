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

/// A node in Donna's knowledge graph (one piece of knowledge about the user).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KgNode {
    pub id: String,
    pub label: String,
    pub group: String,
    pub note: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KgEdge {
    pub source: String,
    pub target: String,
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

    // --- Knowledge graph ---------------------------------------------------

    /// Insert or update a knowledge-graph node (keyed by stable id).
    pub fn upsert_node(&self, id: &str, label: &str, group: &str, note: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO kg_nodes (id, label, \"group\", note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                label = excluded.label,
                \"group\" = excluded.\"group\",
                note = excluded.note,
                updated_at = excluded.updated_at",
            rusqlite::params![id, label, group, note, now],
        )?;
        Ok(())
    }

    /// Insert an edge if it does not already exist.
    pub fn add_edge(&self, source: &str, target: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO kg_edges (source, target) VALUES (?1, ?2)",
            [source, target],
        )?;
        Ok(())
    }

    pub fn list_nodes(&self) -> Result<Vec<KgNode>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, label, \"group\", note, updated_at FROM kg_nodes
             ORDER BY \"group\", label",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(KgNode {
                id: row.get(0)?,
                label: row.get(1)?,
                group: row.get(2)?,
                note: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn list_edges(&self) -> Result<Vec<KgEdge>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT source, target FROM kg_edges")?;
        let rows = stmt.query_map([], |row| {
            Ok(KgEdge {
                source: row.get(0)?,
                target: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
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
        CREATE TABLE IF NOT EXISTS kg_nodes (
            id         TEXT PRIMARY KEY,
            label      TEXT NOT NULL,
            \"group\"    TEXT NOT NULL,
            note       TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS kg_edges (
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            PRIMARY KEY (source, target)
        );",
    )?;
    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}
