# Packaged Desktop App + Landing Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Donna as a downloadable, zero-setup desktop app (embedded donna-server sidecar, hands-free Ollama onboarding, auto-updates) plus a static landing page on GitHub Pages.

**Architecture:** The Tauri shell bundles `donna-server` as a sidecar binary and spawns it on launch with an auto-generated token; the React UI adopts that config only when no server is configured, so remote-server power users are untouched. A managed Ollama runtime (downloaded on demand by the shell) makes the local-model path terminal-free. CI builds the sidecar per target and publishes updater artifacts; the landing page is hand-written static HTML/CSS/JS deployed by a Pages workflow.

**Tech Stack:** Tauri 2 (plugins: shell, updater, process, tray-icon), Rust (reqwest, tokio), React + TS + Tailwind, GitHub Actions, GitHub Pages.

**Spec:** `docs/superpowers/specs/2026-07-08-packaged-app-and-landing-page-design.md`

## Global Constraints

- Rust floor 1.86, Node 20 (existing).
- Server port: prefer **8377**, fall back to an ephemeral port. Ollama: **127.0.0.1:11434**.
- Default local model: **`qwen2.5:3b`** (constant `DEFAULT_LOCAL_MODEL` in `src/components/LocalBrainSetup.tsx`).
- Ollama release pinned by `OLLAMA_VERSION` const in `src-tauri/src/ollama.rs`.
- App version bumps to **0.2.0** in `src-tauri/tauri.conf.json` **and** `package.json` (Task 7).
- Landing page: **no emojis anywhere**; icons are inline SVG or brand SVG files in `site/assets/`; **relative asset paths only** (page serves under `/Donna/`); typographic quotes (’ “ ”) and en/em dashes in copy.
- Builds stay **unsigned** (no Apple/Windows certs). The updater uses its own free keypair.
- Workflow: commit **and push** after each task (repo convention: no PRs, push each step).
- Frontend has no JS test framework — do not add one; verify UI tasks with `npm run build` (tsc) + the listed manual checks. Rust helpers get real unit tests.

---

# Phase 1 — Embedded server sidecar

### Task 1: Sidecar build script + bundle config

**Files:**
- Create: `scripts/build-sidecar.sh`
- Modify: `src-tauri/tauri.conf.json` (add `bundle.externalBin`)
- Modify: `package.json` (add `sidecar` script)
- Modify: `.gitignore` (ignore `src-tauri/binaries/`)
- Modify: `docs/BUILD.md` (dev prerequisite note)

**Interfaces:**
- Produces: `src-tauri/binaries/donna-server-<target-triple>[.exe]` — the sidecar binary Tauri bundles; later tasks spawn it via `.shell().sidecar("donna-server")`.

- [ ] **Step 1: Write the build script**

Create `scripts/build-sidecar.sh`:

```bash
#!/usr/bin/env bash
# Build donna-server and place it where Tauri's externalBin expects:
#   src-tauri/binaries/donna-server-<target-triple>[.exe]
# Usage: build-sidecar.sh [target-triple]   (defaults to the host triple)
set -euo pipefail
cd "$(dirname "$0")/.."

HOST="$(rustc -vV | sed -n 's/^host: //p')"
TARGET="${1:-$HOST}"

if [ "${1:-}" != "" ]; then
  cargo build -p donna-server --release --target "$TARGET"
  SRC="target/$TARGET/release/donna-server"
else
  cargo build -p donna-server --release
  SRC="target/release/donna-server"
fi

EXT=""
case "$TARGET" in *windows*) EXT=".exe" ;; esac

mkdir -p src-tauri/binaries
cp "$SRC$EXT" "src-tauri/binaries/donna-server-$TARGET$EXT"
echo "sidecar ready: src-tauri/binaries/donna-server-$TARGET$EXT"
```

Then: `chmod +x scripts/build-sidecar.sh`

- [ ] **Step 2: Wire config**

In `src-tauri/tauri.conf.json`, add `externalBin` to the existing `bundle` object:

```json
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/donna-server"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
```

In `package.json` `scripts`, add:

```json
    "sidecar": "bash scripts/build-sidecar.sh",
```

Append to `.gitignore`:

```
src-tauri/binaries/
```

- [ ] **Step 3: Run it and verify**

Run: `npm run sidecar`
Expected: prints `sidecar ready: src-tauri/binaries/donna-server-<host-triple>` and the file exists (`ls src-tauri/binaries/`).

- [ ] **Step 4: Document the dev prerequisite**

In `docs/BUILD.md`, under the `## Development` heading, change the code block to:

```bash
npm install
npm run sidecar   # build the donna-server sidecar once (rerun after server changes)
npm run tauri:dev
```

and add below it: `Tauri refuses to start if the sidecar binary is missing — rerun \`npm run sidecar\` after pulling server changes.`

- [ ] **Step 5: Commit**

```bash
git add scripts/build-sidecar.sh src-tauri/tauri.conf.json package.json .gitignore docs/BUILD.md
git commit -m "Bundle donna-server as a Tauri sidecar (build script + externalBin)"
git push
```

---

### Task 2: Shell module — spawn, watch, and expose the embedded server

**Files:**
- Create: `src-tauri/src/embedded_server.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml` (add `tauri-plugin-shell`)

**Interfaces:**
- Consumes: the sidecar binary from Task 1 (`.shell().sidecar("donna-server")`).
- Produces: Tauri command **`embedded_server_status`** returning JSON `{"status":"starting"} | {"status":"ready","url":string,"token":string} | {"status":"failed","error":string}`; `embedded_server::start(AppHandle)` and `embedded_server::kill(&AppHandle)` for lib.rs.

- [ ] **Step 1: Add the shell plugin dependency**

In `src-tauri/Cargo.toml` `[dependencies]`, add:

```toml
tauri-plugin-shell = "2"
```

- [ ] **Step 2: Write failing tests for the pure helpers**

Create `src-tauri/src/embedded_server.rs` containing only the test module for now:

```rust
//! Embedded donna-server sidecar: token + port bootstrap and process lifecycle.
//!
//! The packaged app ships donna-server as a sidecar and spawns it on launch so end
//! users never touch Docker or a terminal. The UI adopts this config only when no
//! server token is stored (see src/lib/server.ts), so remote-server users are unaffected.

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
        // Grab an ephemeral port, keep it bound, and ask pick_port for that exact port.
        let holder = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let taken = holder.local_addr().unwrap().port();
        assert_ne!(pick_port(taken), taken);
        drop(holder);
        // Now free: should return the preferred port itself.
        assert_eq!(pick_port(taken), taken);
    }
}
```

Add `mod embedded_server;` to `src-tauri/src/lib.rs` (below `mod quick_chat;`).

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p donna --lib embedded_server`
Expected: FAIL — `load_or_create_token` / `pick_port` not found.

- [ ] **Step 4: Implement the module**

Replace the file contents (keeping the doc comment and test module) so the full file is:

```rust
//! Embedded donna-server sidecar: token + port bootstrap and process lifecycle.
//!
//! The packaged app ships donna-server as a sidecar and spawns it on launch so end
//! users never touch Docker or a terminal. The UI adopts this config only when no
//! server token is stored (see src/lib/server.ts), so remote-server users are unaffected.

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

/// Spawn the sidecar and keep it alive (bounded restarts). Runs for the app's lifetime.
pub fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let mut rx = match spawn_once(&app).await {
                Ok(rx) => rx,
                Err(e) => {
                    let state = app.state::<EmbeddedState>();
                    *state.status.lock().unwrap() = EmbeddedStatus::Failed { error: e };
                    return;
                }
            };
            // Wait until the child dies.
            while let Some(ev) = rx.recv().await {
                if matches!(ev, CommandEvent::Terminated(_)) {
                    break;
                }
            }
            let state = app.state::<EmbeddedState>();
            if state.quitting.load(Ordering::SeqCst) {
                return;
            }
            let attempts = {
                let mut n = state.restarts.lock().unwrap();
                *n += 1;
                *n
            };
            if attempts > MAX_RESTARTS {
                *state.status.lock().unwrap() = EmbeddedStatus::Failed {
                    error: "embedded server keeps crashing".into(),
                };
                return;
            }
            *state.status.lock().unwrap() = EmbeddedStatus::Starting;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

async fn spawn_once(
    app: &tauri::AppHandle,
) -> Result<tauri::async_runtime::Receiver<CommandEvent>, String> {
    let data_root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let token =
        load_or_create_token(&data_root.join("server-token")).map_err(|e| e.to_string())?;
    let port = pick_port(PREFERRED_PORT);
    let (rx, child) = app
        .shell()
        .sidecar("donna-server")
        .map_err(|e| e.to_string())?
        .env("DONNA_TOKEN", &token)
        .env("DONNA_PORT", port.to_string())
        .env(
            "DONNA_DATA_DIR",
            data_root.join("server").to_string_lossy().to_string(),
        )
        .spawn()
        .map_err(|e| e.to_string())?;
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
    kill(app);
    Err("embedded donna-server did not report healthy within 15s".into())
}

/// Kill the sidecar (app quit, or a failed health wait). Safe to call twice.
pub fn kill(app: &tauri::AppHandle) {
    let state = app.state::<EmbeddedState>();
    state.quitting.store(true, Ordering::SeqCst);
    if let Some(child) = state.child.lock().unwrap().take() {
        let _ = child.kill();
    }
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
```

Note: `kill()` sets `quitting=true` for both call sites on purpose — on app quit it stops the watcher from respawning a child that would outlive the app, and on a failed health wait it stops a never-healthy server from restart-looping.

- [ ] **Step 5: Wire lib.rs**

In `src-tauri/src/lib.rs`:

1. Below `mod quick_chat;` add: `mod embedded_server;` (if not already added in Step 2).
2. In the builder chain, register the plugin after the existing plugins:

```rust
        .plugin(tauri_plugin_shell::init())
```

3. Inside `setup`, after `app.manage(crate::quick_chat::QuickChatState::default());` add:

```rust
            // Embedded brain: spawn the bundled donna-server so end users need zero setup.
            app.manage(crate::embedded_server::EmbeddedState::default());
            crate::embedded_server::start(app.handle().clone());
```

4. Add `commands` entry `embedded_server::embedded_server_status` — the handler macro takes paths, so:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::quick_chat_context,
            commands::google_set_client,
            commands::google_connect,
            commands::export_google_secrets,
            commands::export_server_bundle,
            commands::project_open_in_editor,
            commands::project_list_files,
            commands::project_read_file,
            commands::project_write_file,
            commands::project_status_report,
            embedded_server::embedded_server_status,
        ])
```

5. Replace the final `.run(...)` with a build + run callback that kills the sidecar on exit:

```rust
        .build(tauri::generate_context!())
        .expect("error while building Donna")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                crate::embedded_server::kill(app);
            }
        });
```

- [ ] **Step 6: Run tests + check**

Run: `cargo test -p donna --lib embedded_server`
Expected: PASS (2 tests).
Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add src-tauri
git commit -m "Spawn the bundled donna-server sidecar with auto token/port, expose status command"
git push
```

---

### Task 3: Frontend bootstrap + Settings “Use built-in server”

**Files:**
- Modify: `src/lib/server.ts`
- Modify: `src/main.tsx`
- Modify: `src/routes/Settings.tsx`

**Interfaces:**
- Consumes: Tauri command `embedded_server_status` (Task 2) → `{status:"starting"|"ready"|"failed", url?, token?, error?}`.
- Produces: `bootstrapServerConfig(): Promise<void>` and `export type EmbeddedStatus` from `src/lib/server.ts`.

- [ ] **Step 1: Add bootstrap to server.ts**

At the top of `src/lib/server.ts` add imports, and append the new exports:

```ts
import { invoke } from "@tauri-apps/api/core";
import { isDesktopApp } from "./tauri";
```

```ts
export type EmbeddedStatus =
  | { status: "starting" }
  | { status: "ready"; url: string; token: string }
  | { status: "failed"; error: string };

/**
 * Adopt the embedded sidecar's {url, token} on first run. No-op when a token is
 * already stored (remote-server installs) or outside the desktop app. Resolves when
 * the sidecar is ready, has failed (dev without a sidecar), or ~20s pass.
 */
export async function bootstrapServerConfig(): Promise<void> {
  if (!isDesktopApp() || localStorage.getItem("donna.serverToken")) return;
  for (let i = 0; i < 66; i++) {
    const s = await invoke<EmbeddedStatus>("embedded_server_status").catch(
      () => ({ status: "failed", error: "no shell" }) as EmbeddedStatus,
    );
    if (s.status === "ready") {
      setServerConfig({ url: s.url, token: s.token });
      return;
    }
    if (s.status === "failed") return;
    await new Promise((r) => setTimeout(r, 300));
  }
}
```

- [ ] **Step 2: Gate first render on it in main.tsx**

Replace `src/main.tsx` with:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import { ConfigProvider } from "./lib/useConfig";
import { bootstrapServerConfig } from "./lib/server";
import "./styles/global.css";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

// Splash while the embedded server boots (first run only; instant otherwise).
root.render(
  <div className="flex h-full w-full items-center justify-center bg-donna-bg text-sm text-gray-400">
    Starting Donna…
  </div>
);

bootstrapServerConfig().finally(() => {
  root.render(
    <React.StrictMode>
      <ConfigProvider>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </ConfigProvider>
    </React.StrictMode>
  );
});
```

- [ ] **Step 3: Settings reset button**

In `src/routes/Settings.tsx`:

1. Extend the server import line:

```ts
import { serverConfig, setServerConfig, serverReachable, type EmbeddedStatus } from "../lib/server";
import { invoke } from "@tauri-apps/api/core";
```

2. Next to the existing `testConnection` function add:

```ts
  const useBuiltIn = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const s = await invoke<EmbeddedStatus>("embedded_server_status").catch(() => null);
      if (s && s.status === "ready") {
        setServerUrl(s.url);
        setServerToken(s.token);
        setServerConfig({ url: s.url, token: s.token });
        setTestResult((await serverReachable()) ? "ok" : "fail");
      } else {
        setTestResult("fail");
      }
    } finally {
      setTesting(false);
    }
  };
```

3. In the Server card's button row (the `div` with `Test connection`), add after the Test button:

```tsx
            <Button variant="ghost" onClick={useBuiltIn} disabled={testing}>
              Use built-in server
            </Button>
```

4. Update the Server card subtitle copy to explain the default:

```tsx
            <p className="text-xs text-gray-500">
              Donna ships with a built-in brain that runs automatically. Point her at a
              remote donna-server only if you self-host one.
            </p>
```

- [ ] **Step 4: Verify**

Run: `npm run build`
Expected: tsc + vite succeed.

Manual smoke (requires Task 1's sidecar built): clear config and launch —

```bash
npm run sidecar && npm run tauri:dev
```

In the app's devtools console run `localStorage.clear()` then reload. Expected: brief “Starting Donna…”, then onboarding appears and `localStorage.getItem("donna.serverToken")` is a 32-char string; Settings → Server shows `http://127.0.0.1:8377`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/server.ts src/main.tsx src/routes/Settings.tsx
git commit -m "Auto-adopt the embedded server config on first run; Settings gains Use built-in server"
git push
```

---

### Task 4: Tray icon with Open/Quit

**Files:**
- Modify: `src-tauri/Cargo.toml` (tauri features)
- Modify: `src-tauri/src/lib.rs` (tray in setup)

**Interfaces:**
- Consumes: `embedded_server::kill` via the existing `RunEvent::Exit` handler (Task 2) — Quit just calls `app.exit(0)`.

- [ ] **Step 1: Enable tray features**

In `src-tauri/Cargo.toml` change the tauri dependency line to:

```toml
tauri = { version = "2", features = ["tray-icon", "image-png"] }
```

- [ ] **Step 2: Build the tray in setup**

In `src-tauri/src/lib.rs` `setup`, after the global-shortcut block, add:

```rust
            // Tray: closing the window already hides it (see on_window_event); the tray
            // is how users reopen Donna or actually quit. The server keeps running
            // while hidden, so routines still fire.
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;
                let open = MenuItem::with_id(app, "open", "Open Donna", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit Donna", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&open, &quit])?;
                TrayIconBuilder::with_id("donna-tray")
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, e| match e.id.as_ref() {
                        "open" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .build(app)?;
            }
```

- [ ] **Step 3: Verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml` — clean.
Manual: `npm run tauri:dev` → tray icon appears; close window (app stays alive, tray “Open Donna” restores it); tray “Quit Donna” exits and `ps aux | grep donna-server` shows the sidecar is gone.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "Add tray with Open/Quit so Donna keeps working after the window closes"
git push
```

---

# Phase 2 — Hands-free Ollama onboarding

### Task 5: Shell module — managed Ollama runtime

**Files:**
- Create: `src-tauri/src/ollama.rs`
- Modify: `src-tauri/src/lib.rs` (mod, state, commands, exit kill)
- Modify: `src-tauri/src/commands.rs` (add `open_url`)

**Interfaces:**
- Produces Tauri commands:
  - `ollama_status() -> {running: bool, managedInstalled: bool, models: string[]}`
  - `ollama_install() -> ()` — downloads + extracts the pinned runtime; emits progress
  - `ollama_start() -> ()` — spawns managed `ollama serve`, waits until reachable
  - `ollama_pull(model: string) -> ()` — streams a model pull; emits progress
  - `open_url(url: string) -> ()` — opens an https URL in the default browser
- Produces Tauri event **`ollama:progress`** with payload `{phase: "download"|"pull", detail: string, completed: number, total: number}`.

- [ ] **Step 1: Pin the Ollama release**

Run:

```bash
gh api repos/ollama/ollama/releases/latest --jq '.tag_name'
gh api repos/ollama/ollama/releases/latest --jq '.assets[].name' | grep -E 'darwin|windows-amd64\.zip$|linux-(amd64|arm64)\.tgz$'
```

Note the tag (e.g. `v0.9.6`) — it becomes `OLLAMA_VERSION` in Step 3. Confirm assets named `ollama-darwin.tgz`, `ollama-windows-amd64.zip`, `ollama-linux-amd64.tgz`, `ollama-linux-arm64.tgz` exist; if any name differs, use the listed name in the `asset_name` mapping in Step 3.

- [ ] **Step 2: Write the failing test**

Create `src-tauri/src/ollama.rs` with just:

```rust
//! Managed Ollama runtime: Donna downloads the portable runtime once into her app-data
//! dir, runs `ollama serve` herself, and streams model pulls — no terminal, no admin
//! rights. If the user already runs their own Ollama on 11434, we use it untouched.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_cover_supported_platforms() {
        assert_eq!(asset_name("macos", "aarch64"), Some("ollama-darwin.tgz"));
        assert_eq!(asset_name("macos", "x86_64"), Some("ollama-darwin.tgz"));
        assert_eq!(asset_name("windows", "x86_64"), Some("ollama-windows-amd64.zip"));
        assert_eq!(asset_name("linux", "x86_64"), Some("ollama-linux-amd64.tgz"));
        assert_eq!(asset_name("linux", "aarch64"), Some("ollama-linux-arm64.tgz"));
        assert_eq!(asset_name("freebsd", "x86_64"), None);
    }
}
```

Add `mod ollama;` to `src-tauri/src/lib.rs`.

Run: `cargo test -p donna --lib ollama`
Expected: FAIL — `asset_name` not found.

- [ ] **Step 3: Implement the module**

Full contents of `src-tauri/src/ollama.rs` (keep the test module at the bottom). Use the tag from Step 1 for `OLLAMA_VERSION`:

```rust
//! Managed Ollama runtime: Donna downloads the portable runtime once into her app-data
//! dir, runs `ollama serve` herself, and streams model pulls — no terminal, no admin
//! rights. If the user already runs their own Ollama on 11434, we use it untouched.

use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Pinned Ollama release. Bump deliberately; asset names are release-specific.
pub const OLLAMA_VERSION: &str = "v0.9.6"; // ← value from Task 5 Step 1
const OLLAMA_URL: &str = "http://127.0.0.1:11434";

#[derive(Default)]
pub struct OllamaState(pub Mutex<Option<std::process::Child>>);

#[derive(Clone, Serialize)]
pub struct OllamaProgress {
    pub phase: String,
    pub detail: String,
    pub completed: u64,
    pub total: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaInfo {
    pub running: bool,
    pub managed_installed: bool,
    pub models: Vec<String>,
}

/// Release asset for this OS/arch (values from `std::env::consts`).
pub fn asset_name(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", _) => Some("ollama-darwin.tgz"),
        ("windows", "x86_64") => Some("ollama-windows-amd64.zip"),
        ("linux", "x86_64") => Some("ollama-linux-amd64.tgz"),
        ("linux", "aarch64") => Some("ollama-linux-arm64.tgz"),
        _ => None,
    }
}

fn runtime_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir").join("ollama")
}

fn exe_path(dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        dir.join("ollama.exe")
    } else if cfg!(target_os = "linux") {
        dir.join("bin").join("ollama") // linux tgz layout: bin/ollama + lib/ollama
    } else {
        dir.join("ollama")
    }
}

async fn list_models() -> Option<Vec<String>> {
    let v: serde_json::Value = reqwest::Client::new()
        .get(format!("{OLLAMA_URL}/api/tags"))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some(
        v["models"]
            .as_array()?
            .iter()
            .filter_map(|m| m["name"].as_str().map(String::from))
            .collect(),
    )
}

#[tauri::command]
pub async fn ollama_status(app: tauri::AppHandle) -> OllamaInfo {
    let models = list_models().await;
    OllamaInfo {
        running: models.is_some(),
        managed_installed: exe_path(&runtime_dir(&app)).exists(),
        models: models.unwrap_or_default(),
    }
}

#[tauri::command]
pub async fn ollama_install(app: tauri::AppHandle) -> Result<(), String> {
    let asset = asset_name(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or("unsupported platform for the built-in local model runtime")?;
    let dir = runtime_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let url =
        format!("https://github.com/ollama/ollama/releases/download/{OLLAMA_VERSION}/{asset}");
    let res = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("runtime download failed: HTTP {}", res.status()));
    }
    let total = res.content_length().unwrap_or(0);
    let archive = dir.join(asset);
    let mut file = std::fs::File::create(&archive).map_err(|e| e.to_string())?;
    let mut stream = res.bytes_stream();
    let mut done: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        let _ = app.emit(
            "ollama:progress",
            OllamaProgress {
                phase: "download".into(),
                detail: asset.into(),
                completed: done,
                total,
            },
        );
    }
    drop(file);
    // One extraction path everywhere: macOS/Linux tar reads .tgz natively, and
    // Windows ships bsdtar (zip-capable) in System32 since Windows 10.
    let out = std::process::Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "runtime extract failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let _ = std::fs::remove_file(&archive);
    if !exe_path(&dir).exists() {
        return Err("runtime extracted but the ollama binary was not found".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn ollama_start(app: tauri::AppHandle) -> Result<(), String> {
    if list_models().await.is_some() {
        return Ok(()); // an Ollama (user's own or ours) is already serving
    }
    let exe = exe_path(&runtime_dir(&app));
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("serve").env("OLLAMA_HOST", "127.0.0.1:11434");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    *app.state::<OllamaState>().0.lock().unwrap() = Some(child);
    for _ in 0..40 {
        if list_models().await.is_some() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Err("the local model runtime did not start within 10s".into())
}

#[tauri::command]
pub async fn ollama_pull(app: tauri::AppHandle, model: String) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(format!("{OLLAMA_URL}/api/pull"))
        .json(&serde_json::json!({ "name": model, "stream": true }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("model download failed: HTTP {}", res.status()));
    }
    let mut stream = res.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        while let Some(nl) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=nl).collect();
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&line) {
                if let Some(err) = v["error"].as_str() {
                    return Err(err.to_string());
                }
                let _ = app.emit(
                    "ollama:progress",
                    OllamaProgress {
                        phase: "pull".into(),
                        detail: v["status"].as_str().unwrap_or("").to_string(),
                        completed: v["completed"].as_u64().unwrap_or(0),
                        total: v["total"].as_u64().unwrap_or(0),
                    },
                );
            }
        }
    }
    Ok(())
}

/// Kill the managed runtime on app exit. Never touches a user-installed Ollama.
pub fn kill(app: &tauri::AppHandle) {
    if let Some(mut child) = app.state::<OllamaState>().0.lock().unwrap().take() {
        let _ = child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_cover_supported_platforms() {
        assert_eq!(asset_name("macos", "aarch64"), Some("ollama-darwin.tgz"));
        assert_eq!(asset_name("macos", "x86_64"), Some("ollama-darwin.tgz"));
        assert_eq!(asset_name("windows", "x86_64"), Some("ollama-windows-amd64.zip"));
        assert_eq!(asset_name("linux", "x86_64"), Some("ollama-linux-amd64.tgz"));
        assert_eq!(asset_name("linux", "aarch64"), Some("ollama-linux-arm64.tgz"));
        assert_eq!(asset_name("freebsd", "x86_64"), None);
    }
}
```

- [ ] **Step 4: Add `open_url` to commands.rs**

In `src-tauri/src/commands.rs` (near the other simple commands):

```rust
/// Open an https URL in the default browser (onboarding's "Get Ollama" fallback).
#[tauri::command]
pub fn open_url(url: String) -> Result<()> {
    if !url.starts_with("https://") {
        return Err("only https urls can be opened".into());
    }
    open::that(url).map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Wire lib.rs**

In `src-tauri/src/lib.rs`:
- In setup, after the embedded-server lines: `app.manage(crate::ollama::OllamaState::default());`
- Add to `generate_handler![...]`: `commands::open_url,` `ollama::ollama_status,` `ollama::ollama_install,` `ollama::ollama_start,` `ollama::ollama_pull,`
- In the `RunEvent::Exit` arm, after `embedded_server::kill(app);` add: `crate::ollama::kill(app);`

- [ ] **Step 6: Test + check**

Run: `cargo test -p donna --lib ollama` — PASS.
Run: `cargo check --manifest-path src-tauri/Cargo.toml` — clean.

- [ ] **Step 7: Commit**

```bash
git add src-tauri
git commit -m "Managed Ollama runtime: download, serve, and pull models from inside the app"
git push
```

---

### Task 6: Onboarding — hands-free local brain

**Files:**
- Create: `src/components/LocalBrainSetup.tsx`
- Modify: `src/routes/Onboarding.tsx`

**Interfaces:**
- Consumes: commands + `ollama:progress` event from Task 5.
- Produces: `<LocalBrainSetup onReady={(models: string[]) => void} />`; exports `DEFAULT_LOCAL_MODEL = "qwen2.5:3b"`.

- [ ] **Step 1: Create the component**

`src/components/LocalBrainSetup.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ExternalLink, RefreshCw } from "lucide-react";
import { Button, Spinner } from "./ui";

export const DEFAULT_LOCAL_MODEL = "qwen2.5:3b";

type Phase = "checking" | "installing" | "starting" | "pulling" | "error";

interface Progress {
  phase: string;
  detail: string;
  completed: number;
  total: number;
}

interface OllamaInfo {
  running: boolean;
  managedInstalled: boolean;
  models: string[];
}

const PHASE_LABEL: Record<Exclude<Phase, "error">, string> = {
  checking: "Checking your machine…",
  installing: "Downloading the local AI runtime…",
  starting: "Starting the local AI runtime…",
  pulling: `Downloading your model (${DEFAULT_LOCAL_MODEL})…`,
};

/** Drives the zero-terminal local-model setup: runtime install → serve → model pull. */
export default function LocalBrainSetup({ onReady }: { onReady: (models: string[]) => void }) {
  const [phase, setPhase] = useState<Phase>("checking");
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const started = useRef(false);

  useEffect(() => {
    const un = listen<Progress>("ollama:progress", (e) => setProgress(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const run = async () => {
    setError(null);
    setProgress(null);
    try {
      setPhase("checking");
      let info = await invoke<OllamaInfo>("ollama_status");
      if (!info.running) {
        if (!info.managedInstalled) {
          setPhase("installing");
          await invoke("ollama_install");
        }
        setPhase("starting");
        await invoke("ollama_start");
        info = await invoke<OllamaInfo>("ollama_status");
      }
      if (info.models.length === 0) {
        setPhase("pulling");
        await invoke("ollama_pull", { model: DEFAULT_LOCAL_MODEL });
        info = await invoke<OllamaInfo>("ollama_status");
      }
      onReady(info.models);
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  };

  useEffect(() => {
    if (!started.current) {
      started.current = true;
      run();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (phase === "error") {
    return (
      <div className="space-y-3">
        <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
          {error}
        </p>
        <div className="flex gap-2">
          <Button variant="ghost" onClick={run}>
            <RefreshCw size={16} />
            Try again
          </Button>
          <Button
            variant="ghost"
            onClick={() => invoke("open_url", { url: "https://ollama.com/download" })}
          >
            <ExternalLink size={16} />
            Get Ollama from ollama.com
          </Button>
        </div>
        <p className="text-xs text-gray-500">
          If you install Ollama yourself, come back and press “Try again”.
        </p>
      </div>
    );
  }

  const pct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.completed / progress.total) * 100))
      : null;

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-sm text-gray-300">
        <Spinner />
        {PHASE_LABEL[phase]}
      </div>
      {(phase === "installing" || phase === "pulling") && (
        <div className="h-2 w-full overflow-hidden rounded-full bg-white/10">
          <div
            className="h-full rounded-full bg-donna-accent transition-all"
            style={{ width: `${pct ?? 5}%` }}
          />
        </div>
      )}
      {pct !== null && <p className="text-xs text-gray-500">{pct}%</p>}
      <p className="text-xs text-gray-500">
        Everything runs on your machine — nothing is sent anywhere.
      </p>
    </div>
  );
}
```

- [ ] **Step 2: Use it in Onboarding.tsx**

In `src/routes/Onboarding.tsx`:

1. Add imports:

```tsx
import LocalBrainSetup, { DEFAULT_LOCAL_MODEL } from "../components/LocalBrainSetup";
```

2. In the `configure` step JSX, replace the local branch of the `isLocal ? (…) : (…)` ternary — the “Ollama host” `<label>` block — with the component below. The cloud branch (the API-key `<label>`) stays exactly as it is today:

```tsx
            {isLocal ? (
              models.length === 0 ? (
                <LocalBrainSetup
                  onReady={(list) => {
                    const found = list.length > 0 ? list : [DEFAULT_LOCAL_MODEL];
                    setModels(found);
                    setModel(found[0]);
                  }}
                />
              ) : null
            ) : (
              <label className="block">
                {/* …the existing API-key input block, unchanged… */}
              </label>
            )}
```

Because the host input is gone, `setOllamaHost` becomes unused — change that state line to keep tsc happy:

```tsx
  const [ollamaHost] = useState(DEFAULT_OLLAMA_HOST);
```

3. Hide the manual “Detect models” button for the local path (the component drives itself). Change the Button that calls `fetchModels` to render only for cloud:

```tsx
            {!isLocal && (
              <Button variant="ghost" onClick={fetchModels} disabled={loadingModels}>
                {loadingModels ? <Spinner /> : <RefreshCw size={16} />}
                Verify key & load models
              </Button>
            )}
```

4. Everything else (model select, `finish`, error display) stays as-is; `ollamaHost` state keeps its `DEFAULT_OLLAMA_HOST` value which `finish()` saves — the managed runtime serves on exactly that host.

- [ ] **Step 3: Verify**

Run: `npm run build` — clean.
Manual matrix (in `npm run tauri:dev` after `localStorage.clear()`):
1. No Ollama installed → wizard downloads runtime, starts it, pulls `qwen2.5:3b` with progress, then shows the model select.
2. Own Ollama already running with models → wizard skips straight to the model select.
3. Kill network mid-download → error card with “Try again” and “Get Ollama from ollama.com”.

- [ ] **Step 4: Commit**

```bash
git add src/components/LocalBrainSetup.tsx src/routes/Onboarding.tsx
git commit -m "Onboarding installs and runs the local model runtime hands-free"
git push
```

---

# Phase 3 — Releases and auto-update

### Task 7: Tauri updater wiring

**Files:**
- Modify: `src-tauri/tauri.conf.json` (version, updater config, updater artifacts)
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `package.json` (version + JS plugins)
- Modify: `src/App.tsx` (update banner)

**Interfaces:**
- Consumes: GitHub Releases `latest.json` endpoint (generated by Task 8's CI).
- Produces: an in-app “Update & restart” banner.

- [ ] **Step 1: Generate the updater keypair**

```bash
npm run tauri signer generate -- -w ~/.tauri/donna-updater.key
```

Copy the printed **public key** for Step 2. Keep `~/.tauri/donna-updater.key` (and the password you chose) safe — Task 8 uploads them as repo secrets. Never commit them.

- [ ] **Step 2: Config**

In `src-tauri/tauri.conf.json`:
- `"version": "0.2.0"`
- In `bundle`, add `"createUpdaterArtifacts": true`
- Replace `"plugins": {}` with:

```json
  "plugins": {
    "updater": {
      "pubkey": "<PASTE THE PUBLIC KEY FROM STEP 1>",
      "endpoints": [
        "https://github.com/duckyquang/Donna/releases/latest/download/latest.json"
      ]
    }
  }
```

In `package.json`: `"version": "0.2.0"`.

- [ ] **Step 3: Rust side**

`src-tauri/Cargo.toml` dependencies:

```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

`src-tauri/src/lib.rs`, with the other plugins:

```rust
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
```

`src-tauri/capabilities/default.json` permissions — add:

```json
    "updater:default",
    "process:default"
```

- [ ] **Step 4: JS side**

```bash
npm install @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

In `src/App.tsx`, add imports and the banner component, and render it next to the unreachable banner:

```tsx
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
```

```tsx
function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    // Offline / rate-limited / dev builds: silently skip, try again next launch.
    check()
      .then((u) => u && setUpdate(u))
      .catch(() => {});
  }, []);

  if (!update) return null;
  return (
    <div className="flex items-center justify-center gap-3 border-b border-donna-accent/30 bg-donna-accent/10 px-4 py-2 text-xs text-gray-200">
      <span>Donna {update.version} is available.</span>
      <button
        disabled={installing}
        onClick={async () => {
          setInstalling(true);
          try {
            await update.downloadAndInstall();
            await relaunch();
          } catch {
            setInstalling(false);
          }
        }}
        className="rounded border border-donna-accent/50 px-2 py-0.5 font-medium text-gray-100 hover:bg-donna-accent/20"
      >
        {installing ? "Updating…" : "Update & restart"}
      </button>
    </div>
  );
}
```

In the main return, directly above the `{!reachable && (` banner line, add:

```tsx
      <UpdateBanner />
```

- [ ] **Step 5: Verify + commit**

Run: `npm run build` and `cargo check --manifest-path src-tauri/Cargo.toml` — both clean. (End-to-end update flow is proven in Task 8's prerelease.)

```bash
git add src-tauri package.json package-lock.json src/App.tsx
git commit -m "Wire the Tauri updater: version 0.2.0, update banner, updater artifacts"
git push
```

---

### Task 8: Release workflow — sidecar per target + updater signing

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `docs/BUILD.md`

**Interfaces:**
- Consumes: `scripts/build-sidecar.sh` (Task 1), updater key (Task 7).
- Produces: GitHub Release with installers **and** `latest.json` + `.sig` files.

- [ ] **Step 1: Upload the signing secrets**

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/donna-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body "<the password from Task 7 Step 1>"
```

- [ ] **Step 2: Update the workflow**

Replace the `matrix.include` block and add the sidecar step. Full updated `jobs.release` in `.github/workflows/release.yml`:

```yaml
jobs:
  release:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: macos-latest
            target: aarch64-apple-darwin
            args: --target aarch64-apple-darwin
          - platform: macos-latest
            target: x86_64-apple-darwin
            args: --target x86_64-apple-darwin
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            args: ""
          - platform: windows-latest
            target: x86_64-pc-windows-msvc
            args: ""

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm

      - name: Install frontend dependencies
        run: npm ci

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Build donna-server sidecar
        shell: bash
        run: bash scripts/build-sidecar.sh ${{ matrix.target }}

      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Donna ${{ github.ref_name }}"
          releaseBody: "See the assets to download and install Donna for your platform."
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.args }}
```

- [ ] **Step 3: Document the release ritual**

In `docs/BUILD.md`, replace the `## Release builds (CI)` section body with:

```markdown
Tagged releases (`v*`) trigger `.github/workflows/release.yml`, which builds the
donna-server sidecar plus macOS (Apple Silicon + Intel), Linux, and Windows
installers, signs the update artifacts, and attaches everything (including
`latest.json` for the in-app updater) to a GitHub Release draft.

```bash
git tag v0.2.0
git push origin v0.2.0
```

Then open the draft release on GitHub and **publish** it — the in-app updater and
the landing page's download buttons both read `releases/latest`, which only sees
published releases. Requires the `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repo secrets (from `tauri signer generate`).
```

- [ ] **Step 4: Prove it with a prerelease tag**

```bash
git tag v0.2.0-rc1
git push origin v0.2.0-rc1
gh run watch
```

Expected: all four matrix jobs green; the draft release contains `.dmg` ×2, `.msi`/`.exe`, `.AppImage`/`.deb`, `latest.json`, and `.sig` files. Install the artifact for your own OS and confirm: app opens (after Gatekeeper “Open Anyway”), chat works with zero manual config. Delete the rc draft afterwards: `gh release delete v0.2.0-rc1 --yes` and `git push --delete origin v0.2.0-rc1`.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml docs/BUILD.md
git commit -m "Release CI builds the sidecar per target and signs updater artifacts"
git push
```

---

# Phase 4 — Landing page + README

### Task 9: The landing page

**Files:**
- Create: `site/index.html`, `site/styles.css`, `site/main.js`
- Create: `site/assets/` (Donna icon + brand SVGs)

**Interfaces:**
- Consumes: GitHub Releases API (`releases/latest`) for download links.
- Produces: a self-contained static page; Task 10 deploys the `site/` folder as-is.

**Design constraints (from the reference images):** light warm-gray page (`#f4f4f2`) with a subtle dot texture in the hero; near-black display headline with the second line in gray; floating white UI cards with big radii (16–24 px) and soft shadows; a blue-framed product mock section; small pill section-labels (“Features”, “Privacy”); generous whitespace. No emojis; typographic quotes and dashes in all copy.

- [ ] **Step 1: Gather assets**

```bash
mkdir -p site/assets
cp src-tauri/icons/128x128@2x.png site/assets/donna-icon.png
cd site/assets
curl -fL -o gmail.svg      "https://upload.wikimedia.org/wikipedia/commons/7/7e/Gmail_icon_%282020%29.svg"
curl -fL -o gcal.svg       "https://upload.wikimedia.org/wikipedia/commons/a/a5/Google_Calendar_icon_%282020%29.svg"
curl -fL -o gdrive.svg     "https://upload.wikimedia.org/wikipedia/commons/1/12/Google_Drive_icon_%282020%29.svg"
curl -fL -o slack.svg      "https://upload.wikimedia.org/wikipedia/commons/d/d5/Slack_icon_2019.svg"
curl -fL -o whatsapp.svg   "https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg"
cd ../..
```

Verify each downloaded file starts with `<svg` or `<?xml` (`head -c 100 site/assets/*.svg`). If any URL 404s, find the file on commons.wikimedia.org (search the product name, pick the official-logo SVG) and substitute that URL. Fathom and Discord get monogram tiles built in CSS (no reliable brand SVG) — no download needed.

- [ ] **Step 2: Write `site/index.html`**

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Donna — the AI assistant that's actually yours</title>
  <meta name="description" content="Donna is a private AI personal assistant that runs on your computer. She remembers, reminds, drafts, and gets work done — before you ask." />
  <link rel="icon" href="assets/donna-icon.png" />
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet" />
  <link rel="stylesheet" href="styles.css" />
</head>
<body>

<nav class="nav">
  <a class="brand" href="#top"><img src="assets/donna-icon.png" alt="" /><span>Donna</span></a>
  <div class="nav-links">
    <a href="#features">Features</a>
    <a href="#integrations">Integrations</a>
    <a href="#privacy">Privacy</a>
    <a href="#faq">FAQ</a>
  </div>
  <div class="nav-cta">
    <a href="https://github.com/duckyquang/Donna" class="ghost">GitHub</a>
    <a href="#download" class="button small">Download</a>
  </div>
</nav>

<header class="hero" id="top">
  <div class="hero-cards" aria-hidden="true">
    <div class="card sticky-note">Remind me to send the deck before Friday’s standup — and draft the follow-up email.</div>
    <div class="card mini-card reminders">
      <h4>Reminders</h4>
      <div class="row"><span class="dot blue"></span>Meeting brief — 13:00</div>
      <div class="row"><span class="dot green"></span>Follow up with Sarah</div>
    </div>
    <div class="card mini-card tasks">
      <h4>Today</h4>
      <div class="row"><span class="check"></span>Recap of yesterday’s call</div>
      <div class="row"><span class="check done"></span>Morning briefing</div>
    </div>
    <div class="card logo-cluster">
      <img src="assets/gmail.svg" alt="Gmail" />
      <img src="assets/gcal.svg" alt="Google Calendar" />
      <img src="assets/slack.svg" alt="Slack" />
      <img src="assets/whatsapp.svg" alt="WhatsApp" />
    </div>
  </div>

  <img class="hero-icon" src="assets/donna-icon.png" alt="Donna app icon" />
  <h1>Your day, handled<span class="muted">by an assistant that’s yours</span></h1>
  <p class="sub">Donna is a private AI assistant that lives on your computer. She remembers your people and projects, connects to your tools, and gets work done — before you ask.</p>
  <div class="cta" id="download">
    <a class="button" data-download href="https://github.com/duckyquang/Donna/releases/latest">Download Donna</a>
    <a class="all-platforms" href="https://github.com/duckyquang/Donna/releases/latest">All platforms <span data-version></span> →</a>
  </div>
  <p class="fineprint">Free &amp; open source · macOS, Windows, Linux · <a href="#faq-open">First launch on macOS?</a></p>
</header>

<section class="strip">
  <div class="point">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v4M12 18v4M4.9 4.9l2.8 2.8M16.3 16.3l2.8 2.8M2 12h4M18 12h4M4.9 19.1l2.8-2.8M16.3 7.7l2.8-2.8"/></svg>
    <p><strong>Proactive by default.</strong> Briefings, reminders, and drafts arrive without being asked.</p>
  </div>
  <div class="point">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="10" width="16" height="10" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>
    <p><strong>Private by design.</strong> Runs on your machine. Your data never leaves unless you say so.</p>
  </div>
  <div class="point">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>
    <p><strong>Yours forever.</strong> One download, no subscription, no account. MIT-licensed.</p>
  </div>
</section>

<section class="showcase">
  <span class="pill">Meet Donna</span>
  <h2>A chief of staff,<br />living on your desktop</h2>
  <div class="frame">
    <div class="app-mock">
      <div class="mock-sidebar">
        <div class="mock-logo"></div>
        <div class="mock-nav-item active"></div>
        <div class="mock-nav-item"></div>
        <div class="mock-nav-item"></div>
        <div class="mock-nav-item"></div>
      </div>
      <div class="mock-main">
        <div class="mock-greeting">Good morning</div>
        <div class="mock-grid">
          <div class="mock-card tall">
            <h5>Chat</h5>
            <div class="bubble donna">Your 1:1 with Alex moved to 2 pm — I updated the calendar and drafted your prep notes.</div>
            <div class="bubble user">Perfect. Remind me to review them at 1:30.</div>
            <div class="bubble donna">Done — reminder set for 1:30 pm.</div>
          </div>
          <div class="mock-card">
            <h5>Today</h5>
            <div class="bar" style="--w:65%"></div>
            <div class="bar" style="--w:40%"></div>
            <div class="bar" style="--w:80%"></div>
          </div>
          <div class="mock-card">
            <h5>Memory</h5>
            <div class="node-row"><span class="node"></span><span class="node"></span><span class="node"></span></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</section>

<section class="features" id="features">
  <span class="pill">Features</span>
  <h2>Everything in one place</h2>
  <p class="section-sub">Forget juggling apps — Donna keeps your day together.</p>
  <div class="grid">
    <div class="feature">
      <h3>Chat that remembers</h3>
      <p>Talk to Donna like a person. She keeps a visible, editable memory of your people, projects, and preferences — and it sharpens with every conversation.</p>
    </div>
    <div class="feature">
      <h3>Proactive notifications</h3>
      <p>Morning briefings, meeting prep, and follow-up nudges land as native notifications — before you think to ask.</p>
    </div>
    <div class="feature">
      <h3>Docs that write themselves</h3>
      <p>A recap appears when a meeting ends. A note appears when something important arrives. You just read them.</p>
    </div>
    <div class="feature">
      <h3>Calendar, synced both ways</h3>
      <p>A personal calendar with two-way Google Calendar sync — Donna can view, create, and move events for you.</p>
    </div>
    <div class="feature">
      <h3>Voice, in and out</h3>
      <p>Push-to-talk from the desktop, voice notes over WhatsApp — Donna listens, and answers out loud.</p>
    </div>
    <div class="feature">
      <h3>Skills she teaches herself</h3>
      <p>Recurring recipes become reusable skills Donna can write for herself — and you can read every one.</p>
    </div>
  </div>
</section>

<section class="integrations" id="integrations">
  <span class="pill">Integrations</span>
  <h2>Connects to the tools<br />you already use</h2>
  <div class="logo-grid">
    <div class="tile"><img src="assets/gmail.svg" alt="Gmail" /><span>Gmail</span></div>
    <div class="tile"><img src="assets/gcal.svg" alt="Google Calendar" /><span>Calendar</span></div>
    <div class="tile"><img src="assets/gdrive.svg" alt="Google Drive" /><span>Drive</span></div>
    <div class="tile"><img src="assets/slack.svg" alt="Slack" /><span>Slack</span></div>
    <div class="tile"><img src="assets/whatsapp.svg" alt="WhatsApp" /><span>WhatsApp</span></div>
    <div class="tile"><span class="monogram">F</span><span>Fathom</span></div>
    <div class="tile"><span class="monogram">D</span><span>Discord</span></div>
    <div class="tile more"><span>More on the roadmap</span></div>
  </div>
</section>

<section class="privacy" id="privacy">
  <span class="pill">Privacy</span>
  <h2>Runs on your machine.<br />Stays on your machine.</h2>
  <div class="privacy-grid">
    <div class="privacy-card"><h3>Local first</h3><p>Chats, memory, and docs live in a database on your device — not on someone else’s server.</p></div>
    <div class="privacy-card"><h3>Your keys, your models</h3><p>Use a free local model, or bring your own OpenAI, Anthropic, or Google key — stored in your system keychain.</p></div>
    <div class="privacy-card"><h3>No telemetry</h3><p>Donna doesn’t phone home. When data must leave your device — a cloud model, an integration — she tells you.</p></div>
    <div class="privacy-card"><h3>Open source</h3><p>MIT-licensed and auditable, top to bottom. If you can read code, you can check every claim on this page.</p></div>
  </div>
</section>

<section class="faq" id="faq">
  <span class="pill">FAQ</span>
  <h2>Questions, answered</h2>
  <details id="faq-open">
    <summary>macOS says “Donna can’t be opened” — what now?</summary>
    <p>Donna isn’t code-signed with Apple yet (that costs $99/year — it’s coming). The first time only: open <strong>System Settings → Privacy &amp; Security</strong>, scroll down, and click <strong>Open Anyway</strong> next to Donna. After that she opens normally.</p>
  </details>
  <details>
    <summary>Windows shows “Windows protected your PC”</summary>
    <p>Same reason — no paid certificate yet. Click <strong>More info</strong>, then <strong>Run anyway</strong>. Once, and only once.</p>
  </details>
  <details>
    <summary>Is it really free?</summary>
    <p>Yes. Donna is open source (MIT). With a local model everything is free and private. If you want a frontier model instead, you pay your own API usage — to the provider, not to us.</p>
  </details>
  <details>
    <summary>Which AI does Donna use?</summary>
    <p>Your choice. On first launch she can set up a free local model that runs entirely on your machine, or you can paste an OpenAI, Anthropic, or Google API key. Switch anytime in Settings.</p>
  </details>
  <details>
    <summary>Can Donna work while my computer is off?</summary>
    <p>Out of the box she works whenever your computer is awake (she keeps running in the menu bar). For a true 24/7 assistant you can self-host her brain on any always-on box — see the <a href="https://github.com/duckyquang/Donna#server-first-architecture">self-hosting guide</a>.</p>
  </details>
</section>

<footer class="footer">
  <div class="brand"><img src="assets/donna-icon.png" alt="" /><span>Donna</span></div>
  <p>The assistant that’s actually yours.</p>
  <div class="footer-links">
    <a href="https://github.com/duckyquang/Donna">GitHub</a>
    <a href="https://github.com/duckyquang/Donna/blob/main/LICENSE">MIT License</a>
    <a href="https://github.com/duckyquang/Donna/issues">Report an issue</a>
  </div>
</footer>

<script src="main.js"></script>
</body>
</html>
```

- [ ] **Step 3: Write `site/main.js`**

```js
// OS-aware download button + latest version from the GitHub Releases API.
const REPO = "duckyquang/Donna";

function detectOS() {
  const p = `${navigator.platform} ${navigator.userAgent}`;
  if (/Mac/i.test(p)) return "mac";
  if (/Win/i.test(p)) return "windows";
  if (/Linux/i.test(p)) return "linux";
  return "other";
}

const OS_LABEL = {
  mac: "Download for macOS",
  windows: "Download for Windows",
  linux: "Download for Linux",
  other: "Download Donna",
};

async function latestAssets() {
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`);
  if (!res.ok) return null;
  const rel = await res.json();
  const find = (re) => rel.assets.find((a) => re.test(a.name))?.browser_download_url;
  return {
    version: rel.tag_name,
    mac: find(/aarch64\.dmg$/) || find(/\.dmg$/),
    windows: find(/set(up)?\.exe$/i) || find(/\.msi$/) || find(/\.exe$/),
    linux: find(/\.AppImage$/) || find(/\.deb$/),
  };
}

(async () => {
  const os = detectOS();
  const btn = document.querySelector("[data-download]");
  btn.textContent = OS_LABEL[os];
  const assets = await latestAssets().catch(() => null);
  if (!assets) return; // button already links to the releases page
  if (assets[os]) btn.href = assets[os];
  const v = document.querySelector("[data-version]");
  if (v && assets.version) v.textContent = `· ${assets.version}`;
})();
```

- [ ] **Step 4: Write `site/styles.css`**

Baseline stylesheet implementing the design constraints — complete and shippable; polish iterations on top of it are welcome but keep the tokens:

```css
/* Tokens */
:root {
  --bg: #f4f4f2;
  --surface: #ffffff;
  --ink: #111113;
  --muted: #8a8a86;
  --line: #e4e4e0;
  --accent: #2563eb;
  --radius: 20px;
  --shadow: 0 10px 30px rgba(17, 17, 19, 0.08);
  --font: "Inter", system-ui, -apple-system, sans-serif;
}
* { margin: 0; box-sizing: border-box; }
html { scroll-behavior: smooth; }
body { font-family: var(--font); background: var(--bg); color: var(--ink); line-height: 1.55; }
img { max-width: 100%; }
a { color: inherit; text-decoration: none; }
h1, h2 { font-weight: 700; letter-spacing: -0.03em; line-height: 1.08; }
section { padding: 96px 24px; max-width: 1080px; margin: 0 auto; text-align: center; }

/* Nav */
.nav { position: sticky; top: 0; z-index: 10; display: flex; align-items: center; justify-content: space-between; padding: 14px 28px; background: rgba(244, 244, 242, 0.85); backdrop-filter: blur(12px); border-bottom: 1px solid var(--line); }
.brand { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 17px; }
.brand img { width: 26px; height: 26px; border-radius: 7px; }
.nav-links { display: flex; gap: 26px; font-size: 14px; color: #444; }
.nav-links a:hover { color: var(--ink); }
.nav-cta { display: flex; align-items: center; gap: 16px; font-size: 14px; }
.ghost:hover { color: var(--accent); }

/* Buttons */
.button { display: inline-block; background: var(--accent); color: #fff; font-weight: 600; font-size: 15px; padding: 13px 26px; border-radius: 12px; transition: background 0.15s, transform 0.15s; }
.button:hover { background: #1d4fd8; transform: translateY(-1px); }
.button.small { padding: 8px 16px; font-size: 13px; }

/* Hero */
.hero { position: relative; text-align: center; padding: 110px 24px 130px; max-width: none; background-image: radial-gradient(var(--line) 1px, transparent 1px); background-size: 22px 22px; }
.hero-icon { width: 84px; height: 84px; border-radius: 22px; box-shadow: var(--shadow); margin-bottom: 34px; }
.hero h1 { font-size: clamp(40px, 7vw, 72px); }
.hero h1 .muted { display: block; color: var(--muted); }
.hero .sub { max-width: 560px; margin: 22px auto 34px; color: #555; font-size: 17px; }
.cta { display: flex; flex-direction: column; align-items: center; gap: 14px; }
.all-platforms { font-size: 13px; color: var(--muted); }
.all-platforms:hover { color: var(--ink); }
.fineprint { margin-top: 28px; font-size: 12px; color: var(--muted); }
.fineprint a { text-decoration: underline; }

/* Floating hero cards */
.hero-cards .card { position: absolute; background: var(--surface); border-radius: var(--radius); box-shadow: var(--shadow); }
.sticky-note { top: 120px; left: max(24px, 6vw); width: 210px; padding: 18px; background: #fdf3b4; transform: rotate(-5deg); font-size: 13.5px; text-align: left; font-family: "Comic Sans MS", "Bradley Hand", cursive; }
.mini-card { width: 220px; padding: 16px; text-align: left; font-size: 13px; }
.mini-card h4 { font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); margin-bottom: 10px; }
.mini-card .row { display: flex; align-items: center; gap: 8px; padding: 6px 0; border-top: 1px solid var(--line); }
.reminders { top: 110px; right: max(24px, 6vw); transform: rotate(4deg); }
.tasks { bottom: 90px; left: max(24px, 8vw); transform: rotate(3deg); }
.dot { width: 8px; height: 8px; border-radius: 50%; }
.dot.blue { background: var(--accent); }
.dot.green { background: #16a34a; }
.check { width: 14px; height: 14px; border: 1.5px solid var(--line); border-radius: 5px; }
.check.done { background: var(--accent); border-color: var(--accent); }
.logo-cluster { bottom: 110px; right: max(24px, 8vw); display: flex; gap: 12px; padding: 16px 18px; transform: rotate(-3deg); }
.logo-cluster img { width: 30px; height: 30px; }
@media (max-width: 900px) { .hero-cards { display: none; } }

/* Strip */
.strip { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 28px; text-align: left; padding-top: 40px; padding-bottom: 40px; }
.point svg { width: 26px; height: 26px; color: var(--accent); margin-bottom: 10px; }
.point p { font-size: 14.5px; color: #555; }
.point strong { color: var(--ink); display: block; margin-bottom: 2px; }

/* Section chrome */
.pill { display: inline-block; font-size: 12.5px; font-weight: 500; color: #555; background: var(--surface); border: 1px solid var(--line); border-radius: 999px; padding: 6px 16px; box-shadow: 0 2px 8px rgba(17,17,19,0.05); margin-bottom: 22px; }
section h2 { font-size: clamp(30px, 4.5vw, 44px); margin-bottom: 14px; }
.section-sub { color: var(--muted); margin-bottom: 40px; }

/* Showcase */
.frame { margin-top: 44px; background: linear-gradient(180deg, #5aa2f7, var(--accent)); border-radius: 28px; padding: clamp(16px, 4vw, 48px); box-shadow: var(--shadow); }
.app-mock { display: flex; background: #16161a; border-radius: 16px; overflow: hidden; text-align: left; min-height: 380px; }
.mock-sidebar { width: 64px; background: #1d1d22; padding: 16px 12px; display: flex; flex-direction: column; gap: 14px; }
.mock-logo { width: 30px; height: 30px; border-radius: 9px; background: var(--accent); }
.mock-nav-item { height: 10px; border-radius: 5px; background: #2e2e35; }
.mock-nav-item.active { background: #3d5afe66; }
.mock-main { flex: 1; padding: 22px; }
.mock-greeting { color: #eee; font-weight: 600; font-size: 18px; margin-bottom: 16px; }
.mock-grid { display: grid; grid-template-columns: 2fr 1fr; gap: 12px; }
.mock-card { background: #1d1d22; border-radius: 12px; padding: 14px; }
.mock-card.tall { grid-row: span 2; }
.mock-card h5 { color: #888; font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; margin-bottom: 10px; }
.bubble { font-size: 12.5px; padding: 9px 12px; border-radius: 10px; margin-bottom: 8px; max-width: 90%; }
.bubble.donna { background: #2a2a31; color: #ddd; }
.bubble.user { background: var(--accent); color: #fff; margin-left: auto; }
.bar { height: 8px; border-radius: 4px; background: linear-gradient(90deg, var(--accent) var(--w), #2e2e35 var(--w)); margin-bottom: 10px; }
.node-row { display: flex; gap: 10px; }
.node { width: 22px; height: 22px; border-radius: 50%; background: #2e2e35; border: 2px solid var(--accent); }
@media (max-width: 700px) { .mock-grid { grid-template-columns: 1fr; } }

/* Features */
.grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 18px; }
.feature { background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); padding: 28px; text-align: left; box-shadow: 0 4px 14px rgba(17,17,19,0.04); }
.feature h3 { font-size: 17px; margin-bottom: 8px; }
.feature p { font-size: 14px; color: #555; }

/* Integrations */
.logo-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 14px; margin-top: 40px; }
.tile { background: var(--surface); border: 1px solid var(--line); border-radius: 18px; padding: 22px 12px; display: flex; flex-direction: column; align-items: center; gap: 10px; font-size: 13px; color: #555; box-shadow: 0 4px 14px rgba(17,17,19,0.04); }
.tile img { width: 34px; height: 34px; }
.monogram { display: flex; align-items: center; justify-content: center; width: 34px; height: 34px; border-radius: 10px; background: var(--ink); color: #fff; font-weight: 700; }
.tile.more { justify-content: center; color: var(--muted); border-style: dashed; box-shadow: none; }

/* Privacy */
.privacy-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 18px; margin-top: 40px; }
.privacy-card { background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); padding: 26px; text-align: left; }
.privacy-card h3 { font-size: 16px; margin-bottom: 8px; }
.privacy-card p { font-size: 14px; color: #555; }

/* FAQ */
.faq { max-width: 720px; }
.faq details { background: var(--surface); border: 1px solid var(--line); border-radius: 14px; padding: 18px 22px; margin-bottom: 10px; text-align: left; }
.faq summary { cursor: pointer; font-weight: 600; font-size: 15px; }
.faq details p { margin-top: 10px; font-size: 14px; color: #555; }
.faq a { color: var(--accent); text-decoration: underline; }

/* Footer */
.footer { text-align: center; padding: 60px 24px; border-top: 1px solid var(--line); color: var(--muted); font-size: 13.5px; }
.footer .brand { justify-content: center; display: flex; margin-bottom: 8px; }
.footer .brand img { width: 22px; height: 22px; }
.footer-links { display: flex; justify-content: center; gap: 22px; margin-top: 14px; }
.footer-links a:hover { color: var(--ink); }

@media (max-width: 760px) { .nav-links { display: none; } }
```

- [ ] **Step 5: Verify locally**

```bash
python3 -m http.server 4173 --directory site
```

Open http://localhost:4173 and check: hero renders with floating cards (desktop width) and hides them under 900 px; download button says “Download for macOS” on a Mac; all five SVG logos render; FAQ details open/close; no horizontal scrollbar at 375 px width; zero emojis on the page.

- [ ] **Step 6: Commit**

```bash
git add site
git commit -m "Add the Donna landing page (static site/, OS-aware downloads, honest FAQ)"
git push
```

---

### Task 10: Deploy to GitHub Pages

**Files:**
- Create: `.github/workflows/pages.yml`

- [ ] **Step 1: Workflow**

```yaml
name: Deploy landing page

on:
  push:
    branches: [main]
    paths: ["site/**"]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/configure-pages@v5
      - uses: actions/upload-pages-artifact@v3
        with:
          path: site
      - id: deployment
        uses: actions/deploy-pages@v4
```

- [ ] **Step 2: Enable Pages (once)**

```bash
gh api -X POST repos/duckyquang/Donna/pages -f build_type=workflow
```

(If it 409s, Pages is already enabled — set Source to “GitHub Actions” in repo Settings → Pages.)

- [ ] **Step 3: Deploy + verify**

Commit, push, then trigger (the `paths` filter only fires on `site/**` changes on main; use dispatch if needed):

```bash
git add .github/workflows/pages.yml
git commit -m "Deploy site/ to GitHub Pages"
git push
gh workflow run "Deploy landing page" && gh run watch
```

Open `https://duckyquang.github.io/Donna/` — page renders with assets (relative paths), download button resolves.

Note: the workflow triggers on **main** only. If this work lands on a feature branch first, the site deploys when the branch reaches main — until then, verify with `gh workflow run` (dispatch runs against the default branch, so merge at least the `site/` folder + workflow before expecting a live deploy).

---

### Task 11: README rewrite

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Rewrite the quick start**

Replace the whole `## Quick start (for everyone)` section with:

```markdown
## Quick start (for everyone)

No coding required. One download.

1. **Download Donna** from the [landing page](https://duckyquang.github.io/Donna/)
   (or grab an installer from [Releases](https://github.com/duckyquang/Donna/releases/latest)).
2. **Open it.** First launch on macOS: System Settings → Privacy & Security →
   **Open Anyway** (Donna isn't Apple-notarized yet). On Windows: **More info → Run
   anyway**. Once, and only once.
3. **Follow the onboarding.** Donna sets up her own brain — a free local model
   downloaded for you, or your own OpenAI/Anthropic/Google API key — then connects
   your tools from the Integrations page.

Everything else — her server, her memory, updates — is built in and automatic. Say
hi in the **Chat** tab.
```

- [ ] **Step 2: Re-home the server content**

In the `## Server-first architecture` section, retitle it `## Self-hosting the server (advanced)` and open with:

```markdown
Donna ships with her brain built in — the app runs a bundled `donna-server`
automatically, so most people never think about it. Self-hosting is for power users
who want a 24/7 assistant that works while their computer sleeps: run `donna-server`
on any always-on box and point **Settings → Server** at it.
```

Keep the existing links to the design spec and `donna-server/README.md`. Add the landing page link near the top badges: after the tagline line, add

```markdown
**[Download Donna →](https://duckyquang.github.io/Donna/)**
```

Also update the stale note under “1. Install Donna” if it still exists anywhere else, and update the `## For developers` run-from-source block to include `npm run sidecar` before `npm run tauri dev`.

- [ ] **Step 3: Verify + commit**

Read the diff — no remaining instructions that require a terminal in the “for everyone” path.

```bash
git add README.md
git commit -m "README: download-first quick start; Docker server moves under self-hosting"
git push
```

---

## Post-plan follow-ups (not tasks)

- Replace the CSS app mock on the landing page with real captured screenshots once the packaged app looks final.
- If/when an Apple Developer account is purchased, add notarization env vars to `release.yml` — no design change needed.
