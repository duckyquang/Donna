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

Not yet built (Phases 2–6 of the spec):
- [ ] Agent loop, tool registry, trust engine (earned autonomy for outbound actions)
- [ ] Two-way WhatsApp (webhook, allowlist, voice notes)
- [ ] `USER.md` / `MEMORY.md`, FTS5 message search, events log, background review,
      suggestion queue, agentic `[SILENT]` routines
- [ ] Voice: desktop push-to-talk mode, then WhatsApp voice notes
- [ ] Skills system (`skills_list` / `skill_view` / `skill_create` via suggestions)
