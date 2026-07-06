# Phase 1: Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure Donna into a Cargo workspace where an always-on axum server owns the brain (DB, knowledge base, scheduler, integrations) and the Tauri desktop app becomes a client — with data migration, Docker + Cloudflare Tunnel deploy, and zero functional regression.

**Architecture:** Extract all portable logic from `src-tauri` into `crates/donna-core`. Replace the toy `donna-server` with an axum app exposing `POST /rpc/:command` (mirroring the existing Tauri command interface 1:1) plus a WebSocket for chat streaming and notification push. The React UI keeps its `api` object; only the `invoke` wrapper and the two `Channel` call sites change. Native-only concerns (screenshots, mic, local project files, OAuth browser flows, editor launching) stay as Tauri commands.

**Tech Stack:** Rust workspace (existing deps + axum 0.7, tower, tar, flate2), React/TS (fetch + WebSocket), Docker compose + cloudflared.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-07-donna-jarvis-design.md` — Phase 1 only. No agent loop, no WhatsApp webhook, no voice (Phases 2+).
- **Commit AND push after every task** (owner directive). Branch: `feat/phase-5-projects-discord-proactive`, remote `origin` (github.com/duckyquang/Donna). No PRs.
- Rust edition 2021, rust-version 1.77. Frontend: React 18 + Vite, TypeScript strict.
- Every existing UI feature must still work at the end of Task 12 (running against a local server).
- Server config is env-vars only: `DONNA_DATA_DIR` (default `./donna-data`), `DONNA_TOKEN` (required, no default), `DONNA_PORT` (default `8377`).
- RPC response envelope: success = the command's JSON result verbatim (same shape Tauri returned); error = HTTP 400 with `{"error": "<message>"}`. Auth: `Authorization: Bearer <DONNA_TOKEN>` on everything except `GET /health`.
- Secrets on server: plain JSON file at `$DONNA_DATA_DIR/secrets.json`, mode 0600. `// ponytail: plaintext-on-own-VPS, upgrade path is age encryption if the box is shared.`

---

### Task 0: Commit and push existing WIP

The tree has uncommitted Phase 5/6 work (quick chat, Dashboard, capabilities). It must be its own commit before restructuring touches the same files.

**Files:**
- Modify: none (git only)

- [ ] **Step 1: Verify remote and auth**

Run: `git remote -v && gh auth status`
Expected: `origin  https://github.com/duckyquang/Donna.git` (or SSH equivalent); gh logged in as duckyquang. If gh is not authenticated but the remote uses SSH with a working key, that is fine — test with `git ls-remote origin HEAD`.

- [ ] **Step 2: Review what the WIP contains**

Run: `git status --short && git diff --stat`
Expected: modified `src-tauri/{Cargo.toml,Cargo.lock,capabilities/default.json,src/commands.rs,src/lib.rs}`, `src/{App.tsx,components/Sidebar.tsx,components/mindmap/KgCircleNode.tsx,lib/api.ts,routes/Integrations.tsx,styles/global.css}`; untracked `src-tauri/capabilities/quick-chat.json`, `src-tauri/src/quick_chat.rs`, `src/routes/Dashboard.tsx`, `src/routes/QuickChat.tsx`. If anything else appears, stop and report before committing.

- [ ] **Step 3: Commit and push**

```bash
git add -A
git commit -m "Add quick chat overlay (Cmd+D), dashboard home, integration hub updates

Phase 5/6 work in progress: global-shortcut quick chat with screen context,
dashboard with news/gmail/calendar cards, sidebar and mindmap styling.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push -u origin feat/phase-5-projects-discord-proactive
```

Expected: push succeeds. This is the rollback point for the whole phase.

---

### Task 1: Cargo workspace scaffold

**Files:**
- Create: `Cargo.toml` (repo root)
- Create: `crates/donna-core/Cargo.toml`, `crates/donna-core/src/lib.rs`
- Modify: `src-tauri/Cargo.toml` (no dep changes yet — just joins workspace implicitly)
- Modify: `donna-server/Cargo.toml` (joins workspace)
- Modify: `.gitignore` (root `target/`)

**Interfaces:**
- Produces: workspace members `donna` (src-tauri), `donna-server`, `donna-core`; later tasks add code to `donna-core`.

- [ ] **Step 1: Write root workspace manifest**

```toml
# Cargo.toml (repo root)
[workspace]
resolver = "2"
members = ["src-tauri", "donna-server", "crates/donna-core"]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
futures-util = "0.3"
keyring = "2"
rusqlite = { version = "0.31", features = ["bundled"] }
thiserror = "1"
chrono = { version = "0.4", features = ["clock"] }
chrono-tz = "0.9"
```

- [ ] **Step 2: Create the empty donna-core crate**

```toml
# crates/donna-core/Cargo.toml
[package]
name = "donna-core"
version = "0.1.0"
edition = "2021"
rust-version = "1.77"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
reqwest = { workspace = true }
futures-util = { workspace = true }
keyring = { workspace = true }
rusqlite = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
chrono-tz = { workspace = true }
```

```rust
// crates/donna-core/src/lib.rs
//! Donna's portable brain: DB, knowledge base, providers, integrations.
//! Consumed by the Tauri desktop app and donna-server.
```

- [ ] **Step 3: Point src-tauri and donna-server at workspace deps**

In `src-tauri/Cargo.toml` and `donna-server/Cargo.toml`, replace each dependency line that exists in `[workspace.dependencies]` with the `{ workspace = true }` form (e.g. `serde = { workspace = true }`). Leave Tauri-specific deps (`tauri`, `tauri-plugin-*`, `tauri-build`) untouched. Delete `donna-server/Cargo.lock` if present (workspace uses the root lock; `src-tauri/Cargo.lock` moves to root: `git mv src-tauri/Cargo.lock Cargo.lock`).

- [ ] **Step 4: Verify everything builds**

Run: `cargo check --workspace`
Expected: compiles (donna-core is empty, others unchanged). Then `npm run tauri dev` briefly to confirm the desktop app still launches (tauri finds `src-tauri` by convention; workspace root Cargo.toml is supported by Tauri 2).

- [ ] **Step 5: Commit and push**

```bash
git add Cargo.toml Cargo.lock crates/ src-tauri/Cargo.toml donna-server/Cargo.toml .gitignore
git commit -m "Restructure into Cargo workspace with donna-core crate

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 2: Move leaf modules into donna-core

Everything except `commands.rs`, `scheduler.rs`, `quick_chat.rs`, `lib.rs`, `main.rs` is Tauri-free (verified by grep — only `embeddings.rs:40` uses `tauri::async_runtime` inside dead code `spawn_reindex`, which the spec says to delete).

**Files:**
- Move: `src-tauri/src/{error,db,secrets,oauth,providers,knowledge,docs,embeddings,retrieval}.rs` → `crates/donna-core/src/`
- Move: `src-tauri/src/integrations/` → `crates/donna-core/src/integrations/`
- Modify: `crates/donna-core/src/lib.rs`, `src-tauri/src/lib.rs`, `crates/donna-core/src/embeddings.rs`

**Interfaces:**
- Produces: `donna_core::{error, db, secrets, oauth, providers, knowledge, docs, embeddings, retrieval, integrations}` — all existing pub items unchanged.
- Consumes: workspace from Task 1.

- [ ] **Step 1: Move the files**

```bash
mkdir -p crates/donna-core/src
git mv src-tauri/src/error.rs src-tauri/src/db.rs src-tauri/src/secrets.rs \
       src-tauri/src/oauth.rs src-tauri/src/providers.rs src-tauri/src/knowledge.rs \
       src-tauri/src/docs.rs src-tauri/src/embeddings.rs src-tauri/src/retrieval.rs \
       crates/donna-core/src/
git mv src-tauri/src/integrations crates/donna-core/src/integrations
```

- [ ] **Step 2: Declare modules in donna-core and re-export in src-tauri**

```rust
// crates/donna-core/src/lib.rs
pub mod db;
pub mod docs;
pub mod embeddings;
pub mod error;
pub mod integrations;
pub mod knowledge;
pub mod oauth;
pub mod providers;
pub mod retrieval;
pub mod secrets;
```

In `src-tauri/src/lib.rs`, delete the moved `mod` declarations and add re-exports so `crate::db::...` paths in `commands.rs` keep resolving without edits:

```rust
pub use donna_core::{db, docs, embeddings, error, integrations, knowledge, oauth, providers, retrieval, secrets};
```

Add to `src-tauri/Cargo.toml` dependencies: `donna-core = { path = "../crates/donna-core" }`.

- [ ] **Step 3: Delete dead spawn_reindex and its tauri import**

In `crates/donna-core/src/embeddings.rs`, delete the whole `spawn_reindex` function (it has zero callers — spec §10) and any now-unused imports. `grep -rn "spawn_reindex" .` must return nothing.

- [ ] **Step 4: Fix intra-crate paths**

Inside moved files, `crate::` paths still resolve within donna-core (they only reference each other — verified: no references to commands/scheduler). Run `cargo check --workspace`; fix any leftover `crate::X` that pointed at a non-moved module by the compiler's guidance. Expected end state: clean check.

- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Extract portable modules from src-tauri into donna-core

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 3: SecretStore trait with keychain and file backends

`secrets.rs` today calls `keyring` directly. The server has no keychain. Keep the module-level API (so integrations don't change) but back it with a swappable store.

**Files:**
- Modify: `crates/donna-core/src/secrets.rs`
- Test: inline `#[cfg(test)] mod tests` in the same file

**Interfaces:**
- Produces: `secrets::init(store: Box<dyn SecretStore>)`, `trait SecretStore: Send + Sync { fn get(&self, key: &str) -> Result<Option<String>>; fn set(&self, key: &str, value: &str) -> Result<()>; fn delete(&self, key: &str) -> Result<()>; }`, `KeychainStore::new()`, `FileStore::new(path: PathBuf)`. Existing pub functions (get/set/delete wrappers) keep their signatures, delegating to the initialized store; if `init` was never called they default to `KeychainStore` (desktop behavior unchanged).
- Consumes: `donna_core::error::Result`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_roundtrip() {
        let dir = std::env::temp_dir().join(format!("donna-secrets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = FileStore::new(dir.join("secrets.json"));
        assert_eq!(store.get("api_key").unwrap(), None);
        store.set("api_key", "sk-123").unwrap();
        assert_eq!(store.get("api_key").unwrap(), Some("sk-123".into()));
        store.delete("api_key").unwrap();
        assert_eq!(store.get("api_key").unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn file_store_sets_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("donna-perm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secrets.json");
        FileStore::new(path.clone()).set("k", "v").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p donna-core secrets`
Expected: FAIL — `FileStore` not found.

- [ ] **Step 3: Implement**

```rust
use std::path::PathBuf;
use std::sync::OnceLock;
use std::collections::BTreeMap;

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

pub struct KeychainStore;
impl KeychainStore { pub fn new() -> Self { Self } }
// impl SecretStore for KeychainStore: wrap the existing keyring::Entry calls
// currently in this module (service "ai.donna.app"), unchanged.

pub struct FileStore { path: PathBuf, lock: std::sync::Mutex<()> }
impl FileStore {
    pub fn new(path: PathBuf) -> Self { Self { path, lock: std::sync::Mutex::new(()) } }
    fn read_map(&self) -> Result<BTreeMap<String, String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(serde_json::from_str(&s).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }
    fn write_map(&self, map: &BTreeMap<String, String>) -> Result<()> {
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(map)?)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
impl SecretStore for FileStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        let _g = self.lock.lock().unwrap();
        Ok(self.read_map()?.get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        let _g = self.lock.lock().unwrap();
        let mut m = self.read_map()?; m.insert(key.into(), value.into()); self.write_map(&m)
    }
    fn delete(&self, key: &str) -> Result<()> {
        let _g = self.lock.lock().unwrap();
        let mut m = self.read_map()?; m.remove(key); self.write_map(&m)
    }
}

static STORE: OnceLock<Box<dyn SecretStore>> = OnceLock::new();
pub fn init(store: Box<dyn SecretStore>) { let _ = STORE.set(store); }
fn store() -> &'static dyn SecretStore {
    STORE.get_or_init(|| Box::new(KeychainStore::new())).as_ref()
}
```

Rewrite the existing module-level `get_secret`/`set_secret`/`delete_secret`-style functions (keep their exact current names and signatures — read the file first) to call `store().get/set/delete`. Map error types through the existing `error::Error`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p donna-core secrets && cargo check --workspace`
Expected: 2 passed; workspace still compiles (desktop path untouched — default store is keychain).

- [ ] **Step 5: Commit and push**

```bash
git add crates/donna-core/src/secrets.rs
git commit -m "Add SecretStore trait with keychain and 0600-file backends

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 4: OpenAI embeddings backend + retrieval gate fix

Spec §3: embeddings gain an OpenAI backend (`text-embedding-3-small`); semantic recall stops being Ollama-only. Today `retrieval.rs` skips vectors unless `provider == "ollama"`.

**Files:**
- Modify: `crates/donna-core/src/embeddings.rs`, `crates/donna-core/src/retrieval.rs`
- Test: inline tests in `embeddings.rs`

**Interfaces:**
- Produces: `embeddings::embed(db: &Db, text: &str) -> Result<Vec<f32>>` routes by settings: provider `openai` → POST `https://api.openai.com/v1/embeddings` with model from `embed_model` setting (default `text-embedding-3-small` when provider is openai); provider `ollama` → existing path. `embeddings::backend_available(db: &Db) -> bool`.
- Consumes: `secrets` (OpenAI API key, same key the chat provider uses), `db` settings.

- [ ] **Step 1: Write the failing test** (pure logic only — no network in tests)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn openai_request_body_shape() {
        let body = openai_embed_body("hello", "text-embedding-3-small");
        assert_eq!(body["model"], "text-embedding-3-small");
        assert_eq!(body["input"], "hello");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p donna-core embeddings`
Expected: FAIL — `openai_embed_body` not defined.

- [ ] **Step 3: Implement**

Extract a pure `fn openai_embed_body(input: &str, model: &str) -> serde_json::Value`, then an async `openai_embed(api_key, model, input) -> Result<Vec<f32>>` (parse `data[0].embedding`). In the existing embed entry point, match on the `provider` setting: `"openai"` → openai path with `embed_model` defaulting to `text-embedding-3-small`; `"ollama"` → existing path; anything else → `Err(UnsupportedProvider)`. Add `pub fn backend_available(db: &Db) -> bool` returning true for ollama (existing behavior) or openai-with-key. In `retrieval.rs`, replace the `provider == "ollama"` guard with `embeddings::backend_available(db)`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p donna-core && cargo check --workspace`
Expected: PASS.

- [ ] **Step 5: Commit and push**

```bash
git add crates/donna-core/src/embeddings.rs crates/donna-core/src/retrieval.rs
git commit -m "Add OpenAI embeddings backend; gate retrieval on backend availability

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 5: Extract command bodies into donna-core::ops

`commands.rs` holds ~80 `#[tauri::command]` fns containing the real logic, entangled with `tauri::State<Db>` and `Channel<ChatEvent>`. Move the bodies to `donna_core::ops` as plain functions over `&Db`; leave thin Tauri wrappers.

**Files:**
- Create: `crates/donna-core/src/ops.rs` (one file, same internal ordering as commands.rs — ponytail: don't invent a module taxonomy during a move)
- Modify: `crates/donna-core/src/lib.rs` (`pub mod ops;`), `src-tauri/src/commands.rs`
- Test: `crates/donna-core/src/ops.rs` inline test for one representative op

**Interfaces:**
- Produces: for every Tauri command `foo(state: State<Db>, args...) -> Result<T>`, a core fn `ops::foo(db: &Db, args...) -> Result<T>` with identical name, args, and return type. Streaming commands take a callback instead of a Channel: `ops::send_chat(db: &Db, conversation_id: &str, content: &str, on_event: &(dyn Fn(ChatEvent) + Send + Sync)) -> Result<()>` and `ops::quick_chat_send(db: &Db, prompt: String, context: QuickChatContext, on_event: &(dyn Fn(ChatEvent) + Send + Sync)) -> Result<()>` (match the real current signatures when reading the file — the callback replaces only the `Channel` parameter). `ChatEvent` moves to `ops.rs` (or stays where it lives if already in a moved module).
- Consumes: everything from Tasks 2–4.

- [ ] **Step 1: Transform pattern — apply to every command**

Before (in `src-tauri/src/commands.rs`):

```rust
#[tauri::command]
pub async fn list_notifications(db: tauri::State<'_, Db>) -> Result<Vec<Notification>, String> {
    db.list_notifications().map_err(|e| e.to_string())
}
```

After — body moves to `crates/donna-core/src/ops.rs`:

```rust
pub fn list_notifications(db: &Db) -> Result<Vec<Notification>> {
    db.list_notifications()
}
```

Thin wrapper stays in `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub async fn list_notifications(db: tauri::State<'_, Db>) -> Result<Vec<Notification>, String> {
    donna_core::ops::list_notifications(&db).map_err(|e| e.to_string())
}
```

Streaming command pattern — `send_chat` today sends into a `Channel<ChatEvent>`; the core version takes the callback, the wrapper adapts:

```rust
// src-tauri wrapper
#[tauri::command]
pub async fn send_chat(db: tauri::State<'_, Db>, conversation_id: String, content: String,
                       channel: tauri::ipc::Channel<ChatEvent>) -> Result<(), String> {
    donna_core::ops::send_chat(&db, &conversation_id, &content, &move |ev| { let _ = channel.send(ev); })
        .await.map_err(|e| e.to_string())
}
```

Rules: error type in core is `donna_core::error::Result`; `.map_err(|e| e.to_string())` lives only in wrappers. String-vs-&str and ownership: keep whatever the current signature does, minimally adjusted. Commands that are purely native (see Task 8 list: `quick_chat_context`, `project_open_in_editor`, `project_list_files`, `project_read_file`, `project_write_file`, `project_status_report`) do NOT move — they stay whole in commands.rs.

- [ ] **Step 2: Apply to all commands, in file order, compiling as you go**

Work top-to-bottom through commands.rs in chunks of ~10 commands; after each chunk run `cargo check --workspace`. Full command inventory to transform (from `lib.rs` generate_handler, minus native-only): config (`get_config`, `save_config`, `basics_status`), secrets (`set_api_key`, `has_api_key`, `delete_api_key`), models (`list_models`), chat (`create_conversation`, `list_conversations`, `rename_conversation`, `delete_conversation`, `get_messages`, `add_message`, `send_chat`, `maybe_generate_title` if a command), knowledge graph (`kg_graph`, `kg_extract`, `kg_reset`, `kg_save_node`, `kg_delete_node`, `kg_node_image`, `kg_set_node_image`, `kg_remove_node_image`, `kg_reindex_embeddings`), integrations (`integrations_status`, `google_set_client`, `google_connect`*, `google_disconnect`, `calendar_list_events`, `calendar_create_event`, `calendar_update_event`, `calendar_delete_event`, `gmail_list_messages`, `gmail_create_draft`, `google_create_doc`, `drive_list_files`, `slack_set_token`, `slack_disconnect`, `slack_list_channels`, `slack_send_message`, `fathom_set_key`, `fathom_disconnect`, `fathom_process_recent_meeting`, `github_set_token`, `github_disconnect`, `github_list_repos`, `github_list_issues`, `linear_set_key`, `linear_disconnect`, `linear_list_issues`, `notion_set_token`, `notion_disconnect`, `notion_search_pages`, `telegram_set_credentials`, `telegram_disconnect`, `telegram_send_message`, `whatsapp_set_credentials`, `whatsapp_disconnect`, `whatsapp_send_message`, `discord_set_token`, `discord_disconnect`), routines (`list_routines`, `toggle_routine`, `create_routine`, `delete_routine`), notifications (`list_notifications`, `mark_notification_read`), docs (`list_docs`, `get_doc`, `delete_doc`), projects DB-side (`project_list`, `project_create`, `project_delete`), productivity (`news_fetch_latest`, `news_list_items`, `news_article_summary`, `reading_list_add`, `reading_list_get`, `reading_list_summarize`, `reading_list_delete`, `focus_start`, `focus_end`, `focus_active`, `habit_create`, `habit_list`, `habit_log`, `habit_logged_today`), quick chat (`quick_chat_send` — streaming; `quick_chat_context` stays native). *`google_connect` moves to ops but note: it opens a system browser + loopback listener, so it only works where invoked; on desktop the wrapper calls it as today. (Server never calls it — see Task 8 OAuth flow.) Reconcile this list against the actual `generate_handler![...]` block — the code is truth; adjust names as found, and report any command in code that is missing here.

- [ ] **Step 3: One regression test in core**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    #[test]
    fn conversation_crud_roundtrip() {
        let dir = std::env::temp_dir().join(format!("donna-ops-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("t.sqlite")).unwrap();
        let conv = create_conversation(&db).unwrap();
        assert!(list_conversations(&db).unwrap().iter().any(|c| c.id == conv.id));
        delete_conversation(&db, &conv.id).unwrap();
        assert!(!list_conversations(&db).unwrap().iter().any(|c| c.id == conv.id));
    }
}
```

(Adjust to the real signatures — `create_conversation` may take a title.)

- [ ] **Step 4: Verify**

Run: `cargo test -p donna-core && cargo check --workspace && npm run tauri dev`
Expected: tests pass; app launches; send one chat message end-to-end in the UI (streaming still works through the wrapper).

- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Move command logic into donna-core::ops; Tauri commands become thin wrappers

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 6: Decouple the scheduler from Tauri

**Files:**
- Move: `src-tauri/src/scheduler.rs` → `crates/donna-core/src/scheduler.rs`
- Modify: `crates/donna-core/src/lib.rs`, `src-tauri/src/lib.rs`
- Test: inline in `scheduler.rs`

**Interfaces:**
- Produces: `scheduler::run_loop(db: Arc<Db>, notifier: Arc<dyn Notifier>)` (spawns its own tokio loop); `pub trait Notifier: Send + Sync { fn notify(&self, title: &str, body: &str); }`. Timezone: tick computes "now" via `chrono_tz::Tz` parsed from settings key `timezone` (fallback: `Local`).
- Consumes: `ops`/`db`/`integrations` from earlier tasks.

- [ ] **Step 1: Move file, replace AppHandle**

`git mv src-tauri/src/scheduler.rs crates/donna-core/src/scheduler.rs`; add `pub mod scheduler;` to core lib.rs. In the file: replace `run_loop(app: AppHandle)` with `run_loop(db: Arc<Db>, notifier: Arc<dyn Notifier>)`; replace every `app.state::<Db>()` / `app.try_state::<Db>()` with the `db` argument; replace the `tauri_plugin_notification` call with `notifier.notify(&title, &body)`; replace `tauri::async_runtime::spawn` with `tokio::spawn`. Keep the WhatsApp/Telegram push logic exactly where it is (it's plain reqwest via integrations).

- [ ] **Step 2: Timezone fix (spec §7.3)**

Where the tick derives the current hour/minute/weekday from `chrono::Local`, first read setting `timezone`; if it parses as a `chrono_tz::Tz`, use `Utc::now().with_timezone(&tz)`; else keep `Local::now()`. Extract the schedule matching into a pure function and test it:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_routine_due_at_exact_time() {
        // construct a routine due 08:00 daily, last_run None,
        // now = 2026-07-07T08:00:30 → is_due == true; at 07:59 → false;
        // same day after a run at 08:00 → false (dedupe via last_run slot)
    }
}
```

Fill the test body against the real `DueRoutine`/matching code found in the file — assert the three cases above with concrete `chrono` timestamps.

- [ ] **Step 3: Desktop keeps compiling but stops scheduling**

In `src-tauri/src/lib.rs`, delete the `scheduler::run_loop(...)` call and the notification-plugin wiring for it (the plugin stays for other notification display). The desktop no longer runs routines — the server does (Task 9). `// ponytail: one brain, one scheduler.`

- [ ] **Step 4: Outbound send status check (spec §7.1)**

In `crates/donna-core/src/integrations/whatsapp.rs` and `telegram.rs` send functions: after the reqwest call, add `let resp = resp.error_for_status()?;` (or check `.status().is_success()` and return `Err` with the response text). Compile-verify.

- [ ] **Step 5: Run tests, commit, push**

Run: `cargo test -p donna-core scheduler && cargo check --workspace`

```bash
git add -A
git commit -m "Move scheduler to donna-core behind Notifier trait; add timezone setting and send status checks

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 7: donna-server skeleton — axum, config, auth, health

Replaces the old briefing binary entirely.

**Files:**
- Delete: `donna-server/src/{messaging,calendar,news,weather,config}.rs` (superseded by donna-core equivalents)
- Rewrite: `donna-server/src/main.rs`
- Create: `donna-server/src/state.rs`, `donna-server/src/auth.rs`
- Modify: `donna-server/Cargo.toml`
- Test: `donna-server/tests/http.rs`

**Interfaces:**
- Produces: `AppState { db: Arc<Db>, token: String, events: tokio::sync::broadcast::Sender<ServerEvent> }`; `ServerEvent` enum (serde-tagged) with `Notification { title, body }` for now; router with `GET /health` (open) and everything else behind bearer auth.
- Consumes: `donna_core::{db, secrets, scheduler}`.

- [ ] **Step 1: Deps and failing test**

`donna-server/Cargo.toml` additions: `donna-core = { path = "../crates/donna-core" }`, `axum = { version = "0.7", features = ["ws"] }`, `tower = "0.4"`; dev-deps: `http-body-util = "0.1"`, `tower = { version = "0.4", features = ["util"] }`.

```rust
// donna-server/tests/http.rs
use tower::util::ServiceExt;
use axum::{body::Body, http::{Request, StatusCode}};

#[tokio::test]
async fn health_is_open_and_rpc_needs_token() {
    let app = donna_server::build_app(donna_server::test_state());
    let res = app.clone().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let res = app.clone().oneshot(Request::post("/rpc/list_conversations").body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = app.oneshot(Request::post("/rpc/list_conversations")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```

(Requires `[lib]` target: add `donna-server/src/lib.rs` exporting `build_app`, `test_state`; `main.rs` calls into it. `test_state()` uses a temp-dir DB and token `"test-token"`.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p donna-server`
Expected: FAIL — `build_app` undefined.

- [ ] **Step 3: Implement skeleton**

```rust
// donna-server/src/lib.rs
pub mod auth; pub mod rpc; pub mod state; pub mod ws;
use axum::{routing::{get, post}, Router};
pub use state::{AppState, test_state};

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/rpc/:command", post(rpc::handle))
        .route("/ws", get(ws::handle))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::require_bearer))
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
}
```

```rust
// donna-server/src/state.rs
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

pub fn test_state() -> AppState {
    let dir = std::env::temp_dir().join(format!("donna-server-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    AppState {
        db: Arc::new(Db::open(&dir.join("t.sqlite")).unwrap()),
        token: "test-token".into(),
        events: tokio::sync::broadcast::channel(64).0,
    }
}
```

```rust
// donna-server/src/auth.rs
use axum::{extract::State, http::{Request, StatusCode}, middleware::Next, response::Response};
use crate::state::AppState;

pub async fn require_bearer(State(st): State<AppState>, req: Request<axum::body::Body>, next: Next)
    -> Result<Response, StatusCode> {
    let ok = req.headers().get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == format!("Bearer {}", st.token))
        .unwrap_or(false)
        // WS can't set headers from the browser: accept ?token= on /ws only.
        || (req.uri().path() == "/ws"
            && req.uri().query().unwrap_or("").contains(&format!("token={}", st.token)));
    if ok { Ok(next.run(req).await) } else { Err(StatusCode::UNAUTHORIZED) }
}
```

```rust
// donna-server/src/main.rs
use donna_server::{build_app, AppState};
use donna_core::{db::Db, secrets};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let data_dir = std::path::PathBuf::from(std::env::var("DONNA_DATA_DIR").unwrap_or("./donna-data".into()));
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    std::env::set_var("DONNA_KB_DIR", data_dir.join("knowledge-base"));
    let _ = donna_core::knowledge::ensure_root();
    secrets::init(Box::new(secrets::FileStore::new(data_dir.join("secrets.json"))));
    let token = std::env::var("DONNA_TOKEN").expect("DONNA_TOKEN is required");
    let port: u16 = std::env::var("DONNA_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8377);
    let db = Arc::new(Db::open(&data_dir.join("donna.sqlite")).expect("open db"));
    let (events, _) = tokio::sync::broadcast::channel(256);
    let state = AppState { db, token, events };
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    println!("donna-server listening on :{port}");
    axum::serve(listener, build_app(state)).await.unwrap();
}
```

Stub `rpc::handle` (returns `{"ok":true}` for now) and `ws::handle` (accepts and closes) so it compiles; Tasks 8–9 fill them. Delete the five old module files and their references.

- [ ] **Step 4: Run tests**

Run: `cargo test -p donna-server`
Expected: PASS.

- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Rebuild donna-server as axum app: config, bearer auth, health, state

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 8: RPC dispatcher over donna-core::ops

**Files:**
- Create: `donna-server/src/rpc.rs` (replace stub)
- Test: extend `donna-server/tests/http.rs`

**Interfaces:**
- Produces: `POST /rpc/:command` — body is the same JSON args object the UI passed to `invoke(cmd, args)` (camelCase keys exactly as Tauri received them; Tauri converts camelCase→snake_case for Rust args, so the dispatcher must accept camelCase keys and map to the ops fn parameters). Response: ops result as JSON, or 400 `{"error": msg}`, or 404 for unknown command.
- Consumes: every `ops::` fn from Task 5.

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn rpc_conversation_roundtrip() {
    let app = donna_server::build_app(donna_server::test_state());
    let res = app.clone().oneshot(Request::post("/rpc/create_conversation")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let conv: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(conv["id"].is_string());
    let res = app.oneshot(Request::post("/rpc/nonexistent_command")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p donna-server rpc` → FAIL.

- [ ] **Step 3: Implement with a dispatch macro**

```rust
// donna-server/src/rpc.rs
use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde_json::{json, Value};
use crate::state::AppState;
use donna_core::ops;

fn arg<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(v.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("bad arg {key}: {e}"))
}

macro_rules! ok { ($e:expr) => { serde_json::to_value($e.map_err(|x| x.to_string())?).map_err(|x| x.to_string()) } }

pub async fn handle(State(st): State<AppState>, Path(cmd): Path<String>, Json(a): Json<Value>)
    -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let db = &st.db;
    let out: Result<Value, String> = match cmd.as_str() {
        // config
        "get_config" => ok!(ops::get_config(db)),
        "save_config" => ok!(ops::save_config(db, arg(&a, "config").map_err(es)?)),
        // chat
        "create_conversation" => ok!(ops::create_conversation(db)),
        "list_conversations" => ok!(ops::list_conversations(db)),
        "rename_conversation" => ok!(ops::rename_conversation(db, &arg::<String>(&a, "id").map_err(es)?, &arg::<String>(&a, "title").map_err(es)?)),
        // ... one arm per command in the Task 5 inventory, same pattern.
        // Async ops: append .await inside ok!() — e.g.
        // "calendar_list_events" => ok!(ops::calendar_list_events(db, arg(&a, "start")?, arg(&a, "end")?).await),
        _ => return Err((StatusCode::NOT_FOUND, Json(json!({"error": format!("unknown command {cmd}")})))),
    };
    match out {
        Ok(v) => Ok(Json(v)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"error": e})))),
    }
}
fn es(e: String) -> String { e }
```

Every command from the Task 5 inventory gets an arm (streaming `send_chat`/`quick_chat_send` are NOT here — they're WS-only, Task 9; `google_connect` is desktop-native and gets no arm). Arg key names: match what api.ts passes today — read each api.ts call site; Tauri camelCases (`conversationId`), so use those exact keys in `arg(&a, "conversationId")`. Work through api.ts top-to-bottom to enumerate; compile after every ~15 arms.

- [ ] **Step 4: Run tests** — `cargo test -p donna-server` → PASS.

- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add /rpc dispatcher mapping the full Tauri command surface to donna-core ops

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 9: WebSocket — chat streaming + notification push; scheduler on the server

**Files:**
- Create: `donna-server/src/ws.rs` (replace stub)
- Modify: `donna-server/src/main.rs` (start scheduler), `donna-server/src/state.rs` (Notifier impl)
- Test: `donna-server/tests/ws.rs`

**Interfaces:**
- Produces: `GET /ws?token=<token>` upgrade. Client→server frames: `{"type":"chat","id":"<client-generated>","cmd":"send_chat"|"quick_chat_send","payload":{...same args as invoke...}}`. Server→client: `{"type":"chat_event","id":"...","event":<ChatEvent as JSON: Token|Done|Error>}` and broadcast `{"type":"notification","title":"...","body":"..."}`.
- Consumes: `ops::send_chat` / `ops::quick_chat_send` callbacks (Task 5), `AppState.events` (Task 7), `scheduler::run_loop` + `Notifier` (Task 6).

- [ ] **Step 1: Failing test**

```rust
// donna-server/tests/ws.rs
use futures_util::{SinkExt, StreamExt};

async fn spawn_server() -> (String, donna_server::AppState) {
    let state = donna_server::test_state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = donna_server::build_app(state.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("127.0.0.1:{}", addr.port()), state)
}

#[tokio::test]
async fn ws_rejects_bad_token() {
    let (addr, _st) = spawn_server().await;
    let err = tokio_tungstenite::connect_async(format!("ws://{addr}/ws?token=wrong")).await;
    assert!(err.is_err()); // 401 on upgrade
}

#[tokio::test]
async fn ws_chat_yields_events() {
    let (addr, _st) = spawn_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(
        format!("ws://{addr}/ws?token=test-token")).await.unwrap();
    // No provider key configured: the stream must still answer with an Error
    // chat_event carrying our id — that proves the full WS plumbing.
    ws.send(tokio_tungstenite::tungstenite::Message::text(
        r#"{"type":"chat","id":"t1","cmd":"send_chat","payload":{"conversationId":"nonexistent","content":"hi"}}"#
    )).await.unwrap();
    let msg = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next())
        .await.expect("no frame within 10s").unwrap().unwrap();
    let frame: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(frame["type"], "chat_event");
    assert_eq!(frame["id"], "t1");
}
```

Add dev-dep `tokio-tungstenite = "0.23"` and `futures-util` (workspace) to donna-server.

- [ ] **Step 2: Run to verify failure** — `cargo test -p donna-server ws` → FAIL.

- [ ] **Step 3: Implement**

`ws::handle`: axum `WebSocketUpgrade` → split socket; spawn a task forwarding `state.events.subscribe()` broadcasts as `notification` frames; loop on incoming frames: parse the chat frame, spawn `ops::send_chat(db, ..., &|ev| { tx.send(chat_event_frame(id, ev)) })` (bridge the sync callback to the socket with an `mpsc::unbounded_channel`). In `state.rs`, `impl donna_core::scheduler::Notifier for AppState` — `notify()` does `let _ = self.events.send(ServerEvent::Notification{..})`. In `main.rs` after building state: `donna_core::scheduler::run_loop(state.db.clone(), Arc::new(state.clone()))`.

- [ ] **Step 4: Run tests** — `cargo test -p donna-server` → PASS (all three test files).

- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add WebSocket chat streaming and notification push; scheduler runs on server

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 10: Desktop becomes a client — api.ts swap

**Files:**
- Create: `src/lib/server.ts` (connection config + fetch/WS plumbing)
- Modify: `src/lib/api.ts` (invoke wrapper + 2 Channel call sites at lines ~516 and ~868), `src/routes/Settings.tsx` (server URL + token fields), `src/App.tsx` (unreachable banner)
- Modify: `src-tauri/src/lib.rs` (trim `generate_handler![]` to native-only commands), `src-tauri/src/commands.rs` (delete moved wrappers)

**Interfaces:**
- Consumes: `/rpc/:command`, `/ws` from Tasks 8–9.
- Produces: `server.ts` exports `rpc<T>(cmd, args)`, `chatStream(cmd, payload, onEvent)`, `serverConfig()/setServerConfig({url, token})` (localStorage keys `donna.serverUrl`, `donna.serverToken`), `onServerEvent(cb)` (notification frames), `serverReachable(): Promise<boolean>`.

- [ ] **Step 1: Write server.ts**

```ts
// src/lib/server.ts
export interface ServerConfig { url: string; token: string }
export function serverConfig(): ServerConfig {
  return {
    url: localStorage.getItem("donna.serverUrl") ?? "http://localhost:8377",
    token: localStorage.getItem("donna.serverToken") ?? "",
  };
}
export function setServerConfig(c: ServerConfig) {
  localStorage.setItem("donna.serverUrl", c.url);
  localStorage.setItem("donna.serverToken", c.token);
}

export async function rpc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { url, token } = serverConfig();
  const res = await fetch(`${url}/rpc/${cmd}`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error ?? `rpc ${cmd} failed (${res.status})`);
  }
  return res.json();
}

export async function serverReachable(): Promise<boolean> {
  try {
    const res = await fetch(`${serverConfig().url}/health`, { signal: AbortSignal.timeout(3000) });
    return res.ok;
  } catch { return false; }
}

type Frame = { type: string; id?: string; event?: unknown; title?: string; body?: string };
let socket: WebSocket | null = null;
const chatHandlers = new Map<string, (ev: unknown) => void>();
const eventHandlers = new Set<(f: Frame) => void>();

function ensureSocket(): WebSocket {
  if (socket && socket.readyState <= WebSocket.OPEN) return socket;
  const { url, token } = serverConfig();
  socket = new WebSocket(`${url.replace(/^http/, "ws")}/ws?token=${encodeURIComponent(token)}`);
  socket.onmessage = (m) => {
    const f: Frame = JSON.parse(m.data);
    if (f.type === "chat_event" && f.id) chatHandlers.get(f.id)?.(f.event);
    else eventHandlers.forEach((h) => h(f));
  };
  socket.onclose = () => { socket = null; };
  return socket;
}

export function chatStream(cmd: "send_chat" | "quick_chat_send",
                           payload: Record<string, unknown>,
                           onEvent: (ev: unknown) => void): void {
  const id = crypto.randomUUID();
  chatHandlers.set(id, (ev) => {
    onEvent(ev);
    const e = ev as { type?: string; event?: string };
    if (e?.type === "done" || e?.type === "error" || e?.event === "done" || e?.event === "error")
      chatHandlers.delete(id);
  });
  const ws = ensureSocket();
  const frame = JSON.stringify({ type: "chat", id, cmd, payload });
  if (ws.readyState === WebSocket.OPEN) ws.send(frame);
  else ws.addEventListener("open", () => ws.send(frame), { once: true });
}

export function onServerEvent(cb: (f: Frame) => void): () => void {
  eventHandlers.add(cb); ensureSocket();
  return () => eventHandlers.delete(cb);
}
```

(Match the real `ChatEvent` JSON tag names when wiring `chatHandlers` cleanup — check how serde serializes `ChatEvent` in Rust and mirror it.)

- [ ] **Step 2: Swap the invoke wrapper**

In `src/lib/api.ts`:

```ts
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { rpc, chatStream } from "./server";

const NATIVE_COMMANDS = new Set([
  "quick_chat_context", "google_connect",
  "project_open_in_editor", "project_list_files", "project_read_file",
  "project_write_file", "project_status_report",
]);

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (NATIVE_COMMANDS.has(cmd)) { ensureDesktopApp(); return tauriInvoke<T>(cmd, args); }
  return rpc<T>(cmd, args);
}
```

Replace the two `new Channel<ChatEvent>()` call sites (`sendChat` ~line 516, quick chat ~line 868) with `chatStream("send_chat", { conversationId, content }, onEvent)` — preserving each function's exact external signature so callers don't change. Remove the now-unused `Channel` import.

- [ ] **Step 3: OAuth token push (google_connect stays native)**

`google_connect` runs its browser+loopback flow on the desktop and stores tokens in the desktop keychain. After it resolves, the connect button's handler in `src/routes/Integrations.tsx` additionally calls two new native commands→RPC pushes: add native command `export_google_secrets` (in commands.rs; returns `{ client: String, token: String }` read from keychain keys `google_client` and `oauth:google`) and then `await rpc("import_google_secrets", { client, token })` — a new ops fn that writes both into the server's secret store. Add the `import_google_secrets` arm to rpc.rs and ops.rs (2 `secrets::set` calls). All server-side Google calls then refresh tokens server-side as they already do in `integrations/google.rs`.

- [ ] **Step 4: Settings + banner**

`Settings.tsx`: add a "Server" card with URL + token inputs bound to `serverConfig()/setServerConfig`, a "Test connection" button calling `serverReachable()` showing ok/fail. `App.tsx`: on mount and every 30s, `serverReachable()`; when false render a slim banner over the layout: "Donna is unreachable — check the server. [Retry]". Notifications: subscribe `onServerEvent`, and for `notification` frames call the existing notification display path (whatever the current in-app notification bell uses — refresh its list) — native OS toast display can reuse `tauri-plugin-notification` from JS if the plugin's JS API is already installed, else skip OS toasts this phase.

- [ ] **Step 5: Trim src-tauri**

`generate_handler![]` shrinks to the NATIVE_COMMANDS list + `export_google_secrets`. Delete all other wrapper fns from commands.rs (their logic already lives in ops). Delete DB setup from `src-tauri/src/lib.rs` setup hook EXCEPT what native commands need — `project_status_report` needs a provider call and project path: it should now take the path as an argument from the UI (fetched via `rpc("project_list")`) instead of reading the DB; adjust its signature accordingly. The desktop app no longer opens donna.sqlite at all.

- [ ] **Step 6: Full manual smoke test**

Run: terminal 1: `DONNA_TOKEN=dev cargo run -p donna-server`; terminal 2: `npm run tauri dev`. In Settings set URL `http://localhost:8377`, token `dev`. Verify: chat streams; conversations list; kg mind map loads; integrations page renders statuses; routines list; a notification appears when a routine fires (create a custom routine 1 minute in the future); Cmd+D quick chat answers; kill the server → banner appears; restart → banner clears.

- [ ] **Step 7: Commit and push**

```bash
git add -A
git commit -m "Desktop becomes a server client: RPC/WS api layer, server settings, unreachable banner

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 11: Export bundle + server import

**Files:**
- Modify: `src-tauri/src/commands.rs` (new native command `export_server_bundle`), `src-tauri/src/lib.rs` (register), `src/routes/Settings.tsx` (button), `src/lib/api.ts` (add to NATIVE_COMMANDS + api fn)
- Modify: `donna-server/src/main.rs` (import subcommand)
- Create: `crates/donna-core/src/bundle.rs` (+ `pub mod bundle;`)
- Test: inline in `bundle.rs`

**Interfaces:**
- Produces: `bundle::write_bundle(out: &Path, db_path: &Path, kb_dir: &Path, secrets: &BTreeMap<String,String>) -> Result<()>` (tar.gz: `donna.sqlite`, `knowledge-base/**`, `secrets.json`); `bundle::import_bundle(bundle: &Path, data_dir: &Path) -> Result<()>` (unpacks to the same names). Desktop command `export_server_bundle(dest_dir: String) -> Result<String>` returns the written file path; secrets gathered from the keychain by enumerating the known key list (read `secrets.rs`/integrations for every key name used: provider API keys, `google_client`, `oauth:google`, slack/github/linear/notion/fathom/telegram/whatsapp/discord tokens). Server CLI: `donna-server import <bundle.tar.gz>` (runs before serve; refuses if `donna.sqlite` already exists in DONNA_DATA_DIR — print "data dir not empty, aborting").
- Consumes: `SecretStore` (Task 3).

Deps: add `tar = "0.4"`, `flate2 = "1"` to workspace + donna-core.

- [ ] **Step 1: Failing test** — roundtrip: create temp db file + kb dir with one md file + a secrets map → `write_bundle` → `import_bundle` into a second temp dir → assert all three artifacts exist with identical content.

```rust
#[test]
fn bundle_roundtrip() {
    let src = tempdir("bundle-src"); let dst = tempdir("bundle-dst");
    std::fs::write(src.join("donna.sqlite"), b"fake-db").unwrap();
    std::fs::create_dir_all(src.join("kb/People")).unwrap();
    std::fs::write(src.join("kb/People/alex.md"), b"# Alex").unwrap();
    let secrets = BTreeMap::from([("api_key:openai".to_string(), "sk-1".to_string())]);
    let out = src.join("bundle.tar.gz");
    write_bundle(&out, &src.join("donna.sqlite"), &src.join("kb"), &secrets).unwrap();
    import_bundle(&out, &dst).unwrap();
    assert_eq!(std::fs::read(dst.join("donna.sqlite")).unwrap(), b"fake-db");
    assert_eq!(std::fs::read(dst.join("knowledge-base/People/alex.md")).unwrap(), b"# Alex");
    let m: BTreeMap<String,String> = serde_json::from_str(&std::fs::read_to_string(dst.join("secrets.json")).unwrap()).unwrap();
    assert_eq!(m["api_key:openai"], "sk-1");
}
```

(`tempdir` = the same temp-dir helper pattern used in earlier tests; write it as a small fn in the test module.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p donna-core bundle` → FAIL.
- [ ] **Step 3: Implement** with `tar::Builder` over `flate2::write::GzEncoder` and the reverse with `flate2::read::GzDecoder` + `tar::Archive::unpack`, then rename `kb` entry prefix to `knowledge-base` on write (store it under that name directly). Then wire the desktop command + Settings button ("Export server bundle…" → dialog choose folder via existing `tauri-plugin-dialog` → toast the path) and the server `import` subcommand (plain `std::env::args()` check — no clap. `// ponytail: two subcommands don't need a CLI framework`).
- [ ] **Step 4: Run tests** — `cargo test -p donna-core bundle && cargo check --workspace` → PASS.
- [ ] **Step 5: Manual verify** — export from the running desktop app, `donna-server import <file>` into a fresh dir, start server against it, desktop connects and sees old conversations.
- [ ] **Step 6: Commit and push**

```bash
git add -A
git commit -m "Add export/import bundle for migrating desktop data to donna-server

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 12: Docker, compose, tunnel, docs

**Files:**
- Rewrite: `donna-server/Dockerfile` (workspace-aware), `donna-server/README.md`
- Create: `donna-server/docker-compose.yml`, `donna-server/.env.example` (rewrite)
- Modify: `docs/ROADMAP.md`, `CONTEXT.md`, `README.md` (root)

- [ ] **Step 1: Dockerfile**

```dockerfile
FROM rust:1.77-slim-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY donna-server ./donna-server
COPY src-tauri/Cargo.toml ./src-tauri/Cargo.toml
RUN mkdir -p src-tauri/src && echo "pub fn run() {}" > src-tauri/src/lib.rs \
 && echo "fn main() {}" > src-tauri/src/main.rs
RUN cargo build --release -p donna-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/donna-server /usr/local/bin/
VOLUME /data
ENV DONNA_DATA_DIR=/data
EXPOSE 8377
CMD ["donna-server"]
```

(src-tauri is stubbed because the workspace manifest lists it but its real build needs Tauri toolchain; if `cargo build -p donna-server` complains about the stub's missing tauri deps, exclude instead: build with `--manifest-path donna-server/Cargo.toml` outside the workspace or add `[patch]`-free fallback — first try the stub, it usually suffices since only the built package's deps resolve. Verify empirically and keep whichever works.)

- [ ] **Step 2: docker-compose.yml**

```yaml
services:
  donna:
    build: { context: .., dockerfile: donna-server/Dockerfile }
    restart: unless-stopped
    env_file: .env
    volumes: [ "donna-data:/data" ]
    ports: [ "8377:8377" ]
  tunnel:
    image: cloudflare/cloudflared:latest
    restart: unless-stopped
    command: tunnel --no-autoupdate run
    environment: [ "TUNNEL_TOKEN=${TUNNEL_TOKEN}" ]
volumes:
  donna-data:
```

`.env.example`: `DONNA_TOKEN=change-me`, `DONNA_PORT=8377`, `TUNNEL_TOKEN=` (with a comment: create a tunnel in the Cloudflare Zero Trust dashboard pointing to `http://donna:8377`; tunnel is optional until Phase 3's webhook).

- [ ] **Step 3: Rewrite donna-server/README.md** — new role (Donna's brain: API, scheduler, data), quickstart (`docker compose up -d` after `.env`), migration (`export bundle from desktop Settings → scp → docker compose run donna import /data/bundle.tar.gz`), desktop connection (Settings → Server URL https://your-tunnel-host, token). DELETE the fictional "Settings > Export Google Token" instructions (spec §10). Include a post-migration note: if switching embeddings from Ollama to OpenAI, run "Reindex embeddings" from the Mind Map view (existing `kg_reindex_embeddings`) once so stored vectors match the new model.

- [ ] **Step 4: Docs sync (spec §10)** — `docs/ROADMAP.md`: add "Phase 7 — Server-first foundation" checked items for this phase and unchecked for Phases 2–6 from the spec (link the spec). Root `README.md`: fix the WhatsApp row ("outbound now, two-way in progress — see spec"), add one paragraph on the server-first architecture + link. `CONTEXT.md`: add a short "§14 Server-first evolution (2026-07)" section stating the desktop is now a client of donna-server and linking the spec (do not rewrite the historical sections).

- [ ] **Step 5: Verify** — `docker build -f donna-server/Dockerfile .` succeeds locally (if Docker unavailable, note it in the commit body and verify on the VPS later); `cargo test --workspace` green.

- [ ] **Step 6: Commit and push**

```bash
git add -A
git commit -m "Docker compose + Cloudflare tunnel deploy; rewrite server README; sync docs to server-first

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

## Done criteria (whole phase)

1. `cargo test --workspace` green; `npm run tauri dev` against a local `DONNA_TOKEN=dev cargo run -p donna-server` passes the Task 10 Step 6 smoke list.
2. Desktop contains no DB access, no scheduler, no integration logic — only native commands (quick chat context, project files/editor, OAuth browser flow, bundle export).
3. A bundle exported from the desktop imports cleanly into a fresh server data dir.
4. `docker compose up` serves `/health`; the tunnel exposes it publicly when TUNNEL_TOKEN is set.
5. Docs no longer contradict reality (WhatsApp row, export-token fiction, roadmap phases).
