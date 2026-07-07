# Donna Roadmap

This roadmap tracks the phased build-out of Donna. See [`../CONTEXT.md`](../CONTEXT.md)
for the full architecture and design rationale behind each item.

## Phase 0 — Foundation ✅
- [x] Project vision and source-of-truth doc (`CONTEXT.md`)
- [x] Beautiful, contributor-friendly `README.md`
- [x] Repository scaffold (Tauri + React + TS + Tailwind)
- [x] Model layer interface with Ollama + cloud provider stubs
- [x] Memory/knowledge-graph client stubs
- [x] Rust core command surface (chat, list_models, init_db)

## Phase 1 — MVP ✅
- [x] Onboarding wizard: pick a local model (Ollama) or paste a cloud API key
- [x] Secure key storage in the OS keychain (Rust core)
- [x] Working Chat with streaming responses (Ollama + OpenAI + Anthropic)
- [x] SQLite persistence for conversations and messages
- [x] Settings: provider/model selection + key management
- [x] App icon source (`src-tauri/icons/icon.png`); run `npm run tauri icon` before
      the first packaged build

## Phase 2 — Integrations ✅
- [x] Google OAuth (loopback + PKCE, bring-your-own client); scopes cover
      Gmail, Calendar, Docs/Drive
- [x] Calendar view with two-way Google Calendar sync (list/create/update/delete)
- [x] Slack connector (read channels, send messages)
- [x] Fathom connector (secure API key + meeting list for routines/docs)
- [x] Integrations Hub UI with per-service connect/disconnect + status
- [x] Gmail read (recent messages in Integrations Hub) + Google Docs create API

## Phase 3 — Proactive routines ✅
- [x] Background scheduler in the Rust core (60s tick)
- [x] Native OS notifications via Tauri notification plugin
- [x] Built-in routines: Morning Briefing, Meeting Briefing, Relationship Reconnect
- [x] Auto-doc generation from routines (local Docs store) + Fathom context when connected
- [x] Notifications UI — enable/disable routines, custom daily routines, notification inbox
- [x] Docs UI — browse and read Donna-generated documents

## Phase 4 — Learning & voice ✅
- [x] Knowledge retrieval — keyword search over the folder-based knowledge base, injected into chat
- [x] Tiered autonomy (confirm → act → autonomous) in Settings, reflected in Donna's system prompt
- [x] User-described routines in natural language (custom routine form in Notifications)
- [x] Memory view — Mind Map + node editor + profile onboarding + knowledge audit in chat

## Cross-cutting features
- [x] Formatted chat output — render Donna's replies as Markdown (bold, lists, code,
      links), including while streaming
- [x] Mind Map / Knowledge Cartography — hierarchical folder visualization with category filter
- [x] Folder-based knowledge base — nested paths, Donna-curated after each chat
- [x] Interactive donna-ask questions with batch numbered answers
- [x] Profile onboarding wizard on first conversation
- [x] Embedding-based retrieval (Ollama/local embeddings) — hybrid keyword + cosine similarity
- [x] More integrations (Notion, Telegram, GitHub, Linear, WhatsApp)
- [x] Cross-platform packaged installers (macOS, Windows, Linux) — CI on version tags
- [x] Docs site and contributor guides — see `docs/`
- [x] Full Gmail compose/draft and Drive file management UI

## Phase 7 — Server-first foundation ✅ (Phase 1 of 6, see spec)

Donna is evolving from a desktop chat app into a 24/7 proactive assistant reachable by
text and voice. Full design:
[`docs/superpowers/specs/2026-07-07-donna-jarvis-design.md`](superpowers/specs/2026-07-07-donna-jarvis-design.md).
The spec's own build order has six phases; this roadmap entry covers Phase 1
(Foundation) of that plan.

- [x] Cargo workspace extraction: `donna-core` crate pulled out of `src-tauri`
      (integrations, oauth, providers, db, knowledge base, embeddings, retrieval,
      scheduler)
- [x] `donna-server` grown into an axum host: RPC dispatcher + WebSocket chat/notify
      streaming, bearer auth, health check
- [x] Desktop app becomes a client of donna-server: `api.ts` swaps Tauri `invoke()`
      for fetch/WS + bearer token; "Donna is unreachable" banner when offline
- [x] Migration bundle: `donna-server import <bundle>` + desktop "Export server
      bundle…" to move an existing desktop-only install onto the server
- [x] Docker: workspace-aware multi-stage build (stubs `src-tauri` so only
      `donna-server` compiles), `docker-compose.yml` with a `cloudflared` tunnel
      sidecar for optional public HTTPS

Phase 2 (Hands) of the spec — done:
- [x] Agent loop, tool registry, trust engine (earned autonomy for outbound actions)
- [x] Approvals end-to-end (WS events, RPC arms, Chat UI cards, Settings policy editor)

Phase 3 (Reach) of the spec — done:
- [x] Two-way WhatsApp: Meta Cloud API webhook (verify + signed receive), owner
      allowlist (`whatsapp_my_number`, set from Integrations), inbound dedup,
      agent-loop replies, approvals as interactive buttons. Voice notes deferred
      to Phase 5. See `donna-server/README.md` for the Meta setup guide.

Phase 4 (Growth) of the spec — done:
- [x] Capped `USER.md` / `MEMORY.md` curated via a `memory_update` tool (errors
      `MEMORY_FULL` past the cap, forcing consolidation), injected into every system prompt
- [x] FTS5 full-text `session_search` over the whole chat history, with one-time backfill
      for messages that predate the index
- [x] Events log (`chat request` / `tool call` / `approval`) recorded at the in-tree
      choke points
- [x] Nightly background review (cheap/configurable model) that curates memory and
      files suggestions from recurring patterns
- [x] Consent-first suggestion queue — Dashboard card with Accept/Dismiss, dismissals
      latched by dedup key, Accept on a `routine` suggestion creates the routine
- [x] `[SILENT]` sentinel for routines — a scheduled check that finds nothing produces
      no doc/notification and doesn't re-fire every tick

Phase 5 (Voice) of the spec — done:
- [x] `/voice/transcribe` (Whisper) and `/voice/speak` (TTS) endpoints, bearer-auth,
      400 without an OpenAI key
- [x] WhatsApp voice notes: inbound audio transcribed, run through the agent loop,
      replied with a synthesized voice note (text fallback on any failure)
- [x] Desktop push-to-talk: mic button in Chat records → transcribes → sends via the
      normal streamed-reply path; "Speak replies aloud" toggle + voice picker in
      Settings; macOS mic entitlement (`NSMicrophoneUsageDescription`)

Phase 6 (Craft) of the spec — done:
- [x] File-based skills catalog (`skills.rs`): `SKILL.md` + frontmatter, traversal-guarded
      save/list/view, seeded example skill so the catalog is never empty
- [x] `skills_list` / `skill_view` / `skill_create` registered as agent tools; every
      system prompt carries a `## Available skills` name+description listing
- [x] Accepting a `kind:"skill"` suggestion saves the skill; the nightly review can
      propose a skill for a recurring recipe
- [x] Skills page (`/skills`) — browse the catalog, view a skill's SKILL.md as
      rendered Markdown

All six spec phases (Foundation, Hands, Reach, Growth, Voice, Craft) are now shipped.
