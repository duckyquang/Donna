import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, RefreshCw, Trash2 } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { PROVIDERS, type ProviderId } from "../lib/models/providers";
import { api, type AutonomyLevel, type TrustPolicy } from "../lib/api";
import { serverConfig, setServerConfig, serverReachable } from "../lib/server";
import { useConfig } from "../lib/useConfig";
import { Button, Spinner } from "../components/ui";

const TOOL_LABELS: Record<string, string> = {
  slack_send_message: "Slack: send message",
  telegram_send_message: "Telegram: send message",
};

function toolLabel(actionKind: string): string {
  return (
    TOOL_LABELS[actionKind] ??
    actionKind
      .split("_")
      .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
      .join(" ")
  );
}

const TTS_VOICES = [
  "nova",
  "shimmer",
  "coral",
  "sage",
  "ballad",
  "alloy",
  "echo",
  "fable",
  "onyx",
  "ash",
  "verse",
];

const AUTONOMY_OPTIONS: { value: AutonomyLevel; label: string; desc: string }[] = [
  {
    value: "confirm",
    label: "Confirm before acting",
    desc: "Donna asks before sending emails, creating events, or taking other actions.",
  },
  {
    value: "act",
    label: "Act on clear tasks",
    desc: "Donna handles straightforward tasks automatically and confirms ambiguous ones.",
  },
  {
    value: "autonomous",
    label: "High autonomy",
    desc: "Donna acts proactively with minimal confirmation when she is confident.",
  },
];

export default function Settings() {
  const { config, save, refresh } = useConfig();

  const [provider, setProvider] = useState<ProviderId>("ollama");
  const [model, setModel] = useState("");
  const [ollamaHost, setOllamaHost] = useState("http://localhost:11434");
  const [embedModel, setEmbedModel] = useState("nomic-embed-text");
  const [reviewModel, setReviewModel] = useState("");
  const [ttsVoice, setTtsVoice] = useState("nova");
  const [speakReplies, setSpeakReplies] = useState(false);
  const [autonomyLevel, setAutonomyLevel] = useState<AutonomyLevel>("confirm");
  const [apiKey, setApiKey] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [serverUrl, setServerUrl] = useState(serverConfig().url);
  const [serverToken, setServerToken] = useState(serverConfig().token);
  const [testResult, setTestResult] = useState<"ok" | "fail" | null>(null);
  const [testing, setTesting] = useState(false);

  const [trustPolicies, setTrustPolicies] = useState<TrustPolicy[]>([]);
  const [policyError, setPolicyError] = useState<string | null>(null);

  useEffect(() => {
    api.trustPoliciesList().then(setTrustPolicies).catch((e) => setPolicyError(String(e)));
  }, []);

  const setPolicyMode = async (actionKind: string, mode: "ask" | "auto") => {
    const prev = trustPolicies;
    setPolicyError(null);
    setTrustPolicies((rows) =>
      rows.map((r) => (r.actionKind === actionKind ? { ...r, mode } : r))
    );
    try {
      await api.trustPolicySet(actionKind, mode);
    } catch (e) {
      setTrustPolicies(prev);
      setPolicyError(String(e));
    }
  };

  const testConnection = async () => {
    setServerConfig({ url: serverUrl.trim(), token: serverToken.trim() });
    setTesting(true);
    setTestResult(null);
    try {
      setTestResult((await serverReachable()) ? "ok" : "fail");
    } finally {
      setTesting(false);
    }
  };

  const [exporting, setExporting] = useState(false);

  const exportBundle = async () => {
    setError(null);
    setStatus(null);
    const destDir = await open({ directory: true, multiple: false });
    if (!destDir || Array.isArray(destDir)) return;
    setExporting(true);
    try {
      const path = await api.exportServerBundle(destDir);
      setStatus(`Exported bundle to ${path}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setExporting(false);
    }
  };

  const meta = PROVIDERS.find((p) => p.id === provider)!;
  const isLocal = meta.kind === "local";

  useEffect(() => {
    if (config) {
      setProvider(config.provider);
      setModel(config.model);
      setOllamaHost(config.ollamaHost);
      setEmbedModel(config.embedModel ?? "nomic-embed-text");
      setReviewModel(config.reviewModel ?? "");
      setTtsVoice(config.ttsVoice || "nova");
      setSpeakReplies(config.speakReplies ?? false);
      setAutonomyLevel(config.autonomyLevel ?? "confirm");
    }
  }, [config]);

  useEffect(() => {
    setStatus(null);
    setError(null);
    setApiKey("");
    if (!isLocal) {
      api.hasApiKey(provider).then(setHasKey);
    } else {
      setHasKey(false);
    }
  }, [provider, isLocal]);

  const fetchModels = async () => {
    setError(null);
    setLoadingModels(true);
    try {
      if (!isLocal && apiKey.trim()) {
        await api.setApiKey(provider, apiKey.trim());
        setHasKey(true);
        setApiKey("");
        setStatus("API key saved to your keychain.");
      }
      if (isLocal) {
        await api.saveConfig({
          provider,
          model,
          ollamaHost,
          embedModel,
          reviewModel,
          ttsVoice,
          speakReplies,
          onboarded: true,
          profileOnboarded: config?.profileOnboarded ?? false,
          autonomyLevel,
        });
      }
      const list = await api.listModels(provider);
      setModels(list);
      if (list.length > 0 && !list.includes(model)) setModel(list[0]);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingModels(false);
    }
  };

  const removeKey = async () => {
    await api.deleteApiKey(provider);
    setHasKey(false);
    setStatus("API key removed.");
  };

  const handleSave = async () => {
    setError(null);
    try {
      await save({
        provider,
        model,
        ollamaHost,
        embedModel,
        reviewModel,
        ttsVoice,
        speakReplies,
        onboarded: true,
        profileOnboarded: config?.profileOnboarded ?? false,
        autonomyLevel,
      });
      await refresh();
      setStatus("Settings saved.");
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <PageShell
      title="Settings"
      subtitle="Choose how Donna thinks — a free local model or your own API key."
    >
      <div className="space-y-6">
        <section className="space-y-3 rounded-xl border border-white/10 bg-donna-surface p-4">
          <div>
            <h2 className="text-sm font-medium text-gray-300">Server</h2>
            <p className="text-xs text-gray-500">
              Donna's brain runs in donna-server. Point the desktop app at it.
            </p>
          </div>
          <label className="block">
            <span className="mb-1 block text-sm text-gray-300">Server URL</span>
            <input
              value={serverUrl}
              onChange={(e) => setServerUrl(e.target.value)}
              placeholder="http://localhost:8377"
              className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm text-gray-300">Access token</span>
            <input
              type="password"
              value={serverToken}
              onChange={(e) => setServerToken(e.target.value)}
              placeholder="DONNA_TOKEN"
              className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
            />
          </label>
          <div className="flex items-center gap-3">
            <Button variant="ghost" onClick={testConnection} disabled={testing}>
              {testing ? <Spinner /> : <RefreshCw size={16} />}
              Test connection
            </Button>
            {testResult === "ok" && <span className="text-xs text-green-400">Connected ✓</span>}
            {testResult === "fail" && <span className="text-xs text-red-400">Unreachable</span>}
          </div>
          <div className="border-t border-white/10 pt-3">
            <p className="mb-2 text-xs text-gray-500">
              Migrating from an older desktop-only install? Export your local data as a
              bundle, then run <code className="text-gray-400">donna-server import</code>{" "}
              with it on your server.
            </p>
            <Button variant="ghost" onClick={exportBundle} disabled={exporting}>
              {exporting ? <Spinner /> : null}
              Export server bundle…
            </Button>
          </div>
        </section>

        <section>
          <h2 className="mb-2 text-sm font-medium text-gray-300">Provider</h2>
          <div className="grid grid-cols-2 gap-2">
            {PROVIDERS.map((p) => (
              <button
                key={p.id}
                onClick={() => setProvider(p.id)}
                className={`rounded-xl border p-3 text-left transition-colors ${
                  provider === p.id
                    ? "border-donna-accent bg-donna-accent/10"
                    : "border-white/10 hover:bg-white/5"
                }`}
              >
                <div className="text-sm font-medium text-white">{p.label}</div>
                <div className="text-xs text-gray-400">
                  {p.kind === "local" ? "Free · Local" : "Your API key"}
                </div>
              </button>
            ))}
          </div>
        </section>

        <section className="space-y-3">
          {isLocal ? (
            <>
            <label className="block">
              <span className="mb-1 block text-sm text-gray-300">Ollama host</span>
              <input
                value={ollamaHost}
                onChange={(e) => setOllamaHost(e.target.value)}
                className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
              />
            </label>
            <label className="block">
              <span className="mb-1 block text-sm text-gray-300">Embedding model</span>
              <input
                value={embedModel}
                onChange={(e) => setEmbedModel(e.target.value)}
                placeholder="nomic-embed-text"
                className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
              />
              <span className="mt-1 block text-xs text-gray-500">
                Used for semantic memory retrieval. Run <code className="text-gray-400">ollama pull {embedModel || "nomic-embed-text"}</code> first.
              </span>
            </label>
            </>
          ) : (
            <div className="space-y-1">
              <span className="block text-sm text-gray-300">{meta.label} API key</span>
              {hasKey ? (
                <div className="flex items-center justify-between rounded-lg border border-white/10 bg-donna-bg px-3 py-2">
                  <span className="text-sm text-green-400">
                    Key stored in keychain ✓
                  </span>
                  <button onClick={removeKey} title="Remove key">
                    <Trash2 size={16} className="text-gray-500 hover:text-red-400" />
                  </button>
                </div>
              ) : (
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Paste your API key"
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
              )}
              <span className="block text-xs text-gray-500">
                Stored securely in your OS keychain — never on disk or in the repo.
              </span>
            </div>
          )}

          <Button variant="ghost" onClick={fetchModels} disabled={loadingModels}>
            {loadingModels ? <Spinner /> : <RefreshCw size={16} />}
            Load models
          </Button>

          {models.length > 0 && (
            <label className="block">
              <span className="mb-1 block text-sm text-gray-300">Model</span>
              <select
                value={model}
                onChange={(e) => setModel(e.target.value)}
                className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
              >
                {models.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </label>
          )}

          {!models.length && model && (
            <p className="text-xs text-gray-500">Current model: {model}</p>
          )}

          <label className="block">
            <span className="mb-1 block text-sm text-gray-300">Background review model</span>
            <input
              value={reviewModel}
              onChange={(e) => setReviewModel(e.target.value)}
              placeholder="defaults to your main model"
              className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
            />
            <span className="mt-1 block text-xs text-gray-500">
              Used by the nightly background review that curates memory and files suggestions.
            </span>
          </label>
        </section>

        <section>
          <h2 className="mb-2 text-sm font-medium text-gray-300">Autonomy</h2>
          <p className="mb-3 text-xs text-gray-500">
            How much Donna can do on her own before checking with you.
          </p>
          <div className="space-y-2">
            {AUTONOMY_OPTIONS.map((opt) => (
              <label
                key={opt.value}
                className={`flex cursor-pointer gap-3 rounded-xl border p-3 transition-colors ${
                  autonomyLevel === opt.value
                    ? "border-donna-accent bg-donna-accent/10"
                    : "border-white/10 hover:bg-white/5"
                }`}
              >
                <input
                  type="radio"
                  name="autonomy"
                  value={opt.value}
                  checked={autonomyLevel === opt.value}
                  onChange={() => setAutonomyLevel(opt.value)}
                  className="mt-1 accent-donna-accent"
                />
                <div>
                  <div className="text-sm font-medium text-white">{opt.label}</div>
                  <div className="text-xs text-gray-400">{opt.desc}</div>
                </div>
              </label>
            ))}
          </div>

          {trustPolicies.length > 0 && (
            <div className="mt-4 space-y-2 border-t border-white/10 pt-4">
              <h3 className="text-sm font-medium text-gray-300">Outbound actions</h3>
              <div className="space-y-2">
                {trustPolicies.map((p) => (
                  <div
                    key={p.actionKind}
                    className="flex items-center justify-between rounded-xl border border-white/10 p-3"
                  >
                    <span className="text-sm text-white">{toolLabel(p.actionKind)}</span>
                    <div className="flex overflow-hidden rounded-lg border border-white/10">
                      {(["ask", "auto"] as const).map((mode) => (
                        <button
                          key={mode}
                          onClick={() => setPolicyMode(p.actionKind, mode)}
                          className={`px-3 py-1.5 text-xs font-medium capitalize transition-colors ${
                            p.mode === mode
                              ? "bg-donna-accent/10 text-donna-accent"
                              : "text-gray-400 hover:bg-white/5"
                          }`}
                        >
                          {mode}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
              <p className="text-xs text-gray-500">
                Actions set to ask show an approval card before Donna acts. Donna can never
                take these actions silently unless you set them to auto.
              </p>
              {policyError && (
                <p className="text-xs text-red-400">{policyError}</p>
              )}
            </div>
          )}
        </section>

        <section className="space-y-3 rounded-xl border border-white/10 bg-donna-surface p-4">
          <div>
            <h2 className="text-sm font-medium text-gray-300">Voice</h2>
            <p className="text-xs text-gray-500">Voice needs an OpenAI API key set.</p>
          </div>
          <label className="flex cursor-pointer items-center justify-between gap-3 rounded-xl border border-white/10 p-3">
            <div>
              <div className="text-sm font-medium text-white">Speak replies aloud</div>
              <div className="text-xs text-gray-400">
                Play Donna's reply as speech after it finishes streaming.
              </div>
            </div>
            <input
              type="checkbox"
              checked={speakReplies}
              onChange={(e) => setSpeakReplies(e.target.checked)}
              className="h-4 w-4 accent-donna-accent"
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm text-gray-300">Voice</span>
            <select
              value={ttsVoice}
              onChange={(e) => setTtsVoice(e.target.value)}
              className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
            >
              {TTS_VOICES.map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
          </label>
        </section>

        {status && (
          <p className="rounded-lg border border-green-500/30 bg-green-500/10 p-3 text-xs text-green-300">
            {status}
          </p>
        )}
        {error && (
          <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
            {error}
          </p>
        )}

        <div>
          <Button onClick={handleSave}>
            <Check size={16} />
            Save settings
          </Button>
        </div>
      </div>
    </PageShell>
  );
}
