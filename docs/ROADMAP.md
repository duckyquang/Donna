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
