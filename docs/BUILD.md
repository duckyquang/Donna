# Building Donna

Donna is a [Tauri 2](https://v2.tauri.app/) desktop app: React + TypeScript frontend, Rust backend.

## Prerequisites

- **Node.js** 20+
- **Rust** stable (`rustup`)
- **Ollama** (optional, for local models and embeddings)

Platform-specific Tauri dependencies:

| OS | Install |
|----|---------|
| macOS | Xcode Command Line Tools |
| Linux | `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf` |
| Windows | Microsoft C++ Build Tools, WebView2 |

See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for details.

## Development

```bash
npm install
npm run sidecar   # build the donna-server sidecar once (rerun after server changes)
npm run tauri:dev
```

Tauri refuses to start if the sidecar binary is missing — rerun `npm run sidecar` after pulling server changes.

The knowledge base lives in `knowledge-base/` at the repo root. SQLite and settings are stored in the OS app data directory.

## Production build

```bash
npm run tauri:build
```

Installers are written to `src-tauri/target/release/bundle/`.

## Embeddings (optional)

For semantic memory retrieval with Ollama:

```bash
ollama pull nomic-embed-text
```

Set the embedding model in **Settings** (default: `nomic-embed-text`). Donna indexes nodes automatically after curation; run a full reindex via the `kg_reindex_embeddings` command if needed.

## Release builds (CI)

Tagged releases (`v*`) trigger `.github/workflows/release.yml`, which builds macOS (Apple Silicon + Intel), Linux, and Windows installers and attaches them to a GitHub Release draft.

```bash
git tag v0.1.0
git push origin v0.1.0
```
