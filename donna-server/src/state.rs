use std::sync::Arc;
use donna_core::db::Db;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub token: String,
    pub events: tokio::sync::broadcast::Sender<ServerEvent>,
    /// Meta webhook GET-handshake token. `None` → the verify endpoint always 403s.
    pub wa_verify_token: Option<String>,
    /// Meta app secret for HMAC-SHA256 signature verification of inbound POSTs.
    /// `None` → inbound webhooks are ignored (200, no processing): secure by default.
    pub wa_app_secret: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Notification { title: String, body: String },
}

/// Scheduler notifications are broadcast to every connected WS client as
/// `{"type":"notification",...}` frames.
impl donna_core::scheduler::Notifier for AppState {
    fn notify(&self, title: &str, body: &str) {
        let _ = self.events.send(ServerEvent::Notification {
            title: title.into(),
            body: body.into(),
        });
    }
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
    // Belt-and-braces: keep server tests off the real OS keychain too. `init` is a
    // set-once no-op if a store was already installed in this process.
    donna_core::secrets::init(Box::new(donna_core::secrets::FileStore::new(dir.join("secrets.json"))));
    AppState {
        db: Arc::new(Db::open(&dir.join("t.sqlite")).unwrap()),
        token: "test-token".into(),
        events: tokio::sync::broadcast::channel(64).0,
        wa_verify_token: Some("test-verify".into()),
        wa_app_secret: Some("test-secret".into()),
    }
}

/// Per-process counter so multiple `test_state()` calls within the same test binary
/// (same pid) still get distinct directories.
fn uniq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
