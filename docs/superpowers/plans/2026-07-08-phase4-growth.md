# Phase 4: Growth — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Donna learns. Capped `USER.md`/`MEMORY.md` she curates about you, full-text search over your whole chat history, an events log, a background-review pass that refreshes memory and spots recurring asks, and a consent-first suggestion queue — plus routines that stay quiet unless something matters (`[SILENT]`).

**Architecture:** Two capped markdown files at the KB root, injected into every system prompt and edited only through a `memory_update` tool that errors when full (forced consolidation, Hermes-style). An FTS5 virtual table over `messages` exposed as a `session_search` tool. An `events` table logged at the three in-tree choke points (chat request, tool call, approval). A `suggestions` table (consent-first, approvals-shaped) filled by a background-review pass (cheap model) that runs nightly and after sessions; the user accepts/dismisses from a Dashboard card, dismissals latched by dedup key. Routines gain a `[SILENT]` sentinel so a scheduled check that finds nothing says nothing.

**Tech Stack:** existing only — Rust (rusqlite bundled = FTS5, no new dep), axum RPC, React/TS, providers::complete for the cheap review model.

## Global Constraints

- Spec §5 (memory & pattern learning) + the Hermes patterns. Phase 4 only — no third-party skill installation (Phase 6).
- **Commit AND push after every task.** Branch `feat/phase-5-projects-discord-proactive`, no PRs.
- Memory caps (chars, Hermes-scaled): `USER.md` **1500 chars**, `MEMORY.md` **2500 chars**. `memory_update` with action `add` that would exceed the cap returns `Err(Error::Provider("MEMORY_FULL: <file> is at capacity (<n>/<cap> chars). Consolidate or remove entries before adding. Current contents:\n<contents>"))` — the model sees the contents and must consolidate. `replace`/`remove` never blocked.
- Both files live at `knowledge::kb_root()` as `USER.md` / `MEMORY.md`; `scan()` already skips top-level `*.md` so they never pollute the mind-map graph.
- FTS5 table is contentless-external (`content='messages', content_rowid='id'`) with insert/delete/update triggers; a one-time backfill guarded by settings flag `fts_backfilled` (migrate() runs every open — the rebuild must NOT).
- FTS5 MATCH takes user/model query strings — sanitize: wrap the query as a single quoted FTS string (`"` doubled) so punctuation can't inject FTS operators. `// ponytail: quoted-phrase match is enough; add column filters if search gets fussy.`
- Events: `events(id, kind, conversation_id, tool, payload_json, created_at)`; kinds `user_request` | `tool_call` | `approval`. Logged best-effort (`let _ =` — a logging failure never breaks a request).
- Suggestions: approvals-shaped `suggestions(id, kind, title, body, payload_json, dedup_key, status DEFAULT 'pending' CHECK IN ('pending','accepted','dismissed'), created_at, resolved_at)`; a dismissed `dedup_key` is never re-surfaced (insert skips when a non-pending row with that key exists).
- Cheap review model: setting `review_model`; when unset/empty fall back to `config.model`. Background review uses `providers::complete` (provider-agnostic) — NOT the OpenAI-only agent loop.
- Background review is idempotent and quiet: it only writes memory/files suggestions when there's something new; producing nothing is the normal case.
- New DB tables/vtables only (migrate() is CREATE IF NOT EXISTS, no versioning) — never ALTER existing tables.
- Reconcile, don't replace: keep `kg_extract` (mind-map KB nodes) and the `basics_*` onboarding heuristics as they are; Phase 4 ADDS USER.md/MEMORY.md + the background review on top.

---

### Task 1: Memory files + memory_update tool + prompt injection

**Files:**
- Modify: `crates/donna-core/src/knowledge.rs` (read/write/cap helpers), `crates/donna-core/src/ops.rs` (memory_update op + prompt injection), `crates/donna-core/src/tools.rs` (register tool)
- Test: inline in knowledge.rs + tools.rs

**Interfaces:**
- knowledge.rs:
  - `pub const USER_MD_CAP: usize = 1500; pub const MEMORY_MD_CAP: usize = 2500;`
  - `pub fn read_memory_file(which: MemoryFile) -> Result<String>` (empty string if absent), `MemoryFile { User, Memory }` (`fn filename()` → "USER.md"/"MEMORY.md", `fn cap()`).
  - `pub fn memory_prompt_section() -> Result<String>` — emits `## About you\n<USER.md>\n\n## Working memory\n<MEMORY.md>` (omit a file's block when empty; whole thing empty → "").
  - `pub fn apply_memory_update(which: MemoryFile, action: MemoryAction, text: &str) -> Result<String>` — `MemoryAction { Add, Replace, Remove }`. Add: append `text` as a new line; if result > cap → Err(MEMORY_FULL as specified). Replace: substring-replace the FIRST occurrence of `text`'s… no — Replace takes `text` = the full new file body (simplest, matches Hermes "replace" being a rewrite); over cap → Err. Remove: delete lines containing `text` (substring). Returns the new file contents. Pure core `fn cap_check(body: &str, cap: usize) -> bool`.
- ops.rs: `pub async fn memory_update(db: &Db, file: String, action: String, text: String) -> Result<String>` — map strings to enums (bad value → Err), call knowledge::apply_memory_update. (db unused today but keep the signature uniform with other ops.)
- ops.rs prompt: in `build_system_prompt`, after the `## Basics checklist` block, inject `knowledge::memory_prompt_section()?` when non-empty (before `## What Donna knows about this user`).
- tools.rs: register `memory_update` — Risk::Write, params `{file: "user"|"memory", action: "add"|"replace"|"remove", text: string}`, execute arm calls `ops::memory_update`. Description: "Update your durable memory about the user. USER.md = stable identity/preferences (cap 1500 chars); MEMORY.md = active threads/conventions (cap 2500 chars). 'add' appends a line (errors if full — then consolidate), 'replace' rewrites the whole file, 'remove' deletes lines containing the text. Keep entries terse." Update the registry count assertion (was 31 → 32).

- [ ] **Step 1: Failing tests**

```rust
// knowledge.rs — uses DONNA_KB_DIR temp root (set env per-test like other kb tests)
#[test]
fn memory_add_until_full_then_errors() {
    let _root = temp_kb(); // sets DONNA_KB_DIR to a fresh temp dir + ensure_root
    assert_eq!(read_memory_file(MemoryFile::User).unwrap(), "");
    apply_memory_update(MemoryFile::User, MemoryAction::Add, "Name: Buno").unwrap();
    assert!(read_memory_file(MemoryFile::User).unwrap().contains("Name: Buno"));
    // fill past the 1500 cap
    let big = "x".repeat(1600);
    let err = apply_memory_update(MemoryFile::User, MemoryAction::Add, &big).unwrap_err();
    assert!(err.to_string().contains("MEMORY_FULL"));
    // replace to shrink works even near cap
    apply_memory_update(MemoryFile::User, MemoryAction::Replace, "Name: B").unwrap();
    assert_eq!(read_memory_file(MemoryFile::User).unwrap().trim(), "Name: B");
    // remove
    apply_memory_update(MemoryFile::User, MemoryAction::Remove, "Name: B").unwrap();
    assert_eq!(read_memory_file(MemoryFile::User).unwrap().trim(), "");
}

#[test]
fn memory_prompt_section_shape() {
    let _root = temp_kb();
    assert_eq!(memory_prompt_section().unwrap(), "");
    apply_memory_update(MemoryFile::User, MemoryAction::Add, "Prefers concise replies").unwrap();
    let s = memory_prompt_section().unwrap();
    assert!(s.contains("## About you"));
    assert!(s.contains("Prefers concise replies"));
    assert!(!s.contains("## Working memory")); // MEMORY.md still empty → omitted
}
```

```rust
// tools.rs
#[tokio::test]
async fn memory_update_tool_registered_and_dispatches() {
    let db = test_db();
    let _root = /* temp kb via the crate's kb test helper */;
    let out = execute(&db, "memory_update", &serde_json::json!({"file":"user","action":"add","text":"Likes tea"})).await.unwrap();
    assert!(out.contains("Likes tea"));
    assert_eq!(all().len(), 32);
}
```

- [ ] **Step 2: RED** — `cargo test -p donna-core memory` → FAIL. **Step 3: Implement** (find the existing kb-test temp-root helper — several knowledge.rs tests set DONNA_KB_DIR; reuse the exact pattern). **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`, zero new warnings.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add capped USER.md/MEMORY.md memory files, memory_update tool, prompt injection

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 2: FTS5 index + session_search tool

**Files:**
- Modify: `crates/donna-core/src/db.rs` (FTS DDL + triggers + backfill + search method), `crates/donna-core/src/tools.rs` (register)
- Test: inline in db.rs + tools.rs

**Interfaces:**
- migrate() append (inside the existing execute_batch, after the messages table):
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(content, content='messages', content_rowid='id');
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', old.id, old.content);
END;
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', old.id, old.content);
  INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
```
- `Db::ensure_fts_backfill(&self) -> Result<()>` — called from `open()` AFTER migrate(): if `get_setting("fts_backfilled") != Some("1")` → `conn.execute("INSERT INTO messages_fts(messages_fts) VALUES('rebuild')", [])?` then `set_setting("fts_backfilled", "1")`. (Locks the mutex once; don't call get/set_setting while holding the lock — sequence them.)
- `Db::search_messages(&self, query: &str, limit: i64) -> Result<Vec<Message>>` — sanitize: `let q = format!("\"{}\"", query.replace('"', "\"\""));` then `SELECT m.* FROM messages_fts f JOIN messages m ON m.id = f.rowid WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT ?2`. Empty/whitespace query → Ok(vec![]).
- tools.rs: register `session_search` — Risk::Read, params `{query: string, limit?: integer (default 10, ≤25)}`, execute returns JSON of `[{conversation_id, role, content, created_at}]` (drop id). Description: "Full-text search across the user's entire message history (all past conversations). Use to recall what the user told you before. Returns matching messages newest-relevance first." Registry count 32 → 33.

- [ ] **Step 1: Failing tests**

```rust
// db.rs
#[test]
fn fts_search_finds_and_backfills() {
    let db = test_db();
    let c = db.create_conversation("t").unwrap();
    db.add_message(c, "user", "my dentist is Dr. Alvarez on Maple Street").unwrap();
    db.add_message(c, "assistant", "noted").unwrap();
    let hits = db.search_messages("dentist", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("Alvarez"));
    assert!(db.search_messages("nonexistentword", 10).unwrap().is_empty());
    // punctuation in query must not blow up FTS syntax
    assert!(db.search_messages("Dr. Alvarez\"; DROP", 10).is_ok());
}

#[test]
fn fts_backfill_covers_preexisting_rows() {
    // simulate a DB whose rows predate triggers: insert directly bypassing triggers is hard;
    // instead assert ensure_fts_backfill is idempotent and the flag flips.
    let db = test_db();
    assert_eq!(db.get_setting("fts_backfilled").unwrap().as_deref(), Some("1")); // open() ran it
    db.ensure_fts_backfill().unwrap(); // idempotent second call
}
```

```rust
// tools.rs
#[tokio::test]
async fn session_search_tool() {
    let db = test_db();
    let c = db.create_conversation("t").unwrap();
    db.add_message(c, "user", "remember the wifi password is hunter2").unwrap();
    let out = execute(&db, "session_search", &serde_json::json!({"query":"wifi password"})).await.unwrap();
    assert!(out.contains("hunter2"));
    assert_eq!(all().len(), 33);
}
```

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add FTS5 index over messages with backfill and session_search tool

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 3: Events log + suggestions queue (DB + ops + RPC)

**Files:**
- Modify: `crates/donna-core/src/db.rs` (events + suggestions tables/methods), `crates/donna-core/src/ops.rs` (event logging at 3 hooks + suggestion ops), `crates/donna-core/src/agent.rs` (tool-call event), `donna-server/src/rpc.rs` (arms)
- Test: inline + one rpc test

**Interfaces:**
- db.rs migrate() append: `events` + `idx_events_created` + `suggestions` per Global Constraints. Methods:
  - `insert_event(&self, kind: &str, conversation_id: Option<i64>, tool: Option<&str>, payload_json: Option<&str>) -> Result<i64>`
  - `recent_events(&self, limit: i64) -> Result<Vec<Event>>` (newest first; `Event { id, kind, conversation_id, tool, payload_json, created_at }`)
  - `insert_suggestion(&self, kind, title, body, payload_json: Option<&str>, dedup_key: &str) -> Result<Option<i64>>` — returns None (skips) when a non-pending row with the same dedup_key exists OR a pending row with that key exists (never double-file); else inserts, returns Some(id).
  - `list_suggestions(&self, pending_only: bool) -> Result<Vec<Suggestion>>`, `get_suggestion(&self, id) -> Result<Option<Suggestion>>`, `resolve_suggestion(&self, id, status: &str) -> Result<()>` (pending→accepted/dismissed only).
- ops.rs event hooks (best-effort `let _ =`):
  - `send_chat` (start): `db.insert_event("user_request", Some(conversation_id), None, None)`.
  - `approval_respond`: `db.insert_event("approval", Some(a.conversation_id), Some(&a.tool), Some(&json!({"approved":approve}).to_string()))`.
- agent.rs: in `handle_tool_call` after a tool executes (Auto path), `let _ = db.insert_event("tool_call", Some(conversation_id), Some(name), None);` (log the attempt regardless of success).
- ops.rs suggestion ops: `suggestions_list(db, pending_only: bool) -> Result<Vec<Suggestion>>`; `suggestion_respond(db, id, accept: bool) -> Result<String>` — dismiss → resolve_suggestion(id,"dismissed"), return "dismissed". accept → resolve(id,"accepted") + ACT on it by kind: for Phase 4 the only auto-actable kind is `"routine"` (payload_json = a CreateRoutineInput) → `ops::create_routine`; other kinds just mark accepted and return "accepted" (the act is manual/other-phase). Insert a notification either way ("Suggestion accepted"/"Suggestion dismissed").
- rpc.rs arms: `suggestions_list` (`pendingOnly`), `suggestion_respond` (`id`, `accept`), `recent_events` (`limit`) [read-only, for a future UI].

- [ ] **Step 1: Failing tests**

```rust
// db.rs
#[test]
fn suggestion_dedup_latches() {
    let db = test_db();
    let a = db.insert_suggestion("routine","Daily standup prep","...",None,"routine:standup").unwrap();
    assert!(a.is_some());
    assert!(db.insert_suggestion("routine","dup","...",None,"routine:standup").unwrap().is_none()); // pending dup
    db.resolve_suggestion(a.unwrap(), "dismissed").unwrap();
    assert!(db.insert_suggestion("routine","again","...",None,"routine:standup").unwrap().is_none()); // dismissed → never again
    assert!(db.list_suggestions(false).unwrap().len() == 1);
}
#[test]
fn events_recorded_and_listed() {
    let db = test_db();
    db.insert_event("user_request", Some(1), None, None).unwrap();
    db.insert_event("tool_call", Some(1), Some("kb_search"), None).unwrap();
    let ev = db.recent_events(10).unwrap();
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0].kind, "tool_call"); // newest first
}
```

```rust
// ops.rs
#[tokio::test]
async fn suggestion_accept_routine_creates_it() {
    let db = test_db();
    let payload = serde_json::json!({"name":"Standup prep","schedule_type":"daily","hour":9,"minute":0,"prompt":"..."}).to_string();
    let id = db.insert_suggestion("routine","Standup prep","daily 9am",Some(&payload),"routine:standup").unwrap().unwrap();
    let out = suggestion_respond(&db, id, true).await.unwrap();
    assert_eq!(out, "accepted");
    assert!(db.list_routines().unwrap().iter().any(|r| r.name == "Standup prep"));
}
```

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test --workspace && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add events log and consent-first suggestions queue with RPC arms

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 4: Background review (memory curator + pattern → suggestion)

**Files:**
- Create: `crates/donna-core/src/review.rs` (+ `pub mod review;`)
- Modify: `crates/donna-core/src/ops.rs` (config: review_model read), `crates/donna-core/src/scheduler.rs` (nightly hook)
- Test: inline in review.rs

**Interfaces:**
- `review.rs`:
  - `pub async fn run_background_review(db: &Db) -> Result<ReviewOutcome>` (`ReviewOutcome { memory_updated: bool, suggestions_filed: usize }`):
    1. Gather: last N (=40) messages across recent conversations (add `Db::recent_messages(limit)` if not present — SELECT ... ORDER BY id DESC LIMIT) + current USER.md/MEMORY.md + `db.recent_events(200)`.
    2. Model = setting `review_model` or config.model; if no model configured → Ok(zero outcome). Cheap `providers::complete` call with a REVIEW_PROMPT that asks for STRICT JSON: `{ "memory": [ {"file":"user"|"memory","action":"add"|"replace"|"remove","text":"..."} ], "suggestions": [ {"kind":"routine","title":"...","body":"...","dedup_key":"...","payload": {...CreateRoutineInput...} } ] }`. Empty arrays = nothing to do (the common case).
    3. Apply: each memory op via `knowledge::apply_memory_update` (swallow MEMORY_FULL per-op — the model should consolidate next round; count a success only when applied). Each suggestion via `db.insert_suggestion(...)` (dedup latch does the rest); on a filed suggestion insert a notification "💡 Suggestion: <title>".
    - Pure helper `fn parse_review_json(s: &str) -> ReviewPlan` (reuse the extract-first-`{...}` approach from kg_extract) — TDD this against a canned model output.
- ops.rs: `load_config` unchanged; add a tiny `pub fn review_model(db: &Db) -> String` (setting `review_model` non-empty else config.model).
- scheduler.rs: in `tick`, once per day (guard with a `routine_runs`-style dedupe or a settings `last_review_day` check against the tz-aware now's date) → `let _ = review::run_background_review(db).await;`. Runs regardless of provider (uses complete). `// ponytail: nightly is enough; a post-session trigger can be added when sessions are cheap to detect server-side.`

- [ ] **Step 1: Failing tests** (pure parse + apply, NO network):

```rust
#[test]
fn parse_review_json_extracts_plan() {
    let out = r#"Sure! {"memory":[{"file":"user","action":"add","text":"Timezone: Asia/Bangkok"}],
      "suggestions":[{"kind":"routine","title":"Morning digest","body":"...","dedup_key":"routine:digest","payload":{"name":"Morning digest","schedule_type":"daily","hour":8,"minute":0,"prompt":"..."}}]}"#;
    let plan = parse_review_json(out);
    assert_eq!(plan.memory.len(), 1);
    assert_eq!(plan.suggestions.len(), 1);
    assert_eq!(plan.suggestions[0].dedup_key, "routine:digest");
}

#[test]
fn empty_plan_is_noop() {
    let plan = parse_review_json(r#"{"memory":[],"suggestions":[]}"#);
    assert!(plan.memory.is_empty() && plan.suggestions.is_empty());
    // garbage → empty plan, never panics
    let plan2 = parse_review_json("no json here");
    assert!(plan2.memory.is_empty() && plan2.suggestions.is_empty());
}
```

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test -p donna-core review && cargo test --workspace && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add nightly background review: memory curation and pattern-based suggestions

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 5: [SILENT] routines

**Files:**
- Modify: `crates/donna-core/src/scheduler.rs` (sentinel gate in execute_routine + prompt line)
- Test: inline

**Interfaces:**
- Pure `fn is_silent(content: &str) -> bool` — true when trimmed content is empty OR starts with `[SILENT]` (case-insensitive).
- execute_routine: after computing `content`, if `is_silent(&content)` → mark_routine_run + record dedupe (EXACTLY what the success path does to prevent a 60s re-fire) and RETURN before the emit block (no doc, no notification, no messenger send). Only non-silent content proceeds to doc/notify/send.
- Routine system prompt (execute_routine, the persona ChatTurn ~scheduler.rs:266): append a line: "If, after reviewing the context, there is nothing new or worth surfacing to the user right now, reply with exactly [SILENT] and nothing else."

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn is_silent_detects_sentinel_and_empty() {
    assert!(is_silent(""));
    assert!(is_silent("   \n "));
    assert!(is_silent("[SILENT]"));
    assert!(is_silent("[silent] nothing to report"));
    assert!(!is_silent("Here is your morning briefing..."));
}
```
(The emit-suppression is integration-shaped; assert the pure gate here. In the report, note the manual check: a routine whose model returns [SILENT] must not create a doc/notification but must still update last_run_at so it doesn't re-fire — verify by reading the code path.)

- [ ] **Step 2: RED** → **Step 3: Implement** (find the exact mark_routine_run + dedupe calls in the success path and mirror them on the silent path) → **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Routines stay silent when nothing matters via [SILENT] sentinel

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 6: Dashboard suggestions + Settings review model + docs

**Files:**
- Modify: `src/lib/api.ts` (Suggestion type + methods, review_model in AppConfig mapping), `src/routes/Dashboard.tsx` (suggestions card), `src/routes/Settings.tsx` (review model field), `docs/ROADMAP.md`
- Verify: tsc/build + cargo check + full suite

**Interfaces:**
- api.ts: `Suggestion { id, kind, title, body, status, createdAt }` + `toSuggestion` mapper; `suggestionsList(pendingOnly)`, `suggestionRespond(id, accept)`. Add `reviewModel` to AppConfig + RawConfig mapping (save_config already round-trips settings via config — confirm review_model rides the config blob OR add a dedicated setting write like whatsapp_my_number did; reuse whichever is simplest given how AppConfig persists).
- Dashboard.tsx: a "Suggestions from Donna" card (only when pending suggestions exist) — each row: title + body + Accept/Dismiss buttons → `api.suggestionRespond` → optimistic remove + refetch. Style like the existing dashboard cards / the approval cards from Phase 2.
- Settings.tsx: in the model/provider card, add an optional "Background review model" input (placeholder "defaults to your main model") bound to reviewModel, saved with the config Save button.
- ROADMAP.md: check the Phase 4 (Growth) items; leave Phases 5-6 unchecked.

- [ ] **Step 1: Implement all four.** **Step 2:** `npx tsc --noEmit && npm run build && cargo test --workspace && cargo check --workspace` clean/green. **Step 3:** report a 5-line manual smoke checklist (memory persists across a chat; session_search recalls an old message; a suggestion appears and Accept creates a routine).
- [ ] **Step 4: Commit and push**

```bash
git add -A
git commit -m "Dashboard suggestion cards, background-review model setting, roadmap sync

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

## Done criteria (whole phase)

1. `cargo test --workspace` green; tsc/build clean.
2. `memory_update` writes USER.md/MEMORY.md, errors with MEMORY_FULL past the cap, and the files inject into every system prompt; a fact Donna records in one chat is present in the next.
3. `session_search` finds a message from any past conversation (FTS5), including for messages that predate the index (backfill).
4. Events log records chat requests, tool calls, and approvals.
5. The nightly background review can curate memory and file a suggestion; the suggestion appears on the Dashboard; Accept on a `routine` suggestion creates the routine; Dismiss latches (never re-offered).
6. A routine whose model returns `[SILENT]` produces no doc/notification but does not re-fire every tick.

## Follow-ups noted during planning (not in scope)

- Routines running through the full tool-agent loop (not just `complete`) — deferred; run_agent_turn is conversation-bound + OpenAI-only, and the plain path already gathers rich context. Revisit if routines need to *act*.
- Post-session background-review trigger (vs nightly only) — needs a server-side session-end signal; nightly covers the pattern-learning goal for now.
- Automatic trust-policy graduation from the approvals history (`count_consecutive_approvals` exists) — a natural next suggestion-kind once the queue proves out.
- WhatsApp nudge for new suggestions — notification + Dashboard suffice this phase.
