//! Local document storage — SQLite `docs` table is the source of truth.

use crate::db::{Db, Doc};
use crate::error::Result;

pub fn create(db: &Db, title: &str, source: &str, content: &str) -> Result<i64> {
    db.create_doc(title, source, content)
}

pub fn get(db: &Db, id: i64) -> Result<Option<Doc>> {
    db.get_doc(id)
}

pub fn list(db: &Db) -> Result<Vec<Doc>> {
    db.list_docs()
}

pub fn update(db: &Db, id: i64, title: &str, content: &str) -> Result<()> {
    db.update_doc(id, title, content)
}

pub fn delete(db: &Db, id: i64) -> Result<()> {
    db.delete_doc(id)
}
