# Packaged desktop app + landing page — design

**Date:** 2026-07-08
**Status:** Approved

## Problem

Donna is server-first: the desktop app is a thin client of `donna-server`, which users
must run themselves via Docker Compose (plus optional Cloudflare Tunnel). Every step in
the README's setup — running the server, installing Ollama, pulling a model from a
terminal — is a wall for non-technical users. The goal: a friend clicks **Download** on
a landing page, opens Donna like any other app, and everything else happens inside the
app.

## Goals

- One downloadable installer per platform (macOS arm64/x64, Windows, Linux); no npm,
  no Docker, no terminal, ever, for the default path.
- All README setup absorbed into the app: server, brain (local model or API key),
  integrations.
- Auto-updates so friends never re-download from the site.
- A landing page (ChronoTask-inspired: light, clean, floating UI cards, no emojis,
  real image/SVG assets) with OS-aware download buttons.

## Non-goals

- Code signing (macOS notarization / Windows cert). Deferred; the landing page and
  first-run docs carry a 10-second "Open Anyway" guide instead.
- Multi-user / hosted server. `donna-server` stays single-user.
- 24/7 proactivity for embedded-mode users beyond what a running (or tray-resident)
  app provides. Remote self-hosting remains the power-user answer.

## Decisions (from brainstorming)

| Question | Decision |
|---|---|
| How does the packaged app get its brain? | **Embedded sidecar**: bundle `donna-server` in the app, spawn on launch. Remote server stays an Advanced option. |
| Code signing? | **Unsigned for now**; landing page + first-run note explain Gatekeeper/SmartScreen. |
| Landing page hosting? | **GitHub Pages from this repo** (`site/` folder, Actions deploy). |
| Ollama setup? | **Fully automatic**: Donna downloads the portable Ollama runtime herself and manages `ollama serve`; API-key path (OpenAI/Anthropic/Google) kept as the alternative. |

## Architecture

### 1. Embedded server (sidecar)

- CI builds `donna-server` per target triple and places it at
  `src-tauri/binaries/donna-server-<triple>`; `tauri.conf.json` lists it under
  `bundle.externalBin`.
- On launch, the Rust shell:
  1. Loads or generates a bearer token (random UUID, file in the app-data dir, 0600).
  2. Picks port **8377**, falling back to a free ephemeral port if taken (e.g. the
     user also runs a Docker server locally).
  3. Spawns the sidecar with `DONNA_TOKEN`, `DONNA_PORT`, and
     `DONNA_DATA_DIR=<app_data_dir>/server`.
  4. Polls `/health` until ready (with a visible "starting Donna's brain…" state in
     the UI if it takes more than ~1 s), then exposes `{url, token}` to the webview
     via a Tauri command.
- The frontend auto-bootstraps `localStorage` server config from that command **only
  when no config exists yet** — existing installs pointing at a remote server are
  untouched. Settings keeps an "Advanced: remote server" card (URL + token + test
  connection), and a "Use built-in server" reset.
- **Tray keep-alive:** closing the window hides to the menu bar/tray (server keeps
  running, routines keep firing); the tray menu has Open Donna / Quit. Quit (and app
  exit generally) terminates the sidecar cleanly.
- Sidecar crash → shell restarts it (bounded retries), UI shows the existing
  "unreachable" banner in between.

### 2. Onboarding rework (brain step)

Two cards, same as today's provider step, but the local path is now hands-free:

- **Free & private (Ollama, default):**
  1. If no Ollama is reachable at the configured host, Donna downloads a **pinned**
     portable Ollama runtime for the current OS/arch from Ollama's GitHub releases
     into `<app_data_dir>/ollama/` (progress bar), and manages `ollama serve` as a
     child process on a private port with `OLLAMA_ORIGINS` set for the webview. The
     wizard writes that managed host into the `ollamaHost` setting so `donna-server`
     uses it.
  2. Donna pulls the recommended default model (`qwen2.5:3b` — small enough for
     consumer laptops; the exact tag is a one-line constant) via Ollama's
     `/api/pull`, streaming a real progress bar in the wizard.
  3. Failure fallback at any step: a "Get Ollama instead" button linking to
     ollama.com/download, with detection that flips the wizard forward once Ollama
     appears. If Ollama is already installed/running, skip straight to model pull
     (or model list if models exist).
- **Bring your own key:** unchanged — paste an OpenAI/Anthropic/Google key.
- No server URL/token screen in the default flow. Provider switchable in Settings, as
  today.
- The runtime manager lives in the Tauri shell (it's a client-machine concern;
  `donna-server` keeps talking to whatever `ollamaHost` it's given, unchanged).

### 3. Releases and auto-update

- `release.yml` gains a per-platform step: build `donna-server --release` for the
  matrix target, copy to `src-tauri/binaries/donna-server-<triple>`, then run the
  existing `tauri build`.
- Add the **Tauri updater plugin**: generate an updater keypair (free, unrelated to OS
  code signing); CI signs update artifacts and publishes `latest.json` with the GitHub
  Release. The app checks on launch and offers a one-click "Update & restart".
- Installers remain unsigned; macOS/Windows first-open friction is documented on the
  landing page next to the download button.

### 4. Landing page

- **Stack:** hand-written static `site/index.html` + `site/styles.css` + small
  `site/main.js` (OS detection for the download button, latest-version fetch from the
  GitHub Releases API). No framework, no build step.
- **Deploy:** GitHub Actions workflow → GitHub Pages (`duckyquang.github.io/Donna`;
  custom domain possible later).
- **Structure** (per reference images; no emojis anywhere, brand SVGs + CSS-built UI
  cards as assets):
  1. Sticky nav: Donna wordmark, anchors (Features, Privacy, FAQ), GitHub link,
     Download CTA.
  2. Hero: light textured background, app icon above a two-line headline (black line,
     gray line), one-sentence subline, OS-aware primary Download button + "all
     platforms" link; floating cards around the hero (sticky note, reminders card,
     tasks card, integrations cluster with real Google/Gmail/Slack/WhatsApp/Fathom
     logos).
  3. Three-point strip (proactive, private, yours) with inline-SVG line icons.
  4. Big app screenshot in a rounded, colored frame (real captured screenshots).
  5. Features grid ("Everything in one place"): cards with CSS mini-mockups — chat,
     memory/mind map, calendar, docs, notifications, skills.
  6. Integrations logo grid.
  7. Privacy section ("Runs on your machine") — replaces the reference's fake
     testimonials.
  8. FAQ, including the unsigned-app "first open" guide (macOS Open Anyway, Windows
     SmartScreen "Run anyway") and "Is it really free?".
  9. Footer: GitHub, MIT license, docs.
- Brand logos served as local SVG files in `site/assets/` (downloaded once from
  official brand resources — no hotlinking, no CDN dependency).

### 5. README rewrite

- Quick start becomes: download from the landing page → open → onboarding does the
  rest (with the Open-Anyway note).
- Docker Compose / Cloudflare Tunnel / migration content moves under a
  "Self-hosting the server (advanced)" section; dev instructions stay under "For
  developers".
- Link to the landing page at the top.

## Error handling

- Sidecar fails to start → bounded restarts, then the existing unreachable banner
  with a "Report a problem" link to GitHub issues.
- Ollama runtime download fails (offline, asset renamed) → clear error + "Get Ollama
  instead" fallback + "try again"; the wizard never dead-ends.
- Model pull interrupted → Ollama resumes pulls; the wizard's Retry re-issues the pull.
- Port 8377 taken → ephemeral port fallback (config handed to the UI, nothing
  hardcoded).
- Updater offline/rate-limited → silent skip; check again next launch.

## Testing

- Rust: unit-test token/port bootstrap and sidecar lifecycle helpers where they're
  pure; manual smoke on the built .dmg/.msi/.AppImage for launch → health → chat.
- Onboarding: manual matrix — no Ollama / Ollama installed but no model / model
  present / API-key path; plus the offline-failure fallback.
- CI: release workflow proven by tagging a prerelease (e.g. `v0.2.0-rc1`) and
  installing the artifacts on macOS + Windows.
- Landing page: link check, Lighthouse pass, download buttons resolve to the latest
  release assets, renders sanely at mobile widths.

## Build order (each phase lands independently)

1. **Sidecar + bootstrap** — packaged app works standalone end-to-end.
2. **Ollama auto-install onboarding** — hands-free local brain.
3. **CI + updater** — per-target sidecar builds, updater artifacts, prerelease tag
   proof.
4. **Landing page + README** — `site/`, Pages deploy, README rewrite.

## Known risks

- Ollama's release asset naming can change → version pinned; fallback path keeps the
  wizard alive; bumping the pin is a one-line change.
- Unsigned builds mean scary OS dialogs → mitigated by docs; signing can be added
  later without touching this design.
- Tray behavior differs per OS (menu bar on macOS, notification area on
  Windows/Linux) — use Tauri's tray API defaults, no custom per-OS code beyond icons.
