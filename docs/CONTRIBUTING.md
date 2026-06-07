# Contributing to Donna

Thank you for helping build Donna — a local-first AI personal assistant.

## Getting started

1. Fork and clone the repo.
2. Follow [BUILD.md](./BUILD.md) to run `npm run tauri:dev`.
3. Read [CONTEXT.md](../CONTEXT.md) for product goals and architecture.

## Project layout

| Path | Purpose |
|------|---------|
| `src/` | React UI (routes, components, API client) |
| `src-tauri/src/` | Rust core: commands, knowledge base, integrations, scheduler |
| `knowledge-base/` | User data (folder tree of Markdown nodes) |
| `docs/` | Roadmap and contributor docs |

## Making changes

- **Match existing style** — minimal diffs, no drive-by refactors.
- **Secrets never in git** — API keys and OAuth tokens go in the OS keychain via `secrets.rs`.
- **IPC commands** — add Rust handler in `commands.rs`, register in `lib.rs`, expose in `src/lib/api.ts`.
- **Integrations** — one module per service under `src-tauri/src/integrations/`, status in `integrations/mod.rs`.

## Pull requests

1. Create a focused branch from `main`.
2. Ensure `npm run build` and `cargo check --manifest-path src-tauri/Cargo.toml` pass.
3. Describe what changed and why; include screenshots for UI changes.
4. Link related issues if any.

## Code of conduct

Be respectful and constructive. Donna is open source (MIT).
