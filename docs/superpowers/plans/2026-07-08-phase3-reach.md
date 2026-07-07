# Phase 3: Reach — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two-way WhatsApp: text Donna from your phone and she answers through the agent loop in a rolling session; approvals arrive as WhatsApp interactive buttons and button replies resolve them.

**Architecture:** A `webhook.rs` module in donna-server exposes `GET/POST /webhook/whatsapp` OUTSIDE the bearer-auth layer (Meta can't send bearer tokens): GET is the Meta verify handshake against `DONNA_WA_VERIFY_TOKEN`; POST verifies `X-Hub-Signature-256` (HMAC-SHA256 with `DONNA_WA_APP_SECRET`), dedupes by message id, allowlists the owner's number, and spawns async handling (Meta needs a fast 200). Inbound text joins a rolling "WhatsApp" conversation (6h idle reset) and runs through the existing `ops::send_chat` (which routes to the agent loop); the final assistant message is sent back via the existing `whatsapp::send_message`. Approval requests additionally push interactive Approve/Reject buttons to the owner's number; button replies come back through the webhook into `ops::approval_respond`.

**Tech Stack:** existing + ONE new dependency: `hmac = "0.12"` (RustCrypto; `sha2` already in workspace). Signature verification is security-critical — never hand-rolled.

## Global Constraints

- Spec §6 (WhatsApp two-way) + §7.4 (webhook idempotency). TEXT ONLY this phase — voice notes are Phase 5; non-text inbound gets a polite one-line reply.
- **Commit AND push after every task.** Branch `feat/phase-5-projects-discord-proactive`, no PRs.
- Server env additions (env-only config per Phase 1): `DONNA_WA_VERIFY_TOKEN` (user-invented string pasted into the Meta dashboard) and `DONNA_WA_APP_SECRET` (the Meta app secret). **POST processing requires the app secret**: if unset, respond 200 but ignore the body and log a warning once per request (secure by default, no retry storms). Bad signature → 401.
- Allowlist: inbound `from` must equal setting `whatsapp_my_number` (compare digits-only: strip `+` and non-digits from both). Non-allowlisted senders → 200, silently ignored (spec).
- Dedupe: new table `webhook_events (id TEXT PRIMARY KEY, created_at TEXT NOT NULL)`; claim via INSERT OR IGNORE before processing. `// ponytail: no pruning — one row per inbound message is nothing; add a sweep if it ever matters.`
- Rolling session: setting `whatsapp_conversation_id`; reuse while the conversation's last message is < 6 hours old (RFC3339 compare — both sides are `now_iso()` UTC), else create a new conversation titled "WhatsApp".
- Webhook route MUST be registered after `.layer(require_bearer)` in build_app (alongside /health) — and a test must prove no bearer is needed.
- Meta interactive buttons: max 3 buttons, titles ≤ 20 chars, body ≤ 1024 chars (truncate summary). Button ids: `approve:<approval_id>` / `reject:<approval_id>`.
- Webhook POST must return 200 fast: heavy work (agent turn) runs in `tokio::spawn`; the dedupe claim happens BEFORE spawning.

---

### Task 1: Dedupe table, session logic, button sender

**Files:**
- Modify: `crates/donna-core/src/db.rs` (webhook_events + try_claim), `crates/donna-core/src/ops.rs` (session fn), `crates/donna-core/src/integrations/whatsapp.rs` (buttons)
- Test: inline

**Interfaces:**
- `Db::try_claim_webhook_event(&self, id: &str) -> Result<bool>` — `INSERT OR IGNORE INTO webhook_events (id, created_at) VALUES (?1, ?2)`; returns `changes() == 1`. Table appended to migrate() batch.
- `ops::whatsapp_session_conversation(db: &Db) -> Result<i64>` — read setting `whatsapp_conversation_id`; if it parses and the conversation exists and its LAST message's created_at is < 6h before now (no messages yet ⇒ fresh, reuse), return it; else `db.create_conversation("WhatsApp")`, store the new id in the setting, return it. Extract pure `fn session_is_fresh(last_message_at: &str, now: &str) -> bool` (RFC3339 parse, < 6h).
- `whatsapp::send_approval_buttons(to: &str, approval_id: i64, summary: &str) -> Result<()>` — POST the same Graph endpoint with:
```json
{
  "messaging_product": "whatsapp",
  "to": "<digits>",
  "type": "interactive",
  "interactive": {
    "type": "button",
    "body": { "text": "Approval needed:\n<summary truncated to ~900 chars>" },
    "action": { "buttons": [
      { "type": "reply", "reply": { "id": "approve:<id>", "title": "Approve" } },
      { "type": "reply", "reply": { "id": "reject:<id>",  "title": "Reject" } }
    ]}
  }
}
```
Same status-check/error style as send_message. Extract pure `fn approval_buttons_body(to_digits: &str, approval_id: i64, summary: &str) -> serde_json::Value` and test it (ids, truncation, titles).

- [ ] **Step 1: Failing tests**

```rust
// db.rs
#[test]
fn webhook_event_claim_once() {
    let db = test_db();
    assert!(db.try_claim_webhook_event("wamid.A1").unwrap());
    assert!(!db.try_claim_webhook_event("wamid.A1").unwrap());
    assert!(db.try_claim_webhook_event("wamid.A2").unwrap());
}
```

```rust
// ops.rs
#[test]
fn whatsapp_session_reuses_fresh_creates_stale() {
    let db = test_db();
    let c1 = whatsapp_session_conversation(&db).unwrap();
    db.add_message(c1, "user", "hi").unwrap();
    assert_eq!(whatsapp_session_conversation(&db).unwrap(), c1); // fresh → reuse
    // age the last message 7h via raw SQL (created_at is TEXT RFC3339)
    let old = (chrono::Utc::now() - chrono::Duration::hours(7)).to_rfc3339();
    db.0.lock().unwrap().execute("UPDATE messages SET created_at = ?1", rusqlite::params![old]).unwrap();
    let c2 = whatsapp_session_conversation(&db).unwrap();
    assert_ne!(c2, c1); // stale → new conversation
    assert_eq!(whatsapp_session_conversation(&db).unwrap(), c2); // sticky
}

#[test]
fn session_freshness_boundary() {
    let now = "2026-01-01T12:00:00+00:00";
    assert!(session_is_fresh("2026-01-01T07:00:00+00:00", now));   // 5h
    assert!(!session_is_fresh("2026-01-01T05:00:00+00:00", now));  // 7h
}
```

```rust
// whatsapp.rs
#[test]
fn approval_buttons_shape_and_truncation() {
    let v = approval_buttons_body("15550100", 42, &"x".repeat(2000));
    assert_eq!(v["interactive"]["action"]["buttons"][0]["reply"]["id"], "approve:42");
    assert_eq!(v["interactive"]["action"]["buttons"][1]["reply"]["id"], "reject:42");
    assert!(v["interactive"]["body"]["text"].as_str().unwrap().len() <= 1024);
    assert!(v["interactive"]["action"]["buttons"][0]["reply"]["title"].as_str().unwrap().len() <= 20);
}
```

- [ ] **Step 2: RED** — `cargo test -p donna-core webhook_event whatsapp_session session_fresh approval_buttons` → FAIL.
- [ ] **Step 3: Implement** per interfaces (session: last message = `get_messages(id)?.last()`; missing/unparseable setting or vanished conversation ⇒ create new).
- [ ] **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`, zero new warnings.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add webhook dedupe, rolling WhatsApp session, and approval button sender

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 2: Inbound handlers in ops + approval push from the agent

**Files:**
- Modify: `crates/donna-core/src/ops.rs` (two handlers), `crates/donna-core/src/agent.rs` (approval push)
- Test: inline

**Interfaces:**
- `ops::whatsapp_handle_text(db: &Db, text: &str) -> Result<()>`:
  1. `let conv = whatsapp_session_conversation(db)?;`
  2. `db.add_message(conv, "user", text)?;`
  3. Run the existing brain: `send_chat(db, conv, &on_event).await` where on_event captures only Error messages into an `Arc<Mutex<Option<String>>>` (all other events ignored — no streaming over WhatsApp). Record the last assistant message id BEFORE the call.
  4. After: if a NEW assistant message exists → `whatsapp::send_message(my_number, &content)` (my_number from setting; if unset, log-and-return Ok — nothing to reply to). Else send the captured error text or a stock "I hit a problem handling that." Send failures: log, return Ok (webhook must not error).
- `ops::whatsapp_handle_button(db: &Db, button_id: &str) -> Result<()>`:
  1. Pure `fn parse_button_id(id: &str) -> Option<(bool, i64)>` — `approve:42` → `(true, 42)`, `reject:7` → `(false, 7)`, anything else None. TDD.
  2. None → send "I didn't recognize that button." (best-effort) and Ok.
  3. Some → `approval_respond(db, id, approve).await` → send outcome over WhatsApp: fetch the approval's summary; approved → "Done: <summary>" / "failed: e" → "That failed: <e>" / rejected → "Cancelled: <summary>" / "already resolved" → "Already handled." (approval_respond already persists the in-conversation message + notification; this is just the WhatsApp echo).
- `agent.rs`: in the Ask branch, after `request_approval` + the Approval event emit: best-effort push — if `whatsapp::is_connected()` and setting `whatsapp_my_number` present → `let _ = whatsapp::send_approval_buttons(&num, a.id, &a.summary).await;`. Note: request_approval DEDUPES pending rows — only push buttons when the returned approval was newly created this call; add a `created: bool` to what request_approval returns OR compare created_at freshness — simplest correct: have `trust::request_approval` return `(Approval, bool /*newly_created*/)` and adjust its two call sites + tests. Push only when newly_created (prevents button spam on model retries).

- [ ] **Step 1: Failing tests** — `parse_button_id` (4 cases incl. garbage + negative id string); `whatsapp_handle_button` reject path end-to-end with a seeded approval (no network: use a tool whose args make summarize deterministic; the WhatsApp echo send will fail without creds — the fn must swallow send failures and still resolve the approval; assert approval status == "rejected" after); request_approval's new tuple return (dedupe test updated: second identical call → `newly_created == false`).
- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add WhatsApp inbound handlers and approval-button push from the agent loop

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 3: Webhook endpoints in donna-server

**Files:**
- Create: `donna-server/src/webhook.rs`
- Modify: `donna-server/src/lib.rs` (routes + mod), `donna-server/src/state.rs` (AppState: `wa_verify_token: Option<String>`, `wa_app_secret: Option<String>`; test_state sets `Some("test-verify".into())` / `Some("test-secret".into())`), `donna-server/src/main.rs` (read the two env vars), `donna-server/Cargo.toml` + workspace root (`hmac = "0.12"`)
- Test: `donna-server/tests/webhook.rs`

**Interfaces:**
- `GET /webhook/whatsapp?hub.mode=subscribe&hub.verify_token=X&hub.challenge=Y` → if `Some(X) == state.wa_verify_token` → 200 body Y (plain text); else 403. (Query keys contain dots — use `Query<HashMap<String, String>>`.)
- `POST /webhook/whatsapp` (raw `axum::body::Bytes` + headers):
  1. `state.wa_app_secret` None → log warning, return 200 (body ignored).
  2. Verify `X-Hub-Signature-256: sha256=<hex>` — HMAC-SHA256(app_secret, raw_body), constant-time compare via `hmac`'s `verify_slice`. Missing/bad → 401.
  3. Parse JSON. For each `entry[].changes[].value`:
     - `messages[]` (may be absent — `statuses[]` deliveries are ignored): per message —
       a. `db.try_claim_webhook_event(&msg.id)` false → skip (Meta retry).
       b. Allowlist: digits-only compare of `msg.from` vs setting `whatsapp_my_number` → mismatch/unset → skip silently.
       c. By `msg.type`: `"text"` → spawn `ops::whatsapp_handle_text(db, text.body)`; `"interactive"` with `interactive.button_reply` → spawn `ops::whatsapp_handle_button(db, button_reply.id)`; anything else → spawn best-effort send "I can only read text messages right now."
  4. Return 200 `{"status":"ok"}` immediately (spawned work proceeds).
- Routes registered AFTER the auth layer in build_app:
```rust
.route("/health", get(|| async { "ok" }))
.route("/webhook/whatsapp", get(webhook::verify).post(webhook::receive))
```

- [ ] **Step 1: Failing tests** (`donna-server/tests/webhook.rs`, oneshot style; compute real HMACs with the test secret):

```rust
fn sign(body: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"test-secret").unwrap();
    mac.update(body.as_bytes());
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}
```
(If `hex` isn't already a dep, encode manually with format!("{:02x}") over bytes — do NOT add a dep for hex.)

Cases (each a `#[tokio::test]` against `build_app(test_state())`, NO bearer header anywhere — that itself proves the routes are outside auth):
  1. GET verify: correct token → 200, body == challenge. Wrong token → 403.
  2. POST text message (realistic Meta payload: `{"entry":[{"changes":[{"value":{"messages":[{"from":"15550100","id":"wamid.T1","type":"text","text":{"body":"hello"}}]}}]}]}`) with valid signature, after setting `whatsapp_my_number` = "+1 555 0100" in the test DB → 200; then poll briefly (≤2s) and assert a "WhatsApp" conversation exists with one user message "hello" (the reply attempt fails without creds — that's fine, handle_text swallows it).
  3. Duplicate: same POST again → 200, still exactly ONE user message (dedupe).
  4. Bad signature → 401 and NO message row.
  5. Non-allowlisted `from` ("19998887777") with valid signature → 200, no message row.
  6. Button reply payload (`type":"interactive","interactive":{"type":"button_reply","button_reply":{"id":"reject:<seeded approval id>","title":"Reject"}}`) with valid signature → 200; poll and assert the seeded approval's status becomes "rejected".

- [ ] **Step 2: RED** → **Step 3: Implement** (serde structs for the payload subset; unknown fields ignored via `#[serde(default)]`/Option everywhere — Meta payloads are deep and evolving).
- [ ] **Step 4: GREEN** — `cargo test --workspace && cargo check --workspace`, zero new warnings.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add WhatsApp webhook: verify handshake, signed inbound, dedupe, allowlist, button replies

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 4: Setup surface + docs

**Files:**
- Modify: `src/routes/Integrations.tsx` (WhatsApp card gains a "Your WhatsApp number" field bound to setting `whatsapp_my_number` via existing get/save-config-style RPC — check how other settings fields persist; a `save_config`-adjacent generic `set_setting` RPC may not exist, so reuse whatever the card already does for credentials and add the number to the SAME flow — read the file first; if the card has no settings-write path, add a tiny `whatsapp_set_my_number` ops fn + RPC arm), `donna-server/.env.example` + `donna-server/README.md` (webhook section: invent DONNA_WA_VERIFY_TOKEN, paste app secret, Meta dashboard steps: callback URL = https://<tunnel-host>/webhook/whatsapp, subscribe to `messages` field), `docs/ROADMAP.md` (check Phase 3 items)
- Verify: tsc/build + cargo check + full test suite

- [ ] **Step 1: Implement all three.**
- [ ] **Step 2:** `npx tsc --noEmit && npm run build && cargo test --workspace && cargo check --workspace` — all clean/green.
- [ ] **Step 3: Commit and push**

```bash
git add -A
git commit -m "WhatsApp setup surface: owner number field, webhook env + Meta guide, roadmap

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

## Done criteria (whole phase)

1. `cargo test --workspace` green (incl. 6 webhook integration tests); tsc/build clean.
2. GET verify handshake works against the configured token; POST requires a valid Meta signature (401 otherwise) and processes only the owner's number.
3. Texting Donna (via a signed webhook POST) lands a user message in the rolling "WhatsApp" conversation, runs the brain, and attempts a reply via whatsapp::send_message.
4. An agent-initiated Outbound ask pushes Approve/Reject buttons to the owner's number (when configured); a `reject:<id>` button reply resolves the approval and echoes the outcome.
5. Duplicate webhook deliveries are processed exactly once.
6. Docs: README explains the full Meta setup (tunnel URL, verify token, app secret, subscribe to messages); .env.example carries the two new vars.

## Follow-ups noted during planning (not in scope)

- Voice notes (Phase 5) — non-text inbound gets a polite text reply for now.
- Read receipts/typing indicators — cosmetic, skip.
- webhook_events pruning — ponytail ceiling noted in code.
- Approval buttons only push for NEWLY created approvals (dedupe-aware) — request_approval's return gains a `newly_created` flag in Task 2.
