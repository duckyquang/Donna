# Donna: server-first proactive assistant — design

**Date:** 2026-07-07
**Status:** approved by owner (buno) section-by-section in brainstorming session
**Scope:** the next major evolution of Donna — from a desktop chat app with routines into a 24/7 proactive agent reachable by text and voice.

## 1. Goal

Donna becomes a Jarvis/Suits-Donna-class personal assistant:

- **Does things**, not just says things — she can invoke integrations herself.
- **Proactive** — acts and reminds without being prompted, 24/7, laptop closed.
- **Learns patterns** — notices recurring asks and proposes automations (Hermes-agent style, consent-first).
- **Reachable anywhere** — two-way WhatsApp (text + voice notes) and a desktop voice mode with a female voice.
- **Routes her own skills** — knows which integration/skill fits a request; can author her own skills.
- **Easy to grant access** — setup optimized for the owner ("me first, OSS second").

## 2. Decisions (locked)

| Fork | Decision |
|---|---|
| Brain location | **Server-first**: donna-server becomes the real Donna; desktop app is her face |
| Audience | **Owner first, OSS second** — setup guides may assume a technical owner |
| Voice surfaces | **Desktop voice mode + WhatsApp voice notes** (no phone calls for now) |
| Model stack | **OpenAI end-to-end**: GPT for the agent loop, Whisper STT, OpenAI TTS, text-embedding-3-small; mini model for background review. Provider layer stays swappable |
| Autonomy | **Earned autonomy**: outbound actions start as ask-first proposals; repeated approvals graduate an action kind to autonomous, via explicit consent |
| Build approach | **Rust workspace**: extract `donna-core` crate from src-tauri; grow donna-server into an axum agent host |

## 3. Architecture

Cargo workspace, three members:

```
donna/
  crates/donna-core/   # extracted from src-tauri: integrations (11), oauth,
                       # providers, db, knowledge base, embeddings, retrieval,
                       # scheduler logic, agent loop, tool registry, trust engine
  donna-server/        # axum: REST + WebSocket API, WhatsApp webhook,
                       # scheduler host, agent host, approval + suggestion queues.
                       # Owns donna.sqlite and knowledge-base/.
  src-tauri/ + src/    # Tauri shell + React UI. api.ts swaps invoke() for
                       # fetch/WS + bearer token. Native-only duties: Cmd+D
                       # quick-chat overlay + screenshot, mic capture,
                       # native notification display.
```

- **SecretStore trait** in donna-core: macOS keychain impl (desktop), 0600 `.env`/file impl (server). Secrets live where the brain lives.
- **Embeddings** gain an OpenAI backend (text-embedding-3-small) with one-time reindex — fixes semantic recall being Ollama-only.
- **Deployment:** single Docker compose — donna-server + `cloudflared` sidecar (Cloudflare Tunnel gives the WhatsApp webhook public HTTPS with no open ports). Target: Oracle free-tier ARM, or a home Mac mini / Raspberry Pi.
- **Migration:** one-shot `donna-server import` ingests the desktop `donna.sqlite` + `knowledge-base/`.
- **Desktop offline behavior:** "Donna is unreachable" banner. One brain, one source of truth — no second local brain.

Explicitly skipped: multi-user auth, TLS termination config, and any offline/local fallback brain (the banner above is the entire offline story).

## 4. Agent core

**Loop** (`donna-core::agent`): OpenAI chat completions with `tools`. Build messages → call model → execute tool calls → feed results back → repeat until final answer. Caps: 12 iterations, per-run token budget. Tool errors are returned to the model once for self-correction, then surfaced.

**Sessions:** every channel is a session over the existing `messages` table — desktop conversations, the WhatsApp thread (rolling, resets after ~6h idle), each routine run. System prompt ordered stable-first (persona + tools → skills → volatile memory snapshot + timestamp) for OpenAI prefix-cache economics.

**Tool registry:** compile-time Rust inventory. Each tool: name, description, JSON schema, async handler, **risk class**:

- `Read` — calendar list, gmail read, searches, status pulls → always auto.
- `Write` — KB save, drafts, reminders, own-calendar CRUD → always auto.
- `Outbound` — send email / Slack / WhatsApp to others, calendar invites → trust engine.

~30 tools at launch, mostly thin wrappers over existing integration functions, plus new: `session_search` (FTS5 over all messages), `memory_update` (capped USER.md/MEMORY.md editor), `remember` (one-shot reminders), `suggest_automation`, `skills_list` / `skill_view` / `skill_create`.

**Trust engine:** policy table keyed by action kind, default `ask`. Ask = approval row + push (WhatsApp interactive buttons or desktop notification: Approve / Reject / Edit). 5 consecutive approvals of one kind → Donna files a suggestion to graduate it to `auto` (act, then tell). Any rejection while auto → immediately back to `ask`. Policy table visible/editable in Settings → Autonomy.

**Skills:** `skills/` folder of `SKILL.md` files (agentskills.io format). Progressive disclosure: `skills_list` returns names+descriptions only; `skill_view` loads one. Donna self-authors skills when the background review spots a successful multi-step recipe — proposed through the suggestion queue, never silently. Third-party skill installation (hubs) deferred.

## 5. Memory & pattern learning

- **Keep:** knowledge-base/ folder, mind map UI, post-conversation curation — but curation runs **server-side after every session** (closes the gap where WhatsApp/quick-chat sessions bypass memory).
- **Add (Hermes):** `USER.md` (~500-token hard cap: identity, preferences, working style) and `MEMORY.md` (~2,000-token cap: environment, active threads, conventions) at the KB root. Injected into every session. Edited only via `memory_update`, which **errors when full** — forced consolidation, no auto-compaction.
- **Add:** FTS5 index over the full message history; `session_search` tool for exact recall.
- **Events log:** every user request, tool call, and approval recorded with action kind + timestamp.
- **Background review** (mini model, per-session + nightly): curates KB facts, refreshes USER.md, proposes skills, and scans the events log for recurring patterns.
- **Suggestion queue:** the single consent-first surface for all proactivity proposals. Sources: usage patterns, integration connections, a small curated catalog, skill proposals. Surfaced as a Dashboard card + occasional WhatsApp nudge. Accept → creates the routine/policy/skill. Dismiss → latched by dedup key, never re-offered. Nothing ever auto-schedules.
- **Routines become agentic:** they run through the agent loop with tools (trust engine still governs Outbound) and adopt the `[SILENT]` convention — a scheduled check that finds nothing says nothing.

## 6. Channels & voice

**WhatsApp (two-way):** Meta Cloud API webhook (verify + receive) on donna-server via the tunnel. Allowlist = owner's number only; others silently ignored. Inbound deduped by message id (Meta retries deliveries). Text → agent session → reply. **Voice notes:** download media → Whisper → agent → reply in kind (voice note back via OpenAI TTS → OGG upload; text gets text). Approvals as WhatsApp interactive buttons.

**Desktop voice mode:** push-to-talk (hold hotkey or click mic) → audio to server → Whisper → agent → streamed text + TTS audio together. Voice picked from OpenAI's female voices in Settings. V1 is half-duplex: no wake word, no barge-in (click to stop playback). **Build-time verification:** mic capture in the Tauri WKWebView needs the macOS mic entitlement; if the webview refuses, fall back to native capture via `cpal` — same UX.

**Notifications:** server pushes over WebSocket to desktop (displayed natively); WhatsApp when away.

## 7. Error handling

Fixes three existing silent-failure bugs, plus new-surface handling:

1. Outbound sends check HTTP status (today WhatsApp/Telegram failures are invisible); 3 retries with backoff, then dead-letter to notifications.
2. Scheduler dedupe persisted in DB (today in-memory flags double-send after restart).
3. Timezone from a setting via chrono-tz (already a dependency, unused) instead of server-local.
4. Webhook idempotency via message-id dedupe table.
5. Agent loop: iteration + token caps; tool error → one model self-correction → surface to owner with context.

## 8. Testing

Lean, `cargo test` only:

- Unit: trust-engine transitions (5-approvals graduation, reject-while-auto demotion), tool schema generation, WhatsApp webhook parsing against recorded fixtures, FTS search, scheduler dedupe.
- One agent-loop integration test with mocked OpenAI (canned tool_call responses).
- Manual smoke checklist per channel (WhatsApp text, WhatsApp voice note, desktop voice round-trip).

## 9. Build order

Six phases, each independently shippable. Implementation planning proceeds one phase at a time — each phase gets its own plan against this spec:

1. **Foundation** — workspace extraction, server API + WS, desktop-as-client, data migration, Docker + tunnel. Donna functionally identical, now 24/7.
2. **Hands** — agent loop, tool registry, trust engine, approvals. Donna can do things.
3. **Reach** — WhatsApp two-way text with approval buttons.
4. **Growth** — USER.md/MEMORY.md, FTS5, events log, background review, suggestion queue, agentic `[SILENT]` routines.
5. **Voice** — desktop voice mode, then WhatsApp voice notes.
6. **Craft** — skills system (list/view/create through suggestions).

Deferred: phone-call voice (Twilio), third-party skill installation, hosted OAuth / non-technical setup, multi-user.

## 10. Follow-ups noted during design

- `CONTEXT.md` / `docs/ROADMAP.md` stop at Phase 4 and contradict README on WhatsApp status — update as part of Phase 1 docs.
- donna-server README references a "Settings > Export Google Token" desktop feature that does not exist; Phase 1's migration command supersedes it.
- `embeddings.rs::spawn_reindex` is dead code (no callers) — remove or wire up during extraction.
