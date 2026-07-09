import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ExternalLink, RefreshCw } from "lucide-react";
import { Button, Spinner } from "./ui";

export const DEFAULT_LOCAL_MODEL = "qwen2.5:3b";

type Phase = "checking" | "installing" | "starting" | "pulling" | "error";

interface Progress {
  phase: string;
  detail: string;
  completed: number;
  total: number;
}

interface OllamaInfo {
  running: boolean;
  managedInstalled: boolean;
  models: string[];
}

const PHASE_LABEL: Record<Exclude<Phase, "error">, string> = {
  checking: "Checking your machine…",
  installing: "Downloading the local AI runtime…",
  starting: "Starting the local AI runtime…",
  pulling: `Downloading your model (${DEFAULT_LOCAL_MODEL})…`,
};

/** Drives the zero-terminal local-model setup: runtime install → serve → model pull. */
export default function LocalBrainSetup({ onReady }: { onReady: (models: string[]) => void }) {
  const [phase, setPhase] = useState<Phase>("checking");
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const started = useRef(false);

  useEffect(() => {
    const un = listen<Progress>("ollama:progress", (e) => setProgress(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const run = async () => {
    setError(null);
    setProgress(null);
    try {
      setPhase("checking");
      let info = await invoke<OllamaInfo>("ollama_status");
      if (!info.running) {
        if (!info.managedInstalled) {
          setPhase("installing");
          await invoke("ollama_install");
        }
        setPhase("starting");
        await invoke("ollama_start");
        info = await invoke<OllamaInfo>("ollama_status");
      }
      if (info.models.length === 0) {
        setPhase("pulling");
        await invoke("ollama_pull", { model: DEFAULT_LOCAL_MODEL });
        info = await invoke<OllamaInfo>("ollama_status");
      }
      onReady(info.models);
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  };

  useEffect(() => {
    if (!started.current) {
      started.current = true;
      run();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (phase === "error") {
    return (
      <div className="space-y-3">
        <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
          {error}
        </p>
        <div className="flex gap-2">
          <Button variant="ghost" onClick={run}>
            <RefreshCw size={16} />
            Try again
          </Button>
          <Button
            variant="ghost"
            onClick={() => invoke("open_url", { url: "https://ollama.com/download" })}
          >
            <ExternalLink size={16} />
            Get Ollama from ollama.com
          </Button>
        </div>
        <p className="text-xs text-gray-500">
          If you install Ollama yourself, come back and press “Try again”.
        </p>
      </div>
    );
  }

  const pct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.completed / progress.total) * 100))
      : null;

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-sm text-gray-300">
        <Spinner />
        {PHASE_LABEL[phase]}
      </div>
      {(phase === "installing" || phase === "pulling") && (
        <div className="h-2 w-full overflow-hidden rounded-full bg-white/10">
          <div
            className="h-full rounded-full bg-donna-accent transition-all"
            style={{ width: `${pct ?? 5}%` }}
          />
        </div>
      )}
      {pct !== null && <p className="text-xs text-gray-500">{pct}%</p>}
      <p className="text-xs text-gray-500">
        Everything runs on your machine — nothing is sent anywhere.
      </p>
    </div>
  );
}
