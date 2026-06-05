import { useEffect, useState } from "react";
import { Check, RefreshCw, Trash2 } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { PROVIDERS, type ProviderId } from "../lib/models/providers";
import { api } from "../lib/api";
import { useConfig } from "../lib/useConfig";
import { Button, Spinner } from "../components/ui";

export default function Settings() {
  const { config, save, refresh } = useConfig();

  const [provider, setProvider] = useState<ProviderId>("ollama");
  const [model, setModel] = useState("");
  const [ollamaHost, setOllamaHost] = useState("http://localhost:11434");
  const [apiKey, setApiKey] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const meta = PROVIDERS.find((p) => p.id === provider)!;
  const isLocal = meta.kind === "local";

  useEffect(() => {
    if (config) {
      setProvider(config.provider);
      setModel(config.model);
      setOllamaHost(config.ollamaHost);
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
        await api.saveConfig({ provider, model, ollamaHost, onboarded: true });
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
      await save({ provider, model, ollamaHost, onboarded: true });
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
            <label className="block">
              <span className="mb-1 block text-sm text-gray-300">Ollama host</span>
              <input
                value={ollamaHost}
                onChange={(e) => setOllamaHost(e.target.value)}
                className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
              />
            </label>
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
