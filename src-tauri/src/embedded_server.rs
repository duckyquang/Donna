//! Embedded donna-server sidecar: token + port bootstrap and process lifecycle.
//!
//! The packaged app ships donna-server as a sidecar and spawns it on launch so end
//! users never touch Docker or a terminal. The UI adopts this config only when no
//! server token is stored (see src/lib/server.ts), so remote-server users are unaffected.
//!
//! Restart policy: both a post-healthy crash and a startup health-check timeout count
//! against `MAX_RESTARTS` and retry with a 2s delay. Only a spawn error (e.g. the
//! sidecar binary is missing — dev builds without `npm run sidecar`) is immediately
//! terminal, since retrying can't fix a missing binary.

use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

const PREFERRED_PORT: u16 = 8377;
const MAX_RESTARTS: u32 = 5;

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum EmbeddedStatus {
    Starting,
    Ready { url: String, token: String },
    Failed { error: String },
}

pub struct EmbeddedState {
    pub status: Mutex<EmbeddedStatus>,
    pub child: Mutex<Option<CommandChild>>,
    pub restarts: Mutex<u32>,
    pub quitting: AtomicBool,
}

impl Default for EmbeddedState {
    fn default() -> Self {
        Self {
            status: Mutex::new(EmbeddedStatus::Starting),
            child: Mutex::new(None),
            restarts: Mutex::new(0),
            quitting: AtomicBool::new(false),
        }
    }
}

/// Load the persistent sidecar bearer token, generating one on first run.
pub fn load_or_create_token(path: &Path) -> std::io::Result<String> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        let existing = existing.trim().to_string();
        if !existing.is_empty() {
            return Ok(existing);
        }
    }
    let token: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| char::from(rng.sample(rand::distributions::Alphanumeric)))
            .collect()
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, &token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(token)
}

/// `preferred` when free, else an OS-assigned ephemeral port.
pub fn pick_port(preferred: u16) -> u16 {
    if let Ok(l) = std::net::TcpListener::bind(("127.0.0.1", preferred)) {
        drop(l);
        return preferred;
    }
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .map(|l| l.local_addr().unwrap().port())
        .unwrap_or(preferred)
}

/// A `spawn_once` failure, distinguishing a retryable startup timeout from a terminal
/// spawn error (e.g. the sidecar binary is missing in dev).
enum SpawnFailure {
    /// Health check never went green within the timeout; child was killed. Retryable.
    HealthTimeout,
    /// Couldn't even launch the process. Not retryable.
    SpawnError(String),
}

/// Spawn the sidecar and keep it alive (bounded restarts). Runs for the app's lifetime.
pub fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let mut rx = match spawn_once(&app).await {
                Ok(rx) => {
                    // Reached Ready — forgive past restarts so a long healthy run
                    // doesn't inherit a stale counter from earlier flakiness.
                    let state = app.state::<EmbeddedState>();
                    *state.restarts.lock().unwrap() = 0;
                    rx
                }
                Err(SpawnFailure::SpawnError(e)) => {
                    let state = app.state::<EmbeddedState>();
                    *state.status.lock().unwrap() = EmbeddedStatus::Failed { error: e };
                    return;
                }
                Err(SpawnFailure::HealthTimeout) => {
                    if !count_restart_or_fail(&app, "embedded server did not become healthy") {
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            // Wait until the child dies.
            while let Some(ev) = rx.recv().await {
                if matches!(ev, CommandEvent::Terminated(_)) {
                    break;
                }
            }
            let state = app.state::<EmbeddedState>();
            // Mark as Starting immediately when child dies to avoid stale-Ready window.
            *state.status.lock().unwrap() = EmbeddedStatus::Starting;
            if state.quitting.load(Ordering::SeqCst) {
                return;
            }
            if !count_restart_or_fail(&app, "embedded server keeps crashing") {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

/// Bump the restart counter; if it's past `MAX_RESTARTS`, mark Failed and return false
/// (caller should stop looping). Otherwise returns true (caller should retry).
fn count_restart_or_fail(app: &tauri::AppHandle, fail_message: &str) -> bool {
    let state = app.state::<EmbeddedState>();
    let attempts = {
        let mut n = state.restarts.lock().unwrap();
        *n += 1;
        *n
    };
    if attempts > MAX_RESTARTS {
        *state.status.lock().unwrap() = EmbeddedStatus::Failed {
            error: fail_message.into(),
        };
        return false;
    }
    *state.status.lock().unwrap() = EmbeddedStatus::Starting;
    true
}

async fn spawn_once(
    app: &tauri::AppHandle,
) -> Result<tauri::async_runtime::Receiver<CommandEvent>, SpawnFailure> {
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|e| SpawnFailure::SpawnError(e.to_string()))?;
    let token = load_or_create_token(&data_root.join("server-token"))
        .map_err(|e| SpawnFailure::SpawnError(e.to_string()))?;
    let port = pick_port(PREFERRED_PORT);
    let (rx, child) = app
        .shell()
        .sidecar("donna-server")
        .map_err(|e| SpawnFailure::SpawnError(e.to_string()))?
        .env("DONNA_TOKEN", &token)
        .env("DONNA_PORT", port.to_string())
        .env(
            "DONNA_DATA_DIR",
            data_root.join("server").to_string_lossy().to_string(),
        )
        .env("DONNA_BIND", "127.0.0.1")
        .spawn()
        .map_err(|e| SpawnFailure::SpawnError(e.to_string()))?;
    let state = app.state::<EmbeddedState>();
    *state.child.lock().unwrap() = Some(child);

    let url = format!("http://127.0.0.1:{port}");
    for _ in 0..60 {
        let healthy = reqwest::get(format!("{url}/health"))
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if healthy {
            *state.status.lock().unwrap() = EmbeddedStatus::Ready {
                url: url.clone(),
                token: token.clone(),
            };
            return Ok(rx);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    kill_child(app);
    Err(SpawnFailure::HealthTimeout)
}

/// Take and kill the child process without touching `quitting`. Used by the
/// health-timeout path, where the sidecar should be restarted, not shut down for good.
fn kill_child(app: &tauri::AppHandle) {
    let state = app.state::<EmbeddedState>();
    let child = state.child.lock().unwrap().take();
    if let Some(child) = child {
        let _ = child.kill();
    }
}

/// Kill the sidecar for good (app quit). Safe to call twice.
pub fn kill(app: &tauri::AppHandle) {
    let state = app.state::<EmbeddedState>();
    state.quitting.store(true, Ordering::SeqCst);
    kill_child(app);
}

#[tauri::command]
pub fn embedded_server_status(state: tauri::State<EmbeddedState>) -> EmbeddedStatus {
    state.status.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_created_then_reused() {
        let dir = std::env::temp_dir().join(format!("donna-test-{}", std::process::id()));
        let path = dir.join("server-token");
        let _ = std::fs::remove_dir_all(&dir);
        let first = load_or_create_token(&path).unwrap();
        assert_eq!(first.len(), 32);
        let second = load_or_create_token(&path).unwrap();
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pick_port_prefers_free_port_and_dodges_taken_one() {
        let holder = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let taken = holder.local_addr().unwrap().port();
        assert_ne!(pick_port(taken), taken);
        drop(holder);
        assert_eq!(pick_port(taken), taken);
    }
}
