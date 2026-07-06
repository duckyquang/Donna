import { useEffect, useState } from "react";
import { Check, HelpCircle, Link2, Plug, RefreshCw, Send, Unplug, X } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { Button, Spinner } from "../components/ui";
import {
  api,
  type DriveFile,
  type GitHubIssue,
  type GitHubRepo,
  type GmailMessage,
  type IntegrationStatus,
  type LinearIssue,
  type NotionPage,
} from "../lib/api";

export default function Integrations() {
  const [statuses, setStatuses] = useState<IntegrationStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showHelp, setShowHelp] = useState(false);

  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [slackToken, setSlackToken] = useState("");
  const [fathomKey, setFathomKey] = useState("");
  const [githubToken, setGithubToken] = useState("");
  const [linearKey, setLinearKey] = useState("");
  const [notionToken, setNotionToken] = useState("");
  const [telegramToken, setTelegramToken] = useState("");
  const [telegramChatId, setTelegramChatId] = useState("");
  const [whatsappToken, setWhatsappToken] = useState("");
  const [whatsappPhoneId, setWhatsappPhoneId] = useState("");
  const [discordToken, setDiscordToken] = useState("");

  const [gmailMessages, setGmailMessages] = useState<GmailMessage[]>([]);
  const [driveFiles, setDriveFiles] = useState<DriveFile[]>([]);
  const [gmailLoading, setGmailLoading] = useState(false);
  const [draftTo, setDraftTo] = useState("");
  const [draftSubject, setDraftSubject] = useState("");
  const [draftBody, setDraftBody] = useState("");

  const [githubRepos, setGithubRepos] = useState<GitHubRepo[]>([]);
  const [githubIssues, setGithubIssues] = useState<GitHubIssue[]>([]);
  const [linearIssues, setLinearIssues] = useState<LinearIssue[]>([]);
  const [notionPages, setNotionPages] = useState<NotionPage[]>([]);

  const refresh = async () => {
    const next = await api.integrationsStatus();
    setStatuses(next);
    const connected = (id: string) => !!next.find((s) => s.id === id)?.connected;

    if (connected("google")) {
      setGmailLoading(true);
      try {
        const [messages, files] = await Promise.all([
          api.gmailListMessages(),
          api.driveListFiles(),
        ]);
        setGmailMessages(messages);
        setDriveFiles(files);
      } catch {
        setGmailMessages([]);
        setDriveFiles([]);
      } finally {
        setGmailLoading(false);
      }
    } else {
      setGmailMessages([]);
      setDriveFiles([]);
    }

    if (connected("github")) {
      try {
        const [repos, issues] = await Promise.all([
          api.githubListRepos(),
          api.githubListIssues(),
        ]);
        setGithubRepos(repos);
        setGithubIssues(issues);
      } catch {
        setGithubRepos([]);
        setGithubIssues([]);
      }
    } else {
      setGithubRepos([]);
      setGithubIssues([]);
    }

    if (connected("linear")) {
      try {
        setLinearIssues(await api.linearListIssues());
      } catch {
        setLinearIssues([]);
      }
    } else {
      setLinearIssues([]);
    }

    if (connected("notion")) {
      try {
        setNotionPages(await api.notionSearchPages());
      } catch {
        setNotionPages([]);
      }
    } else {
      setNotionPages([]);
    }
  };

  useEffect(() => {
    refresh()
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  const status = (id: string) => statuses.find((s) => s.id === id);

  const run = async (id: string, fn: () => Promise<void>) => {
    setError(null);
    setBusy(id);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const inputClass =
    "w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent";

  return (
    <>
    {showHelp && <SetupGuideModal onClose={() => setShowHelp(false)} />}
    <PageShell
      title="Integrations"
      subtitle="Connect your tools so Donna can work across them. Credentials are stored in your OS keychain."
    >
      <div className="mb-6 flex justify-end">
        <button
          onClick={() => setShowHelp(true)}
          className="flex items-center gap-1.5 rounded-lg border border-white/10 px-3 py-1.5 text-xs text-gray-400 hover:bg-white/5 hover:text-gray-200 transition-colors"
        >
          <HelpCircle size={13} />
          Setup guides
        </button>
      </div>
      {loading ? (
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <Spinner /> Loading…
        </div>
      ) : (
        <div className="space-y-4">
          {error && (
            <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
              {error}
            </p>
          )}

          <Card name="Google Workspace" sub="Calendar, Gmail, Docs & Drive" connected={!!status("google")?.connected}>
            {status("google")?.connected ? (
              <div className="space-y-3">
                <Button variant="danger" onClick={() => run("google", api.googleDisconnect)} disabled={busy === "google"}>
                  {busy === "google" ? <Spinner /> : <Unplug size={16} />} Disconnect
                </Button>
                <PreviewBox title="Recent Gmail">
                  {gmailLoading ? (
                    <SpinnerRow label="Loading messages…" />
                  ) : gmailMessages.length === 0 ? (
                    <p className="text-xs text-gray-500">No recent messages.</p>
                  ) : (
                    <ul className="space-y-2">
                      {gmailMessages.slice(0, 5).map((m) => (
                        <li key={m.id} className="rounded-lg border border-white/5 px-2 py-1.5">
                          <div className="truncate text-xs font-medium text-white">{m.subject || "(No subject)"}</div>
                          <div className="truncate text-[11px] text-gray-400">{m.from} · {m.snippet}</div>
                        </li>
                      ))}
                    </ul>
                  )}
                </PreviewBox>
                <PreviewBox title="Compose Gmail draft">
                  <div className="space-y-2">
                    <input value={draftTo} onChange={(e) => setDraftTo(e.target.value)} placeholder="To" className={inputClass} />
                    <input value={draftSubject} onChange={(e) => setDraftSubject(e.target.value)} placeholder="Subject" className={inputClass} />
                    <textarea value={draftBody} onChange={(e) => setDraftBody(e.target.value)} placeholder="Message" rows={3} className={inputClass} />
                    <Button
                      onClick={() =>
                        run("gmail-draft", async () => {
                          await api.gmailCreateDraft(
                            draftTo.trim(),
                            draftSubject.trim(),
                            draftBody.trim()
                          );
                        })
                      }
                      disabled={busy === "gmail-draft" || !draftTo.trim() || !draftSubject.trim()}
                    >
                      {busy === "gmail-draft" ? <Spinner /> : <Send size={16} />} Save draft
                    </Button>
                  </div>
                </PreviewBox>
                <PreviewBox title="Google Drive">
                  {gmailLoading ? (
                    <SpinnerRow label="Loading files…" />
                  ) : driveFiles.length === 0 ? (
                    <p className="text-xs text-gray-500">No files found.</p>
                  ) : (
                    <ul className="space-y-2">
                      {driveFiles.slice(0, 5).map((f) => (
                        <li key={f.id} className="rounded-lg border border-white/5 px-2 py-1.5">
                          <div className="truncate text-xs font-medium text-white">{f.name}</div>
                          <div className="truncate text-[11px] text-gray-400">{f.mimeType}</div>
                        </li>
                      ))}
                    </ul>
                  )}
                </PreviewBox>
              </div>
            ) : (
              <div className="space-y-2">
                <p className="text-xs text-gray-400">
                  Create a Google Cloud OAuth client (Desktop app) and paste credentials. Donna runs sign-in locally.
                </p>
                <input value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="Client ID" className={inputClass} />
                <input type="password" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} placeholder="Client secret" className={inputClass} />
                <div className="flex gap-2">
                  <Button variant="ghost" onClick={() => run("google", () => api.googleSetClient(clientId.trim(), clientSecret.trim()))} disabled={busy === "google" || !clientId.trim() || !clientSecret.trim()}>
                    <Check size={16} /> Save client
                  </Button>
                  <Button onClick={() => run("google", api.googleConnect)} disabled={busy === "google" || status("google")?.needsSetup}>
                    {busy === "google" ? <Spinner /> : <Link2 size={16} />} Connect Google
                  </Button>
                </div>
              </div>
            )}
          </Card>

          <Card name="Slack" sub="Read channels & send messages" connected={!!status("slack")?.connected}>
            {status("slack")?.connected ? (
              <Button variant="danger" onClick={() => run("slack", api.slackDisconnect)} disabled={busy === "slack"}>
                {busy === "slack" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <input type="password" value={slackToken} onChange={(e) => setSlackToken(e.target.value)} placeholder="Slack bot token (xoxb-…)" className={inputClass} />
                <Button onClick={() => run("slack", () => api.slackSetToken(slackToken.trim()))} disabled={busy === "slack" || !slackToken.trim()}>
                  {busy === "slack" ? <Spinner /> : <Link2 size={16} />} Connect Slack
                </Button>
              </div>
            )}
          </Card>

          <Card name="Fathom" sub="Turn meetings into docs" connected={!!status("fathom")?.connected}>
            {status("fathom")?.connected ? (
              <Button variant="danger" onClick={() => run("fathom", api.fathomDisconnect)} disabled={busy === "fathom"}>
                {busy === "fathom" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <input type="password" value={fathomKey} onChange={(e) => setFathomKey(e.target.value)} placeholder="Fathom API key" className={inputClass} />
                <Button onClick={() => run("fathom", () => api.fathomSetKey(fathomKey.trim()))} disabled={busy === "fathom" || !fathomKey.trim()}>
                  {busy === "fathom" ? <Spinner /> : <Link2 size={16} />} Connect Fathom
                </Button>
              </div>
            )}
          </Card>

          <Card name="GitHub" sub="Repos & assigned issues" connected={!!status("github")?.connected}>
            {status("github")?.connected ? (
              <div className="space-y-3">
                <Button variant="danger" onClick={() => run("github", api.githubDisconnect)} disabled={busy === "github"}>
                  {busy === "github" ? <Spinner /> : <Unplug size={16} />} Disconnect
                </Button>
                <PreviewBox title="Recent repos">
                  {githubRepos.length === 0 ? (
                    <p className="text-xs text-gray-500">No repos.</p>
                  ) : (
                    <ul className="space-y-1 text-xs text-gray-300">
                      {githubRepos.slice(0, 5).map((r) => (
                        <li key={r.id}>{r.fullName}</li>
                      ))}
                    </ul>
                  )}
                </PreviewBox>
                <PreviewBox title="Open issues">
                  {githubIssues.length === 0 ? (
                    <p className="text-xs text-gray-500">No open issues assigned to you.</p>
                  ) : (
                    <ul className="space-y-1 text-xs text-gray-300">
                      {githubIssues.slice(0, 5).map((i) => (
                        <li key={i.id}>{i.repo} #{i.number}: {i.title}</li>
                      ))}
                    </ul>
                  )}
                </PreviewBox>
              </div>
            ) : (
              <div className="space-y-2">
                <input type="password" value={githubToken} onChange={(e) => setGithubToken(e.target.value)} placeholder="Personal access token" className={inputClass} />
                <Button onClick={() => run("github", () => api.githubSetToken(githubToken.trim()))} disabled={busy === "github" || !githubToken.trim()}>
                  {busy === "github" ? <Spinner /> : <Link2 size={16} />} Connect GitHub
                </Button>
              </div>
            )}
          </Card>

          <Card name="Linear" sub="Open issues" connected={!!status("linear")?.connected}>
            {status("linear")?.connected ? (
              <div className="space-y-3">
                <Button variant="danger" onClick={() => run("linear", api.linearDisconnect)} disabled={busy === "linear"}>
                  {busy === "linear" ? <Spinner /> : <Unplug size={16} />} Disconnect
                </Button>
                <PreviewBox title="Open issues">
                  {linearIssues.length === 0 ? (
                    <p className="text-xs text-gray-500">No open issues.</p>
                  ) : (
                    <ul className="space-y-1 text-xs text-gray-300">
                      {linearIssues.slice(0, 5).map((i) => (
                        <li key={i.id}>{i.identifier}: {i.title}</li>
                      ))}
                    </ul>
                  )}
                </PreviewBox>
              </div>
            ) : (
              <div className="space-y-2">
                <input type="password" value={linearKey} onChange={(e) => setLinearKey(e.target.value)} placeholder="Linear API key" className={inputClass} />
                <Button onClick={() => run("linear", () => api.linearSetKey(linearKey.trim()))} disabled={busy === "linear" || !linearKey.trim()}>
                  {busy === "linear" ? <Spinner /> : <Link2 size={16} />} Connect Linear
                </Button>
              </div>
            )}
          </Card>

          <Card name="Notion" sub="Search pages" connected={!!status("notion")?.connected}>
            {status("notion")?.connected ? (
              <div className="space-y-3">
                <Button variant="danger" onClick={() => run("notion", api.notionDisconnect)} disabled={busy === "notion"}>
                  {busy === "notion" ? <Spinner /> : <Unplug size={16} />} Disconnect
                </Button>
                <PreviewBox title="Recent pages">
                  {notionPages.length === 0 ? (
                    <p className="text-xs text-gray-500">No pages found.</p>
                  ) : (
                    <ul className="space-y-1 text-xs text-gray-300">
                      {notionPages.slice(0, 5).map((p) => (
                        <li key={p.id}>{p.title}</li>
                      ))}
                    </ul>
                  )}
                </PreviewBox>
              </div>
            ) : (
              <div className="space-y-2">
                <input type="password" value={notionToken} onChange={(e) => setNotionToken(e.target.value)} placeholder="Notion integration token" className={inputClass} />
                <Button onClick={() => run("notion", () => api.notionSetToken(notionToken.trim()))} disabled={busy === "notion" || !notionToken.trim()}>
                  {busy === "notion" ? <Spinner /> : <Link2 size={16} />} Connect Notion
                </Button>
              </div>
            )}
          </Card>

          <Card name="Telegram" sub="Send messages via bot" connected={!!status("telegram")?.connected}>
            {status("telegram")?.connected ? (
              <Button variant="danger" onClick={() => run("telegram", api.telegramDisconnect)} disabled={busy === "telegram"}>
                {busy === "telegram" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <input type="password" value={telegramToken} onChange={(e) => setTelegramToken(e.target.value)} placeholder="Bot token" className={inputClass} />
                <input value={telegramChatId} onChange={(e) => setTelegramChatId(e.target.value)} placeholder="Chat ID" className={inputClass} />
                <Button onClick={() => run("telegram", () => api.telegramSetCredentials(telegramToken.trim(), telegramChatId.trim()))} disabled={busy === "telegram" || !telegramToken.trim() || !telegramChatId.trim()}>
                  {busy === "telegram" ? <Spinner /> : <Link2 size={16} />} Connect Telegram
                </Button>
              </div>
            )}
          </Card>

          <Card name="WhatsApp" sub="WhatsApp Business Cloud API" connected={!!status("whatsapp")?.connected}>
            {status("whatsapp")?.connected ? (
              <Button variant="danger" onClick={() => run("whatsapp", api.whatsappDisconnect)} disabled={busy === "whatsapp"}>
                {busy === "whatsapp" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <p className="text-xs text-gray-400">Requires Meta Business app with WhatsApp Cloud API enabled.</p>
                <input type="password" value={whatsappToken} onChange={(e) => setWhatsappToken(e.target.value)} placeholder="Permanent access token" className={inputClass} />
                <input value={whatsappPhoneId} onChange={(e) => setWhatsappPhoneId(e.target.value)} placeholder="Phone number ID" className={inputClass} />
                <Button onClick={() => run("whatsapp", () => api.whatsappSetCredentials(whatsappToken.trim(), whatsappPhoneId.trim()))} disabled={busy === "whatsapp" || !whatsappToken.trim() || !whatsappPhoneId.trim()}>
                  {busy === "whatsapp" ? <Spinner /> : <Link2 size={16} />} Connect WhatsApp
                </Button>
              </div>
            )}
          </Card>

          <Card name="Discord" sub="Send messages via Discord bot" connected={!!status("discord")?.connected}>
            {status("discord")?.connected ? (
              <Button variant="danger" onClick={() => run("discord", api.discordDisconnect)} disabled={busy === "discord"}>
                {busy === "discord" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <p className="text-xs text-gray-400">
                  Create a Discord bot at discord.com/developers, add it to your server, and paste its token here. Enable the <strong>MESSAGE_CONTENT</strong> intent in the Discord Developer Portal under Bot &gt; Privileged Gateway Intents.
                </p>
                <input type="password" value={discordToken} onChange={(e) => setDiscordToken(e.target.value)} placeholder="Bot token" className={inputClass} />
                <Button onClick={() => run("discord", () => api.discordSetToken(discordToken.trim()))} disabled={busy === "discord" || !discordToken.trim()}>
                  {busy === "discord" ? <Spinner /> : <Link2 size={16} />} Connect Discord
                </Button>
              </div>
            )}
          </Card>

          <button onClick={() => run("", refresh)} className="flex items-center gap-2 text-xs text-gray-500 hover:text-gray-300">
            <RefreshCw size={12} /> Refresh status
          </button>
        </div>
      )}
    </PageShell>
    </>
  );
}

// ─── Setup Guide Modal ───────────────────────────────────────────────────────

const GUIDES: { id: string; name: string; steps: { title: string; body: string }[] }[] = [
  {
    id: "google",
    name: "Google Workspace",
    steps: [
      { title: "Open Google Cloud Console", body: "Go to console.cloud.google.com → select or create a project." },
      { title: "Enable APIs", body: "APIs & Services → Library → enable Gmail API, Google Calendar API, and Google Drive API." },
      { title: "Create OAuth credentials", body: "APIs & Services → Credentials → Create Credentials → OAuth client ID → choose Desktop app → copy Client ID and Client Secret." },
      { title: "Configure consent screen", body: "OAuth consent screen → External (or Internal) → add your Gmail as a test user → add scopes: gmail.readonly, gmail.compose, calendar, drive.readonly." },
      { title: "Paste into Donna", body: "Paste Client ID and Client Secret in the Google card → Save client → Connect Google." },
    ],
  },
  {
    id: "slack",
    name: "Slack",
    steps: [
      { title: "Create a Slack app", body: "Go to api.slack.com/apps → Create New App → From scratch → name it (e.g. Donna)." },
      { title: "Add bot scopes", body: "OAuth & Permissions → Bot Token Scopes → add: channels:read, chat:write, groups:read, im:read, users:read." },
      { title: "Install to workspace", body: "Install App → Install to Workspace → allow → copy the Bot User OAuth Token (starts with xoxb-)." },
      { title: "Paste into Donna", body: "Paste the xoxb- token into the Slack card and click Connect." },
    ],
  },
  {
    id: "fathom",
    name: "Fathom",
    steps: [
      { title: "Open Fathom settings", body: "Log into fathom.video → click your avatar → Settings → API → Create API Key." },
      { title: "Copy the key", body: "Name it 'Donna' and copy the generated key." },
      { title: "Paste into Donna", body: "Paste the key into the Fathom card and click Connect." },
    ],
  },
  {
    id: "discord",
    name: "Discord",
    steps: [
      { title: "Create a Discord application", body: "Go to discord.com/developers/applications → New Application → name it (e.g. Donna)." },
      { title: "Create a bot", body: "Select your app → Bot → Add Bot → copy the Token." },
      { title: "Enable intents", body: "Bot → Privileged Gateway Intents → enable MESSAGE CONTENT INTENT (and Server Members if needed)." },
      { title: "Invite bot to server", body: "OAuth2 → URL Generator → check bot + Send Messages → open the generated URL and add to your server." },
      { title: "Paste into Donna", body: "Paste the bot Token into the Discord card and click Connect." },
    ],
  },
  {
    id: "github",
    name: "GitHub",
    steps: [
      { title: "Open GitHub Settings", body: "github.com → avatar → Settings → Developer settings → Personal access tokens → Tokens (classic)." },
      { title: "Generate token", body: "Generate new token → give it a name → check scopes: repo, read:user, read:org, issues." },
      { title: "Copy and paste", body: "Copy the token immediately (it won't show again) → paste into the GitHub card → Connect." },
    ],
  },
  {
    id: "linear",
    name: "Linear",
    steps: [
      { title: "Open Linear settings", body: "linear.app → your workspace → Settings → Security & access → Personal API keys → New API key." },
      { title: "Name and create", body: "Name it 'Donna' → Create key → copy the shown value (you won't see it again)." },
      { title: "Paste into Donna", body: "Paste into the Linear card and click Connect." },
    ],
  },
  {
    id: "notion",
    name: "Notion",
    steps: [
      { title: "Create an integration", body: "Go to notion.so/my-integrations → New integration → name it 'Donna' → select your workspace." },
      { title: "Set capabilities", body: "Capabilities → enable Read content, Read user info without email." },
      { title: "Share pages", body: "In Notion, open each page/database you want Donna to read → … → Connections → Donna." },
      { title: "Copy and paste", body: "Back in the integration settings, copy the Internal Integration Token → paste into the Notion card → Connect." },
    ],
  },
  {
    id: "telegram",
    name: "Telegram",
    steps: [
      { title: "Create a bot", body: "Open Telegram → search @BotFather → /newbot → follow prompts → copy the bot token." },
      { title: "Get your chat ID", body: "Start a chat with your new bot, then visit: https://api.telegram.org/bot<TOKEN>/getUpdates — look for 'id' in 'chat'." },
      { title: "Paste into Donna", body: "Enter the bot token and your chat ID in the Telegram card → Connect." },
    ],
  },
  {
    id: "whatsapp",
    name: "WhatsApp",
    steps: [
      { title: "Set up Meta Business", body: "Go to developers.facebook.com → My Apps → Create App → Business → add WhatsApp product." },
      { title: "Get a test number", body: "WhatsApp → API Setup → use the sandbox phone number or add a production number." },
      { title: "Generate access token", body: "System Users → generate a permanent token with whatsapp_business_messaging permission." },
      { title: "Get Phone Number ID", body: "WhatsApp → API Setup → copy the Phone number ID (not the phone number itself)." },
      { title: "Paste into Donna", body: "Paste the access token and phone number ID into the WhatsApp card → Connect." },
    ],
  },
  {
    id: "server",
    name: "donna-server (24/7)",
    steps: [
      { title: "Deploy the server binary", body: "The donna-server/ folder is a standalone Rust binary. See donna-server/README.md for guides for Oracle Cloud Free Tier, Fly.io, and Raspberry Pi." },
      { title: "Set environment variables", body: "Copy donna-server/.env.example to .env and fill in DONNA_* vars: WhatsApp/Telegram credentials, Google calendar OAuth token, location coords, and briefing hours." },
      { title: "Run with Docker", body: "cd donna-server && docker build -t donna-server . && docker run --env-file .env donna-server" },
      { title: "What it does", body: "Sends you a morning briefing, daily tech news digest, weekly review, and pre-meeting background checks — all via WhatsApp or Telegram — even while your Mac is off." },
    ],
  },
];

function SetupGuideModal({ onClose }: { onClose: () => void }) {
  const [activeId, setActiveId] = useState(GUIDES[0].id);
  const guide = GUIDES.find((g) => g.id === activeId) ?? GUIDES[0];

  return (
    <>
      {/* Backdrop */}
      <button
        type="button"
        aria-label="Close guide"
        className="fixed inset-0 z-40 bg-black/60 backdrop-blur-[2px]"
        onClick={onClose}
      />
      {/* Panel */}
      <div className="fixed inset-y-0 right-0 z-50 flex w-full max-w-2xl flex-col border-l border-white/10 bg-donna-panel shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/10 px-6 py-4">
          <div className="flex items-center gap-2.5">
            <HelpCircle size={16} className="text-donna-accent-light" />
            <h2 className="text-sm font-semibold text-white">Setup Guides</h2>
          </div>
          <button
            onClick={onClose}
            className="rounded-lg p-1 text-gray-400 hover:bg-white/8 hover:text-white transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        <div className="flex flex-1 overflow-hidden">
          {/* Sidebar */}
          <div className="w-44 flex-shrink-0 overflow-y-auto border-r border-white/8 py-3">
            {GUIDES.map((g) => (
              <button
                key={g.id}
                onClick={() => setActiveId(g.id)}
                className={`w-full px-4 py-2 text-left text-xs transition-colors ${
                  g.id === activeId
                    ? "bg-donna-accent/12 text-donna-accent-light font-medium"
                    : "text-gray-400 hover:bg-white/5 hover:text-gray-200"
                }`}
              >
                {g.name}
              </button>
            ))}
          </div>

          {/* Steps */}
          <div className="flex-1 overflow-y-auto px-6 py-5">
            <h3 className="mb-5 text-base font-semibold text-white">{guide.name}</h3>
            <ol className="space-y-5">
              {guide.steps.map((step, i) => (
                <li key={i} className="flex gap-4">
                  <div className="mt-0.5 flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full bg-donna-accent/20 text-[10px] font-bold text-donna-accent-light">
                    {i + 1}
                  </div>
                  <div>
                    <p className="text-sm font-medium text-gray-100">{step.title}</p>
                    <p className="mt-1 text-xs leading-relaxed text-gray-400">{step.body}</p>
                  </div>
                </li>
              ))}
            </ol>
          </div>
        </div>
      </div>
    </>
  );
}

function Card({ name, sub, connected, children }: { name: string; sub: string; connected: boolean; children: React.ReactNode }) {
  return (
    <div className="rounded-xl border border-white/10 bg-donna-surface p-4">
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-donna-accent/15 text-donna-accent-light">
            <Plug size={18} />
          </div>
          <div>
            <div className="text-sm font-medium text-white">{name}</div>
            <div className="text-xs text-gray-400">{sub}</div>
          </div>
        </div>
        <span className={`rounded-full px-2 py-0.5 text-xs ${connected ? "bg-green-500/15 text-green-400" : "bg-white/10 text-gray-400"}`}>
          {connected ? "Connected" : "Not connected"}
        </span>
      </div>
      {children}
    </div>
  );
}

function PreviewBox({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-white/10 bg-donna-bg/50 p-3">
      <div className="mb-2 text-xs font-medium text-gray-300">{title}</div>
      {children}
    </div>
  );
}

function SpinnerRow({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-2 text-xs text-gray-400">
      <Spinner /> {label}
    </div>
  );
}
