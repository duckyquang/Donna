# Phase 2: Hands — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Donna can *do* things: chat runs an OpenAI tool-calling agent loop over 31 tools with risk classes, gated by a trust engine — Read/Write execute silently, Outbound asks first via approval cards, with a policy table the user controls.

**Architecture:** New `agent` + `tools` + `trust` modules in donna-core. `providers.rs` gains one new OpenAI-only entry point (`openai_agent_step`) that streams content deltas AND accumulates tool-call deltas — the existing `stream_chat`/`consume_sse` used by everything else is untouched. `ops::send_chat` routes to the agent loop when provider is `openai` (with API key); all other providers keep today's plain path. Approvals and trust policies are DB tables surfaced over new RPC arms; the Chat UI shows tool-status lines while streaming and DB-backed approval cards; Settings gets a policy editor.

**Tech Stack:** existing only — Rust (reqwest SSE, serde_json), axum RPC/WS, React/TS. No new dependencies.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-07-donna-jarvis-design.md` §4 (agent core). Phase 2 only — no WhatsApp webhook (Phase 3), no USER.md/FTS5/suggestion queue (Phase 4), no skills tools (Phase 6).
- **Commit AND push after every task.** Branch `feat/phase-5-projects-discord-proactive`, no PRs.
- Agent loop caps: **12 iterations** per turn; token budget **60,000 total tokens** per turn accumulated from OpenAI `usage` (stream_options include_usage); a tool that errors **twice in one turn** is disabled for the rest of that turn (result replaced with `TOOL_DISABLED: repeated failures`).
- Risk classes: `Read` and `Write` always auto-execute. `Outbound` consults `trust_policies` (action_kind = tool name): mode `ask` (default when absent) or `auto`.
- Tool results fed to the model are JSON strings truncated to **6,000 chars** (append `…[truncated]` marker).
- Tool call/result turns are **NOT persisted** to the messages table — only user/assistant messages, exactly as today. `// ponytail: per-turn tool context is ephemeral; cross-turn memory arrives with Phase 4's MEMORY.md.`
- New DB tables only (migrate() is CREATE IF NOT EXISTS with no versioning — never add columns to existing tables).
- ChatEvent gains variants `Tool { name, label, status }` (status: `running`|`done`|`error`) and `Approval { approval_id, summary, tool }` — camelCase variant tags, snake_case fields, matching the existing serde attrs. The client's terminal events remain only `done`/`error`.
- Excluded from the tool surface, deliberately: `kg_reset`/`knowledge::reset` (destructive), `project_*` (desktop-local filesystem), `kg_extract` + nested-LLM ops (`fathom_process_recent_meeting`, `news_article_summary`, `reading_list_summarize`) — the model reads data and reasons itself.
- Phase-2 simplification (documented, spec-conformant in spirit): automatic ask→auto graduation and auto→ask demotion arrive with Phase 4's suggestion queue. In Phase 2 the policy table is edited manually in Settings; approvals history (the graduation fuel) is recorded from day one.

---

### Task 1: DB tables + reminders

**Files:**
- Modify: `crates/donna-core/src/db.rs` (migrate batch + new method sections)
- Modify: `crates/donna-core/src/scheduler.rs` (reminder check in tick)
- Modify: `crates/donna-core/src/ops.rs` (remember op)
- Test: inline in db.rs + scheduler.rs

**Interfaces:**
- Produces (db.rs methods, follow the existing section-comment + `Result<T>` patterns):
  - `// --- Trust policies ---` `get_trust_policy(&self, action_kind: &str) -> Result<Option<String>>`, `set_trust_policy(&self, action_kind: &str, mode: &str) -> Result<()>` (upsert like set_setting), `list_trust_policies(&self) -> Result<Vec<TrustPolicy>>` (`TrustPolicy { action_kind: String, mode: String, updated_at: String }`)
  - `// --- Approvals ---` `insert_approval(&self, conversation_id: i64, tool: &str, args_json: &str, summary: &str) -> Result<i64>`, `get_approval(&self, id: i64) -> Result<Option<Approval>>`, `list_approvals(&self, pending_only: bool) -> Result<Vec<Approval>>`, `list_pending_approvals_for_conversation(&self, conversation_id: i64) -> Result<Vec<Approval>>`, `resolve_approval(&self, id: i64, status: &str) -> Result<()>` (sets resolved_at; only from `pending`), `count_consecutive_approvals(&self, tool: &str) -> Result<i64>` (recorded now, consumed by Phase 4)
  - `Approval { id: i64, conversation_id: i64, tool: String, args_json: String, summary: String, status: String, created_at: String, resolved_at: Option<String> }`
  - `// --- Reminders ---` `insert_reminder(&self, text: &str, due_at: &str) -> Result<i64>`, `due_unfired_reminders(&self, now_iso: &str) -> Result<Vec<Reminder>>`, `mark_reminder_fired(&self, id: i64) -> Result<()>` (`Reminder { id, text, due_at, fired, created_at }`)
  - `ops::remember(db: &Db, text: String, due_at: String) -> Result<i64>` (validates due_at parses as RFC3339, else `Error::Provider("due_at must be RFC3339…")`)
- Scheduler tick: after routine processing, fetch due unfired reminders → `notifier.notify("Reminder", &r.text)` + `db.insert_notification(...)` (match how routine notifications insert) + `mark_reminder_fired`.

- [ ] **Step 1: Write failing tests** (append to db.rs tests or new `#[cfg(test)]` mod; temp-dir Db like ops.rs's `conversation_crud_roundtrip`)

```rust
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
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p donna-core trust_policy approval reminders` → FAIL (methods undefined).

- [ ] **Step 3: Implement** — append to migrate() batch:

```sql
CREATE TABLE IF NOT EXISTS trust_policies (
  action_kind TEXT PRIMARY KEY,
  mode TEXT NOT NULL CHECK (mode IN ('ask','auto')),
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS approvals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  conversation_id INTEGER NOT NULL,
  tool TEXT NOT NULL,
  args_json TEXT NOT NULL,
  summary TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','expired')),
  created_at TEXT NOT NULL,
  resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_approvals_status ON approvals(status);
CREATE TABLE IF NOT EXISTS reminders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  text TEXT NOT NULL,
  due_at TEXT NOT NULL,
  fired INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
```

Methods follow existing patterns (`now_iso()`, Mutex lock, section comments). `resolve_approval`: `UPDATE approvals SET status=?1, resolved_at=?2 WHERE id=?3 AND status='pending'`. `count_consecutive_approvals`: latest-first scan of resolved approvals for the tool, count leading `approved` until first `rejected` (SQL or Rust-side loop — either fine). Scheduler: add the reminder sweep to `tick` after routines, using the same `now` the tick computed.

- [ ] **Step 4: Run tests** — `cargo test -p donna-core && cargo check --workspace` → PASS, zero new warnings.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add trust policies, approvals, and reminders tables with scheduler sweep

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 2: OpenAI agent step — tool-calling with streamed deltas

**Files:**
- Modify: `crates/donna-core/src/providers.rs`
- Test: inline

**Interfaces:**
- Produces (providers.rs):
  - `AgentTurn { role: String, content: Option<String>, tool_calls: Option<Vec<ToolCallOut>>, tool_call_id: Option<String> }` with `#[serde(skip_serializing_if = "Option::is_none")]` on the options — serializes to the exact OpenAI messages shape (assistant turns carry tool_calls; tool turns carry tool_call_id + content).
  - `ToolCallOut { id: String, #[serde(rename = "type")] kind: String /* "function" */, function: FunctionCall }`, `FunctionCall { name: String, arguments: String }`
  - `ToolCall { id: String, name: String, arguments: String }` (the assembled result the loop consumes)
  - `AgentStep { content: String, tool_calls: Vec<ToolCall>, total_tokens: u64 }`
  - `pub async fn openai_agent_step(api_key: &str, model: &str, messages: &[AgentTurn], tools: &serde_json::Value, on_token: &mut (impl FnMut(&str) + Send)) -> Result<AgentStep>`
  - Pure, tested: `fn accumulate_tool_delta(acc: &mut Vec<PartialToolCall>, delta: &serde_json::Value)` assembling fragmented `choices[0].delta.tool_calls[]` chunks by `index` (id and function.name arrive on the first fragment; function.arguments accumulates across fragments).
- Consumes: nothing new. `stream_chat`/`consume_sse`/`ChatTurn` are NOT modified.

- [ ] **Step 1: Write failing tests** — the accumulator is the risky logic; test with real OpenAI-shaped chunks:

```rust
#[test]
fn tool_delta_accumulation_assembles_fragmented_calls() {
    let mut acc = Vec::new();
    for chunk in [
        r#"{"index":0,"id":"call_a","type":"function","function":{"name":"slack_send_message","arguments":""}}"#,
        r#"{"index":0,"function":{"arguments":"{\"channel\":"}}"#,
        r#"{"index":0,"function":{"arguments":"\"#general\",\"text\":\"hi\"}"}}"#,
        r#"{"index":1,"id":"call_b","type":"function","function":{"name":"list_docs","arguments":"{}"}}"#,
    ] {
        accumulate_tool_delta(&mut acc, &serde_json::from_str(chunk).unwrap());
    }
    let calls = finish_tool_calls(acc);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "slack_send_message");
    assert_eq!(calls[0].arguments, r#"{"channel":"#general","text":"hi"}"#);
    assert_eq!(calls[1].name, "list_docs");
}

#[test]
fn agent_turn_serializes_openai_shapes() {
    let assistant = AgentTurn { role: "assistant".into(), content: None,
        tool_calls: Some(vec![ToolCallOut { id: "call_a".into(), kind: "function".into(),
            function: FunctionCall { name: "list_docs".into(), arguments: "{}".into() } }]),
        tool_call_id: None };
    let v = serde_json::to_value(&assistant).unwrap();
    assert!(v.get("content").is_none());
    assert_eq!(v["tool_calls"][0]["function"]["name"], "list_docs");
    let tool = AgentTurn { role: "tool".into(), content: Some("[]".into()), tool_calls: None, tool_call_id: Some("call_a".into()) };
    let v = serde_json::to_value(&tool).unwrap();
    assert_eq!(v["tool_call_id"], "call_a");
    assert!(v.get("tool_calls").is_none());
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p donna-core providers` → FAIL.
- [ ] **Step 3: Implement** — `openai_agent_step` POSTs the same endpoint as `stream_openai` (bearer_auth, stream: true) with body `{model, messages, tools, stream: true, stream_options: {"include_usage": true}}`. Write a bespoke SSE loop for this fn (do NOT bend `consume_sse` — its extract contract is text-only): per data line, parse JSON; `choices[0].delta.content` → `on_token` + append to content; `choices[0].delta.tool_calls[]` → `accumulate_tool_delta`; final usage chunk → `total_tokens = usage.total_tokens`. Non-2xx → same status+body error shape as `stream_openai`. `finish_tool_calls(acc) -> Vec<ToolCall>` drops entries with empty name.
- [ ] **Step 4: Run tests** — `cargo test -p donna-core && cargo check --workspace` → PASS.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add OpenAI agent step: streamed tool-call deltas with usage accounting

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 3: Tool registry

**Files:**
- Create: `crates/donna-core/src/tools.rs` (+ `pub mod tools;` in lib.rs)
- Modify: `crates/donna-core/src/ops.rs` (add `create_doc` wrapper), `crates/donna-core/src/integrations/weather.rs` (derive Serialize on WeatherSummary — check `description: &'static str` serializes fine; it does)
- Test: inline in tools.rs

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, PartialEq)] pub enum Risk { Read, Write, Outbound }`
  - `pub struct ToolDef { pub name: &'static str, pub description: &'static str, pub params: serde_json::Value, pub risk: Risk }`
  - `pub fn all() -> Vec<ToolDef>` — the full registry
  - `pub fn openai_tools_json() -> serde_json::Value` — `[{ "type": "function", "function": { name, description, parameters } }, …]`
  - `pub fn risk_of(name: &str) -> Option<Risk>`
  - `pub async fn execute(db: &Db, name: &str, args: &serde_json::Value) -> Result<String>` — one match (rpc.rs-style), each arm deserializes args, calls the underlying fn, serializes the result to JSON, truncates to 6,000 chars via a shared `fn truncate_result(s: String) -> String`
  - `pub fn summarize_call(name: &str, args: &serde_json::Value) -> String` — human-readable one-liner for approval cards / Tool events ("Send Slack message to #general", "Create calendar event 'Standup'…"). A match over Outbound + notable Write tools; default `"{name} {args}"` truncated to 120 chars.
- Consumes: ops fns and integration fns per the inventory below; `ops::remember` from Task 1.

**The registry (31 tools; args schemas are OpenAI JSON Schema objects):**

Read (12): `calendar_list_events(time_min, time_max: string RFC3339)`, `gmail_list_messages(max_results: integer ≤25, default 10)`, `drive_list_files(max_results: integer ≤25)`, `slack_list_channels()`, `github_list_repos(max_results)`, `github_list_issues(max_results)`, `linear_list_issues(max_results)`, `notion_list_pages(max_results)` → integrations::notion::search_pages, `fathom_recent_meetings(limit ≤10)` → integrations::fathom::list_recent_meetings, `news_top_stories(limit ≤15)`, `weather_current(lat: number, lon: number)` → `integrations::weather::fetch` + `format_summary` (return the formatted string), `kb_search(query: string)` → `retrieval::search_for_prompt(query, db, &RetrievalConfig{…from settings})`.
Plus reads over Donna's own data (5): `list_docs()`, `get_doc(id: integer)`, `list_routines()`, `reading_list_get()`, `habit_list()`.

Write (11): `kb_save_node(folder: string[], label, note, node_type: string)` → ops::kg_save_node (from_folder/from_id None), `gmail_create_draft(to, subject, body)`, `google_create_doc(title)`, `create_doc(title, content)` → new `ops::create_doc(db, title, content)` wrapping `docs::create(db, title, "agent", content)`, `calendar_create_event(summary, description?, start, end)` → build CalendarEvent{id:None,…}, `calendar_update_event(id, summary?, description?, start, end)`, `calendar_delete_event(id)`, `reading_list_add(url, title)`, `habit_log(habit_id: integer, note?)`, `remember(text, due_at: string RFC3339)`, `create_routine(name, schedule_type: "daily"|"weekly", hour, minute, day_of_week?, prompt?)` → ops::create_routine.

Outbound (3): `slack_send_message(channel, text)`, `telegram_send_message(text)`, `whatsapp_send_message(to: string E.164, text)`.

Note `calendar_delete_event` is Write per the spec's own-calendar rule; `summarize_call` must still render it clearly ("Delete calendar event <id>").

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn registry_names_unique_and_schemas_valid() {
    let defs = all();
    assert_eq!(defs.len(), 31);
    let names: std::collections::HashSet<_> = defs.iter().map(|d| d.name).collect();
    assert_eq!(names.len(), defs.len());
    for d in &defs {
        assert_eq!(d.params["type"], "object", "{} params must be an object schema", d.name);
    }
    let json = openai_tools_json();
    assert_eq!(json.as_array().unwrap().len(), 31);
}

#[tokio::test]
async fn execute_dispatches_db_tool() {
    let db = test_db();
    crate::docs::create(&db, "T", "test", "body").unwrap();
    let out = execute(&db, "list_docs", &serde_json::json!({})).await.unwrap();
    assert!(out.contains("\"T\""));
    let err = execute(&db, "no_such_tool", &serde_json::json!({})).await.unwrap_err();
    assert!(err.to_string().contains("unknown tool"));
}

#[test]
fn truncation_and_summaries() {
    assert!(truncate_result("x".repeat(10_000)).len() < 6_100);
    let s = summarize_call("slack_send_message", &serde_json::json!({"channel":"#general","text":"hi"}));
    assert!(s.contains("#general"));
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p donna-core tools` → FAIL.
- [ ] **Step 3: Implement** per the interfaces. Args deserialization errors → `Err(Error::Provider(format!("bad arguments for {name}: {e}")))` (the loop feeds this back to the model). READ THE REAL SIGNATURES before writing arms — the inventory above came from code recon but the code is truth.
- [ ] **Step 4: Run tests** — `cargo test -p donna-core && cargo check --workspace` → PASS.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add agent tool registry: 31 tools with risk classes, dispatch, and summaries

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 4: Trust engine

**Files:**
- Create: `crates/donna-core/src/trust.rs` (+ `pub mod trust;`)
- Test: inline

**Interfaces:**
- Produces:
  - `pub enum Decision { Auto, Ask }`
  - `pub fn decide(db: &Db, tool_name: &str) -> Result<Decision>` — `tools::risk_of(tool_name)`: Read/Write → Auto; Outbound → `db.get_trust_policy(tool_name)`: `Some("auto")` → Auto, else Ask. Unknown tool → Err.
  - `pub fn request_approval(db: &Db, conversation_id: i64, tool: &str, args: &serde_json::Value) -> Result<Approval>` — summary via `tools::summarize_call`, inserts approval row AND a notification row (`title: "Approval needed"`, body = summary) so it reaches the Notifications page + WS broadcast path used by the scheduler.
- Consumes: Task 1 Db methods, Task 3 `risk_of`/`summarize_call`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn decide_by_risk_and_policy() {
    let db = test_db();
    assert!(matches!(decide(&db, "list_docs").unwrap(), Decision::Auto));           // Read
    assert!(matches!(decide(&db, "kb_save_node").unwrap(), Decision::Auto));        // Write
    assert!(matches!(decide(&db, "slack_send_message").unwrap(), Decision::Ask));   // Outbound default
    db.set_trust_policy("slack_send_message", "auto").unwrap();
    assert!(matches!(decide(&db, "slack_send_message").unwrap(), Decision::Auto));
    db.set_trust_policy("slack_send_message", "ask").unwrap();
    assert!(matches!(decide(&db, "slack_send_message").unwrap(), Decision::Ask));
    assert!(decide(&db, "nonexistent").is_err());
}

#[test]
fn request_approval_creates_row_and_notification() {
    let db = test_db();
    let a = request_approval(&db, 7, "whatsapp_send_message", &serde_json::json!({"to":"+15550100","text":"yo"})).unwrap();
    assert_eq!(a.status, "pending");
    assert!(a.summary.contains("+15550100"));
    assert!(db.list_notifications().unwrap().iter().any(|n| n.title == "Approval needed"));
}
```

- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement** (match the real `insert_notification` signature in db.rs). **Step 4:** `cargo test -p donna-core && cargo check --workspace` → PASS.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add trust engine: risk/policy decisions and approval requests

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 5: Agent loop + send_chat routing

**Files:**
- Create: `crates/donna-core/src/agent.rs` (+ `pub mod agent;`)
- Modify: `crates/donna-core/src/ops.rs` (ChatEvent variants; send_chat routes to agent when provider==openai && key present)
- Test: inline in agent.rs (pure parts)

**Interfaces:**
- Produces:
  - ChatEvent gains: `Tool { name: String, label: String, status: String }`, `Approval { approval_id: i64, summary: String, tool: String }` (same serde attrs; wire: `{"type":"tool",…}`, `{"type":"approval",…}`).
  - `pub async fn run_agent_turn(db: &Db, conversation_id: i64, api_key: &str, model: &str, on_event: &(dyn Fn(ChatEvent) + Send + Sync)) -> Result<()>`
- Loop mechanics (all constants from Global Constraints):
  1. Build `Vec<AgentTurn>` = system turn (existing `build_system_prompt` + the Task 8 agent addendum) + full user/assistant history (content-only turns) — mirroring send_chat's current history assembly.
  2. Iterate ≤12: `providers::openai_agent_step(key, model, &turns, &tools::openai_tools_json(), on_token→ChatEvent::Token)`. Accumulate `total_tokens`; if >60_000 → append a final assistant apology message and break (persist + Done).
  3. If `tool_calls` empty → final answer: persist assistant content (`db.add_message`), `maybe_generate_title` best-effort, emit `Done { message_id }`, return.
  4. Else push the assistant AgentTurn (content + tool_calls), then for each call:
     - `trust::decide`: Auto → emit `Tool{status:"running", label: summarize_call(…)}`, `tools::execute`, emit `Tool{status:"done"|"error"}`, push tool AgentTurn with the result (or the error text). Track per-tool error counts; second error → result `"TOOL_DISABLED: repeated failures"` and stop calling execute for that tool this turn.
     - Ask → `trust::request_approval`, emit `Approval{…}`, push tool AgentTurn with `"PENDING_APPROVAL: the user has been asked to approve this action out-of-band. Do not retry or work around it; acknowledge and continue."`
  5. Loop. If iteration cap hits → persist whatever content the last step produced (or a "I hit my step limit" message), Done.
- send_chat routing (ops.rs): after config/api_key load — `if config.provider == "openai" && api_key.is_some() { return agent::run_agent_turn(…).await; }` — all other paths byte-identical to today. quick_chat_send stays plain (no tools) this phase.

- [ ] **Step 1: Failing tests for the pure parts** — extract `fn build_history_turns(messages: &[Message], system: String) -> Vec<AgentTurn>` and `struct ToolErrorTracker` (`should_disable(&mut self, name) -> bool` on second error):

```rust
#[test]
fn history_turns_shape() {
    let msgs = vec![msg("user", "hi"), msg("assistant", "hello")];
    let turns = build_history_turns(&msgs, "SYS".into());
    assert_eq!(turns[0].role, "system");
    assert_eq!(turns[1].content.as_deref(), Some("hi"));
    assert_eq!(turns.len(), 3);
}

#[test]
fn tool_error_tracker_disables_on_second_failure() {
    let mut t = ToolErrorTracker::default();
    assert!(!t.record_error("kb_search"));
    assert!(t.record_error("kb_search"));   // second → disable
    assert!(!t.record_error("list_docs"));  // independent per tool
}
```

- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement** loop + routing + ChatEvent variants (check the TS union impact lands in Task 7).
- [ ] **Step 4:** `cargo test -p donna-core && cargo test -p donna-server && cargo check --workspace` → PASS (server WS tests must still pass — new variants are additive).
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add agent loop: tool-calling chat with trust gating, caps, and tool events

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 6: Approval response path + RPC arms

**Files:**
- Modify: `crates/donna-core/src/ops.rs` (approvals/policies ops), `donna-server/src/rpc.rs` (4 arms), `donna-server/src/state.rs` or `ws.rs` only if broadcast needs plumbing (it shouldn't — notifications already broadcast via Notifier)
- Test: ops tests + one rpc test in `donna-server/tests/http.rs`

**Interfaces:**
- Produces (ops.rs):
  - `approvals_list(db: &Db) -> Result<Vec<Approval>>` (all, newest first), `approvals_pending_for_conversation(db: &Db, conversation_id: i64) -> Result<Vec<Approval>>`
  - `trust_policies_list(db: &Db) -> Result<Vec<TrustPolicy>>` — IMPORTANT: returns a row for EVERY Outbound tool in the registry (defaulting mode "ask" when absent from the table) so the Settings editor shows the full surface, not just edited rows.
  - `trust_policy_set(db: &Db, action_kind: String, mode: String) -> Result<()>` — validates action_kind is a registry Outbound tool and mode ∈ {ask, auto}.
  - `approval_respond(db: &Db, id: i64, approve: bool) -> Result<String>`:
    - reject → `resolve_approval(id, "rejected")`, insert notification "Action cancelled: <summary>", return "rejected".
    - approve → `resolve_approval(id, "approved")`, then `tools::execute(db, &a.tool, &parse(a.args_json))`. On success: one `providers::complete` call (config from settings; skip if no model configured) with system "You are Donna. The user approved this action and it has now been executed. Write one short confirmation message." + the summary + result → persist as assistant message in `a.conversation_id`, insert notification "Done: <summary>". On execute error: insert notification "Failed: <summary> — <err>", persist assistant message with the failure, return the error text (as Ok — the RPC response carries the outcome either way). `// ponytail: one-shot confirmation, not a loop re-entry; full resume can come later if it ever matters.`
- RPC arms (rpc.rs, follow existing style): `"approvals_list"`, `"approvals_pending_for_conversation"` (`conversationId`), `"approval_respond"` (`id`, `approve`), `"trust_policies_list"`, `"trust_policy_set"` (`actionKind`, `mode`).

- [ ] **Step 1: Failing tests**

```rust
// ops.rs
#[tokio::test]
async fn approval_respond_reject_path() {
    let db = test_db();
    let a = crate::trust::request_approval(&db, 1, "slack_send_message", &serde_json::json!({"channel":"#g","text":"x"})).unwrap();
    let out = approval_respond(&db, a.id, false).await.unwrap();
    assert_eq!(out, "rejected");
    assert_eq!(db.get_approval(a.id).unwrap().unwrap().status, "rejected");
}

#[tokio::test]
async fn trust_policies_list_covers_all_outbound() {
    let db = test_db();
    let rows = trust_policies_list(&db).unwrap();
    assert_eq!(rows.len(), 3); // slack, telegram, whatsapp
    assert!(rows.iter().all(|p| p.mode == "ask"));
}
```

```rust
// donna-server/tests/http.rs — arm smoke
#[tokio::test]
async fn rpc_trust_policies_roundtrip() {
    let app = donna_server::build_app(donna_server::test_state());
    let res = /* POST /rpc/trust_policy_set {"actionKind":"slack_send_message","mode":"auto"} with bearer */;
    assert_eq!(res.status(), StatusCode::OK);
    let body = /* POST /rpc/trust_policies_list {} */;
    /* assert the slack row shows "auto" */
}
```

(Write the http test fully in the existing oneshot style.)

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4:** `cargo test --workspace && cargo check --workspace` → PASS.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add approval response path and trust/approval RPC arms

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 7: Chat UI — tool status + approval cards

**Files:**
- Modify: `src/lib/api.ts` (ChatEvent union + Approval/TrustPolicy types + api methods), `src/routes/Chat.tsx`
- Verify: `npx tsc --noEmit`, `npm run build`

**Interfaces:**
- api.ts:
  - `ChatEvent` union += `{ type: "tool"; name: string; label: string; status: "running" | "done" | "error" } | { type: "approval"; approval_id: number; summary: string; tool: string }` (fields snake_case per the Rust serde — match exactly).
  - `Approval { id, conversationId, tool, argsJson, summary, status, createdAt }` + `toApproval` raw mapping (snake_case → camel, like toNotification).
  - Methods: `approvalsPendingForConversation(conversationId)`, `approvalRespond(id, approve)`, `trustPoliciesList()`, `trustPolicySet(actionKind, mode)`.
  - The done/error terminal checks in api.ts `sendChat`/server.ts stay UNCHANGED (tool/approval are non-terminal).
- Chat.tsx:
  - New state: `toolEvents: {name,label,status}[]` (reset on each send), `pendingApprovals: Approval[]`.
  - onEvent: `token` as today; `tool` → upsert into toolEvents by name+label (running→done/error transitions replace); `approval` → refetch `approvalsPendingForConversation(activeId)`.
  - Render: during streaming, above/inside the streaming bubble show a compact tool-status list (`<Spinner/>` while running, check/cross when done/error, label text — reuse existing Tailwind tokens `border-white/10 bg-donna-surface`, text-xs). After messages reload (post-done), also refetch pendingApprovals for the conversation.
  - Approval card component (below the last message when pendingApprovals non-empty): summary text + Approve / Reject `Button`s → `api.approvalRespond(id, bool)` → optimistic remove + `loadMessages(activeId)` after a short delay (the confirmation message is persisted server-side) + refetch approvals. Style like the unread-notification card (`border-donna-accent/30 bg-donna-accent/5`).
  - On conversation switch/load: fetch pendingApprovals alongside messages.

- [ ] **Step 1: Implement api.ts types/methods.** **Step 2: Implement Chat.tsx** per above. **Step 3:** `npx tsc --noEmit && npm run build` clean. **Step 4:** report a 6-line manual smoke checklist for the controller (chat with a Read tool question e.g. "what's on HN?", watch tool line; ask Donna to send a Slack message → approval card → Approve → confirmation message appears).
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Chat UI: live tool status lines and DB-backed approval cards

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 8: Settings policy editor + system prompt + docs

**Files:**
- Modify: `src/routes/Settings.tsx` (Autonomy card extension), `crates/donna-core/src/ops.rs` (system prompt addendum), `docs/ROADMAP.md`
- Verify: tsc/build + cargo check

**Interfaces / content:**
- Settings.tsx: extend the existing Autonomy section (radio cards stay) with an "Outbound actions" sub-list: rows from `api.trustPoliciesList()` — tool name (prettified), ask/auto segmented toggle, IMMEDIATE save via `api.trustPolicySet` on change (Server-card semantics, not the config Save button; note in a caption: "Actions set to ask show an approval card before Donna acts.").
- System prompt (ops.rs DONNA_SYSTEM_PROMPT or build_system_prompt addendum, gated to the agent path): a short "## Acting with tools" section — you have tools; use them rather than describing; outbound actions may require user approval — when a tool returns PENDING_APPROVAL, tell the user you've asked for their approval and STOP pursuing that action; never fabricate tool results.
- ROADMAP.md: check the Phase 2 items under the Phase 7/spec section (agent loop, tool registry, trust engine, approvals) — leave Phases 3-6 unchecked.

- [ ] **Step 1: Implement all three.** **Step 2:** `npx tsc --noEmit && npm run build && cargo check --workspace && cargo test --workspace` clean/green.
- [ ] **Step 3: Commit and push**

```bash
git add -A
git commit -m "Settings outbound-policy editor, agent system-prompt addendum, roadmap sync

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

## Done criteria (whole phase)

1. `cargo test --workspace` green; `npx tsc --noEmit` + `npm run build` clean.
2. With provider=openai + key: chatting "what's on Hacker News?" triggers `news_top_stories` (tool status line visible) and answers from the result — no approval.
3. "Send 'hello' to #general on Slack" produces an approval card + notification; Approve executes and a confirmation message lands in the conversation; Reject cancels with a notification.
4. Settings shows the 3 outbound tools defaulting to ask; flipping one to auto makes the same request execute without asking.
5. Non-OpenAI providers chat exactly as before (plain path untouched).
6. Approvals history rows accumulate with statuses (fuel for Phase 4 graduation).

## Follow-ups noted during planning (not in scope)

- Automatic graduation/demotion → Phase 4 suggestion queue (approvals history already recorded).
- quick_chat_send stays tool-less this phase.
- `notion_list_pages` has no query param upstream (lists pages); rename in registry description to avoid model confusion.
- Reminder sweep uses the scheduler's tick `now` (respects the timezone setting automatically).
