import { useEffect, useState } from "react";
import { Check, Link2, Plug, RefreshCw, Unplug } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { Button, Spinner } from "../components/ui";
import { api, type IntegrationStatus } from "../lib/api";

export default function Integrations() {
  const [statuses, setStatuses] = useState<IntegrationStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Google client credential inputs (bring-your-own OAuth client).
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [slackToken, setSlackToken] = useState("");
  const [fathomKey, setFathomKey] = useState("");

  const refresh = async () => {
    setStatuses(await api.integrationsStatus());
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

  const google = status("google");
  const slack = status("slack");
  const fathom = status("fathom");

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

          {/* Google Workspace */}
          <Card
            name="Google Workspace"
            sub="Calendar, Gmail, Docs & Drive"
            connected={!!google?.connected}
          >
            {google?.connected ? (
              <Button
                variant="danger"
                onClick={() => run("google", api.googleDisconnect)}
                disabled={busy === "google"}
              >
                {busy === "google" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <p className="text-xs text-gray-400">
                  Create a Google Cloud OAuth client (type: Desktop app) and paste its
                  credentials. Donna runs the sign-in locally and stores tokens in your
                  keychain.
                </p>
                <input
                  value={clientId}
                  onChange={(e) => setClientId(e.target.value)}
                  placeholder="Client ID"
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
                <input
                  type="password"
                  value={clientSecret}
                  onChange={(e) => setClientSecret(e.target.value)}
                  placeholder="Client secret"
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
                <div className="flex gap-2">
                  <Button
                    variant="ghost"
                    onClick={() =>
                      run("google", () =>
                        api.googleSetClient(clientId.trim(), clientSecret.trim())
                      )
                    }
                    disabled={busy === "google" || !clientId.trim() || !clientSecret.trim()}
                  >
                    <Check size={16} /> Save client
                  </Button>
                  <Button
                    onClick={() => run("google", api.googleConnect)}
                    disabled={busy === "google" || google?.needsSetup}
                  >
                    {busy === "google" ? <Spinner /> : <Link2 size={16} />} Connect Google
                  </Button>
                </div>
                {google?.needsSetup && (
                  <p className="text-[11px] text-gray-500">
                    Save your client credentials first, then connect.
                  </p>
                )}
              </div>
            )}
          </Card>

          {/* Slack */}
          <Card name="Slack" sub="Read channels & send messages" connected={!!slack?.connected}>
            {slack?.connected ? (
              <Button
                variant="danger"
                onClick={() => run("slack", api.slackDisconnect)}
                disabled={busy === "slack"}
              >
                {busy === "slack" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <input
                  type="password"
                  value={slackToken}
                  onChange={(e) => setSlackToken(e.target.value)}
                  placeholder="Slack bot token (xoxb-…)"
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
                <Button
                  onClick={() => run("slack", () => api.slackSetToken(slackToken.trim()))}
                  disabled={busy === "slack" || !slackToken.trim()}
                >
                  {busy === "slack" ? <Spinner /> : <Link2 size={16} />} Connect Slack
                </Button>
              </div>
            )}
          </Card>

          {/* Fathom */}
          <Card name="Fathom" sub="Turn meetings into docs" connected={!!fathom?.connected}>
            {fathom?.connected ? (
              <Button
                variant="danger"
                onClick={() => run("fathom", api.fathomDisconnect)}
                disabled={busy === "fathom"}
              >
                {busy === "fathom" ? <Spinner /> : <Unplug size={16} />} Disconnect
              </Button>
            ) : (
              <div className="space-y-2">
                <input
                  type="password"
                  value={fathomKey}
                  onChange={(e) => setFathomKey(e.target.value)}
                  placeholder="Fathom API key"
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
                <Button
                  onClick={() => run("fathom", () => api.fathomSetKey(fathomKey.trim()))}
                  disabled={busy === "fathom" || !fathomKey.trim()}
                >
                  {busy === "fathom" ? <Spinner /> : <Link2 size={16} />} Connect Fathom
                </Button>
              </div>
            )}
          </Card>

          <button
            onClick={() => run("", refresh)}
            className="flex items-center gap-2 text-xs text-gray-500 hover:text-gray-300"
          >
            <RefreshCw size={12} /> Refresh status
          </button>
        </div>
      )}
    </PageShell>
  );
}

function Card({
  name,
  sub,
  connected,
  children,
}: {
  name: string;
  sub: string;
  connected: boolean;
  children: React.ReactNode;
}) {
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
        <span
          className={`rounded-full px-2 py-0.5 text-xs ${
            connected
              ? "bg-green-500/15 text-green-400"
              : "bg-white/10 text-gray-400"
          }`}
        >
          {connected ? "Connected" : "Not connected"}
        </span>
      </div>
      {children}
    </div>
  );
}
