import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Check, Cpu, KeyRound, RefreshCw } from "lucide-react";
import { PROVIDERS, type ProviderId } from "../lib/models/providers";
import { api } from "../lib/api";
import { useConfig } from "../lib/useConfig";
import { Button, Spinner } from "../components/ui";

const DEFAULT_OLLAMA_HOST = "http://localhost:11434";

type Step = "provider" | "configure";

export default function Onboarding() {
  const navigate = useNavigate();
  const { save } = useConfig();

  const [step, setStep] = useState<Step>("provider");
  const [provider, setProvider] = useState<ProviderId>("ollama");
  const [ollamaHost, setOllamaHost] = useState(DEFAULT_OLLAMA_HOST);
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [model, setModel] = useState("");
  const [loadingModels, setLoadingModels] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [finishing, setFinishing] = useState(false);

  const meta = PROVIDERS.find((p) => p.id === provider)!;
  const isLocal = meta.kind === "local";

  const fetchModels = async () => {
    setError(null);
    setLoadingModels(true);
    try {
      if (!isLocal && apiKey.trim()) {
        await api.setApiKey(provider, apiKey.trim());
      }
      if (isLocal) {
        await api.saveConfig({
          provider,
          model: "",
          ollamaHost,
          embedModel: "nomic-embed-text",
          reviewModel: "",
          ttsVoice: "",
          speakReplies: false,
          onboarded: false,
          profileOnboarded: false,
          autonomyLevel: "confirm",
        });
      }
      const list = await api.listModels(provider);
      setModels(list);
      if (list.length > 0) setModel((m) => m || list[0]);
      if (list.length === 0 && isLocal) {
        setError(
          "No local models found. Install Ollama and pull a model (e.g. `ollama pull qwen2.5`), then refresh."
        );
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingModels(false);
    }
  };

  const finish = async () => {
    if (!model) {
      setError("Please choose a model to continue.");
      return;
    }
    setFinishing(true);
    try {
      await save({
        provider,
        model,
        ollamaHost,
        embedModel: "nomic-embed-text",
        reviewModel: "",
        ttsVoice: "",
        speakReplies: false,
        onboarded: true,
        profileOnboarded: false,
        autonomyLevel: "confirm",
      });
      navigate("/chat", { replace: true });
    } catch (e) {
      setError(String(e));
      setFinishing(false);
    }
  };

  return (
    <div className="flex h-full w-full items-center justify-center bg-donna-bg p-6">
      <div className="w-full max-w-xl rounded-2xl border border-white/10 bg-donna-surface p-8">
        <div className="mb-6 flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-donna-accent text-lg font-bold text-white">
            D
          </div>
          <div>
            <h1 className="text-xl font-semibold text-white">Welcome to Donna</h1>
            <p className="text-sm text-gray-400">
              Let&apos;s choose how Donna thinks. You can change this anytime.
            </p>
          </div>
        </div>

        {step === "provider" && (
          <div className="space-y-3">
            {PROVIDERS.map((p) => {
              const selected = p.id === provider;
              return (
                <button
                  key={p.id}
                  onClick={() => setProvider(p.id)}
                  className={`flex w-full items-center gap-3 rounded-xl border p-4 text-left transition-colors ${
                    selected
                      ? "border-donna-accent bg-donna-accent/10"
                      : "border-white/10 hover:bg-white/5"
                  }`}
                >
                  {p.kind === "local" ? (
                    <Cpu size={20} className="text-donna-accent" />
                  ) : (
                    <KeyRound size={20} className="text-donna-accent" />
                  )}
                  <div className="flex-1">
                    <div className="text-sm font-medium text-white">{p.label}</div>
                    <div className="text-xs text-gray-400">{p.description}</div>
                  </div>
                  {selected && <Check size={18} className="text-donna-accent" />}
                </button>
              );
            })}
            <div className="flex justify-end pt-2">
              <Button onClick={() => setStep("configure")}>Continue</Button>
            </div>
          </div>
        )}

        {step === "configure" && (
          <div className="space-y-4">
            {isLocal ? (
              <label className="block">
                <span className="mb-1 block text-sm text-gray-300">Ollama host</span>
                <input
                  value={ollamaHost}
                  onChange={(e) => setOllamaHost(e.target.value)}
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                  placeholder={DEFAULT_OLLAMA_HOST}
                />
                <span className="mt-1 block text-xs text-gray-500">
                  Donna talks to your local Ollama server. Nothing leaves your machine.
                </span>
              </label>
            ) : (
              <label className="block">
                <span className="mb-1 block text-sm text-gray-300">
                  {meta.label} API key
                </span>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                  placeholder="Paste your API key"
                />
                <span className="mt-1 block text-xs text-gray-500">
                  Stored securely in your OS keychain — never on disk or in the repo.
                </span>
              </label>
            )}

            <Button variant="ghost" onClick={fetchModels} disabled={loadingModels}>
              {loadingModels ? <Spinner /> : <RefreshCw size={16} />}
              {isLocal ? "Detect models" : "Verify key & load models"}
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

            {error && (
              <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
                {error}
              </p>
            )}

            <div className="flex justify-between pt-2">
              <Button variant="ghost" onClick={() => setStep("provider")}>
                Back
              </Button>
              <Button onClick={finish} disabled={finishing || !model}>
                {finishing ? <Spinner /> : <Check size={16} />}
                Start using Donna
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
