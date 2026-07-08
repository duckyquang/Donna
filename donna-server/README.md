# donna-server

donna-server is Donna's 24/7 brain. It's a headless Rust (axum) binary that runs on a
VPS, Raspberry Pi, home server, or any always-on Linux box, and it owns everything the
desktop app used to keep locally:

- `donna.sqlite` — conversations, memory, mind map, routines
- `knowledge-base/` — the folder-based knowledge base
- `secrets.json` — API keys / integration credentials (file-backed, 0600)

It exposes an RPC + WebSocket API on port 8377 that the Donna desktop app (and any
other client) talks to. The desktop app no longer touches the database directly — it's
a client of this server. See the design spec at
[`docs/superpowers/specs/2026-07-07-donna-jarvis-design.md`](../docs/superpowers/specs/2026-07-07-donna-jarvis-design.md)
for the full architecture.

The built-in scheduler still runs here too, so proactive routines (morning briefing,
meeting briefings, etc.) fire even when no desktop app is open.

---

## Quickstart

```bash
cd donna-server
cp .env.example .env
# edit .env — set DONNA_TOKEN to a real secret, leave TUNNEL_TOKEN blank for now
docker compose up -d
```

That starts two containers:

- `donna` — the server itself, listening on `:8377`, data persisted in the
  `donna-data` Docker volume (`/data` inside the container).
- `tunnel` — a `cloudflared` sidecar. It's a no-op until you set `TUNNEL_TOKEN` (see
  below) — safe to leave running either way.

Check it's alive:

```bash
curl http://localhost:8377/health
```

---

## Environment variables

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `DONNA_TOKEN` | Yes | — | Bearer token clients must send. Server refuses to start without it. |
| `DONNA_PORT` | No | `8377` | Port the server listens on. |
| `DONNA_BIND` | No | `0.0.0.0` | Interface to bind to. The Tauri desktop app's embedded sidecar sets this to `127.0.0.1` since it's only ever reached locally; leave the Docker default alone unless you know you want it exposed differently. |
| `DONNA_DATA_DIR` | No | `/data` (in the container) | Where `donna.sqlite`, `knowledge-base/`, and `secrets.json` live. Set by the Dockerfile; only override if running the binary outside Docker. |
| `TUNNEL_TOKEN` | No | — | Cloudflare Tunnel token, for exposing the server publicly. See below. |
| `DONNA_WA_VERIFY_TOKEN` | No | — | Verify token for the WhatsApp webhook handshake. See [WhatsApp two-way](#whatsapp-two-way-phase-3) below. |
| `DONNA_WA_APP_SECRET` | No | — | Meta App Secret, verifies inbound webhook signatures. See [WhatsApp two-way](#whatsapp-two-way-phase-3) below. |

---

## Exposing it publicly with Cloudflare Tunnel

You don't need this for local/LAN use — only if you want to reach the server from
outside your network (e.g. for a future WhatsApp webhook, or connecting the desktop
app while you're away from home).

1. In the [Cloudflare Zero Trust dashboard](https://one.dash.cloudflare.com/), go to
   **Networks → Tunnels** and create a tunnel.
2. Add a public hostname route pointing at `http://donna:8377` (that's the Docker
   Compose service name — the tunnel sidecar reaches it over the compose network).
3. Copy the tunnel token into `TUNNEL_TOKEN` in `.env`, then `docker compose up -d`
   again to pick it up.

---

## WhatsApp two-way (Phase 3)

Text Donna and get replies back, from your own WhatsApp number, via the Meta Cloud API.

**Prerequisites:**
- WhatsApp connected in the desktop app's Integrations page (access token + phone
  number ID from your Meta Business app's WhatsApp Cloud API setup).
- Your own WhatsApp number set in the same Integrations page ("Your WhatsApp
  number" field on the WhatsApp card) — this is the allowlist; the webhook
  ignores messages from anyone else.
- The server reachable over HTTPS (see [Exposing it publicly with Cloudflare
  Tunnel](#exposing-it-publicly-with-cloudflare-tunnel) above).

**Meta dashboard steps:**
1. In your Meta app, go to **WhatsApp → Configuration**.
2. Set **Callback URL** to `https://<your-tunnel-host>/webhook/whatsapp`.
3. Set **Verify token** to the same value you put in `DONNA_WA_VERIFY_TOKEN` (see
   below).
4. Click **Verify and save**, then **Subscribe** to the `messages` webhook field.

**Server config:**

Set both `DONNA_WA_VERIFY_TOKEN` and `DONNA_WA_APP_SECRET` in `.env` (see
`.env.example` for where to find each value), then restart the server
(`docker compose up -d` or `cargo run`, whichever you're running).

**Test it:** text your WhatsApp number from your phone — Donna replies using the
same agent loop as the desktop chat. Approval requests Donna needs your sign-off
on arrive as WhatsApp Approve/Reject buttons instead of plain text.

---

## Voice

Donna can listen and talk back:

- **Desktop app:** click the mic button in the chat composer to record a push-to-talk
  clip; it's transcribed and sent as your message. macOS will prompt for microphone
  permission the first time you record (only appears in a built/signed app or `tauri
  dev`, not a plain browser preview). Turn on "Speak replies aloud" in Settings → Voice
  to have her spoken reply play back after each response, and pick a voice from the
  same card.
- **WhatsApp:** voice notes work automatically once WhatsApp is set up (see above) —
  send one and Donna transcribes it, replies through the normal agent loop, and sends
  a synthesized voice note back (falling back to text if synthesis fails).
- **Requirements:** both paths need an OpenAI API key set (Settings → Provider, or
  the desktop app's key manager) — `/voice/transcribe` and `/voice/speak` return a
  clear 400 error without one, they never crash.

## Migrating from a desktop-only install

If you've been running Donna as a desktop-only app and want to move its data onto the
server:

1. In the desktop app, go to **Settings** → click **Export server bundle…**. This
   writes a `bundle.tar.gz` containing your `donna.sqlite`, `knowledge-base/`, and
   secrets.
2. Copy it to the server (e.g. `scp bundle.tar.gz you@server:/path/`).
3. Import it into a **fresh** data directory:
   ```bash
   docker compose run --rm donna donna-server import /data/bundle.tar.gz
   ```
   The import refuses to run if `/data/donna.sqlite` already exists — it's meant for
   a first-time migration, not merging. If an import is interrupted partway through,
   delete the data volume/dir and retry from the bundle; it's a minor caveat since
   this only matters mid-migration, not during normal operation.
4. `docker compose up -d` to start the server against the migrated data.
5. Back in the desktop app's **Settings**, set **Server URL** (your tunnel hostname,
   or `http://localhost:8377` for LAN/local) and **Access token** (your
   `DONNA_TOKEN`), then **Test connection**.

### If you're switching embedding providers (e.g. Ollama → OpenAI)

Stored vectors are tied to whatever model produced them — switching providers after
migration means old vectors no longer match new query embeddings. Run a one-time
reindex (the existing `kg_reindex_embeddings` Rust command, exposed as
`api.kgReindexEmbeddings()` in `src/lib/api.ts`) so everything is re-embedded with the
new model. There's no UI button wired up for this yet — call
`api.kgReindexEmbeddings()` from the desktop app's devtools console, or wire a button
into the Mind Map view if you hit this regularly.

---

## Local development (without Docker)

```bash
cd donna-server
DONNA_TOKEN=dev cargo run
```

Data defaults to `./donna-data` when `DONNA_DATA_DIR` isn't set.

---

## What's NOT here yet

This is Phase 1 (foundation) of the [server-first design](../docs/superpowers/specs/2026-07-07-donna-jarvis-design.md).
Phases 2 (agent loop / tool registry / trust engine), 3 (two-way WhatsApp text +
approval buttons), 4 (memory/FTS5/suggestion queue), and 5 (voice) are done — see
above. Not built yet: skills (Phase 6).
