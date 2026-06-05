# Donna Roadmap

This roadmap tracks the phased build-out of Donna. See [`../CONTEXT.md`](../CONTEXT.md)
for the full architecture and design rationale behind each item.

## Phase 0 — Foundation ✅
- [x] Project vision and source-of-truth doc (`CONTEXT.md`)
- [x] Beautiful, contributor-friendly `README.md`
- [x] Repository scaffold (Tauri + React + TS + Tailwind)
- [x] Model layer interface with Ollama + cloud provider stubs
- [x] Memory/knowledge-graph client stubs
- [x] Rust core command surface (chat, list_models, schedule_routine, init_db)

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
- [x] Fathom connector (secure API key connection; transcript actions land with Docs)
- [x] Integrations Hub UI with per-service connect/disconnect + status
- [ ] Full Gmail/Docs/Drive actions (wired up alongside their features)

## Phase 3 — Proactive routines
- [ ] Background scheduler in the Rust core
- [ ] Native OS notifications (actionable)
- [ ] Built-in routines: Morning Briefing, Meeting Briefing, Relationship Reconnect
- [ ] Auto-doc generation from Fathom meetings and important messages

## Phase 4 — Learning & voice
- [ ] Richer knowledge graph + hybrid (structured + embedding) retrieval
- [ ] Voice/style calibration with tiered autonomy (confirm → act → autonomous)
- [ ] User-described routines in natural language
- [ ] Memory view: browse, edit, and audit what Donna knows

## Cross-cutting features
- [x] Formatted chat output — render Donna's replies as Markdown (bold, lists, code,
      links), including while streaming
- [x] Mind Map / Knowledge Cartography — node-based, clustered visualization of the
      knowledge graph, continuously updated by Donna, with click-to-reveal notes per node
- [ ] Editable nodes/notes directly from the Mind Map
- [ ] Embedding-based retrieval over the knowledge graph

## Ongoing
- [ ] More integrations (Notion, Telegram, GitHub, Linear, …)
- [ ] Cross-platform packaged installers (macOS, Windows, Linux)
- [ ] Docs site and contributor guides
