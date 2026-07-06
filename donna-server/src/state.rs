use std::sync::Arc;
use donna_core::db::Db;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub token: String,
    pub events: tokio::sync::broadcast::Sender<ServerEvent>,
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Notification { title: String, body: String },
}

/// Builds an `AppState` backed by a unique temp-dir SQLite DB, so parallel tests never
/// collide on the same file.
pub fn test_state() -> AppState {
    let dir = std::env::temp_dir().join(format!(
        "donna-server-test-{}-{}",
        std::process::id(),
        uniq()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    AppState {
        db: Arc::new(Db::open(&dir.join("t.sqlite")).unwrap()),
        token: "test-token".into(),
        events: tokio::sync::broadcast::channel(64).0,
    }
}

/// Per-process counter so multiple `test_state()` calls within the same test binary
/// (same pid) still get distinct directories.
fn uniq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
