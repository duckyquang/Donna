# donna-server

donna-server is a headless Rust binary that runs 24/7 on a server (VPS, Raspberry Pi, Docker container) and handles Donna's proactive tasks without the desktop app running. It sends you a morning briefing at 8am, daily tech news from Hacker News at 9am, pre-meeting briefings 30 minutes before each Google Calendar event, and a weekly review every Sunday at 8pm — all via WhatsApp or Telegram, using only free APIs.

---

## Prerequisites — Environment Variables

Copy `.env.example` to `.env` and fill in the values you need.

| Group | Variables | Required? |
|---|---|---|
| WhatsApp | `DONNA_WHATSAPP_TOKEN`, `DONNA_WHATSAPP_PHONE_ID`, `DONNA_MY_WHATSAPP` | One of WhatsApp or Telegram |
| Telegram | `DONNA_TELEGRAM_TOKEN`, `DONNA_TELEGRAM_CHAT_ID` | One of WhatsApp or Telegram |
| Google Calendar | `DONNA_GOOGLE_CLIENT_ID`, `DONNA_GOOGLE_CLIENT_SECRET`, `DONNA_GOOGLE_REFRESH_TOKEN` | Optional — enables calendar features |
| AI Provider | `DONNA_AI_PROVIDER`, `DONNA_AI_KEY`, `DONNA_AI_MODEL` | Optional — enriches briefings |
| Ollama | `DONNA_OLLAMA_URL` | Only if `DONNA_AI_PROVIDER=ollama` |
| Location | `DONNA_LAT`, `DONNA_LON` | Optional — enables weather in morning briefing |
| Schedule | `DONNA_NEWS_HOUR`, `DONNA_BRIEFING_HOUR` | Optional — default 9 and 8 |

---

## Run locally

```bash
cd donna-server

# Option A: export vars directly
export DONNA_WHATSAPP_TOKEN=your_token
export DONNA_WHATSAPP_PHONE_ID=your_phone_id
export DONNA_MY_WHATSAPP=+84123456789
# … add more as needed

cargo run

# Option B: use a .env file with a tool like dotenvx or direnv
cp .env.example .env
# edit .env, then:
dotenvx run -- cargo run
```

The server prints a startup summary and then ticks every 60 seconds. Tasks fire once per day/week at their scheduled hour.

---

## Deploy with Docker

```bash
# Build
docker build -t donna-server .

# Run with an env file
docker run -d --restart unless-stopped --env-file .env --name donna-server donna-server

# Check logs
docker logs -f donna-server
```

---

## Free deployment options

### Oracle Cloud Free Tier (recommended)

Oracle's Always Free tier includes an ARM instance with 4 OCPU and 24 GB RAM — more than enough to run donna-server forever at no cost.

1. Sign up at [cloud.oracle.com](https://cloud.oracle.com)
2. Create an **Ampere A1** instance (choose Ubuntu or Debian)
3. SSH in, install Docker or copy the binary directly:
   ```bash
   # Copy binary
   scp target/release/donna-server ubuntu@your-ip:/usr/local/bin/

   # Or use Docker (install Docker first, then run the docker command above)
   ```
4. Set up environment variables and start the service — it runs forever for free.

### Fly.io

```bash
# Install flyctl, then from this directory:
fly launch           # follow prompts, pick a small shared-cpu instance
fly secrets set DONNA_WHATSAPP_TOKEN=xxx DONNA_MY_WHATSAPP=+84... # etc.
fly deploy
fly logs             # watch it run
```

### Raspberry Pi

```bash
# Cross-compile or build on the Pi itself
cargo build --release

# Copy binary
scp target/release/donna-server pi@raspberrypi.local:/usr/local/bin/

# Create a systemd unit
sudo tee /etc/systemd/system/donna-server.service <<EOF
[Unit]
Description=Donna Server
After=network.target

[Service]
ExecStart=/usr/local/bin/donna-server
Restart=always
EnvironmentFile=/etc/donna-server.env

[Install]
WantedBy=multi-user.target
EOF

# Put your env vars in /etc/donna-server.env (one KEY=value per line)
sudo systemctl daemon-reload
sudo systemctl enable --now donna-server
sudo journalctl -u donna-server -f
```

---

## Exporting your Google refresh token from Donna desktop

1. Open the Donna desktop app
2. Go to **Settings**
3. Click **Export Google Token** (or "Export credentials")
4. Copy the refresh token and set it as `DONNA_GOOGLE_REFRESH_TOKEN` in your `.env`

The same OAuth client credentials (`DONNA_GOOGLE_CLIENT_ID` and `DONNA_GOOGLE_CLIENT_SECRET`) are reused — you can find these in your Google Cloud Console project.

---

## FAQ

**Can I run donna-server next to Ollama on the same machine?**

Yes. Set `DONNA_AI_PROVIDER=ollama` and `DONNA_OLLAMA_URL=http://localhost:11434` (or whatever host/port your Ollama instance is on). donna-server will call Ollama for AI-enriched briefings with zero external API cost. This works great on an Oracle Cloud ARM instance or a Raspberry Pi 4 with enough RAM to run a small model like Mistral 7B.

**Does this replace the Donna desktop app?**

No — they are complementary. The desktop app handles interactive conversations and on-demand queries. donna-server handles proactive, scheduled notifications so you get updates even when the desktop app is closed.

**What if I don't set up WhatsApp or Telegram?**

donna-server will still run and log all scheduled tasks to stdout — you just won't receive messages. This is useful for testing your config before connecting a messaging channel.
