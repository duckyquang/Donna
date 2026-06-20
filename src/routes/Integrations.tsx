import { useEffect, useState } from "react";
import { Check, Link2, Plug, RefreshCw, Send, Unplug } from "lucide-react";
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
    <PageShell
      title="Integrations"
      subtitle="Connect your tools so Donna can work across them. Credentials are stored in your OS keychain."
    >
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
