export type ProviderId = "ollama" | "openai" | "anthropic" | "google";

export interface ProviderMeta {
  id: ProviderId;
  label: string;
  kind: "local" | "cloud";
  description: string;
}

/** Catalog of supported model providers shown in onboarding and Settings. */
export const PROVIDERS: ProviderMeta[] = [
  {
    id: "ollama",
    label: "Ollama (local)",
    kind: "local",
    description: "Run Qwen, Llama, or Gemma on your machine. Free and fully private.",
  },
  {
    id: "openai",
    label: "OpenAI",
    kind: "cloud",
    description: "GPT models via your own API key.",
  },
  {
    id: "anthropic",
    label: "Anthropic",
    kind: "cloud",
    description: "Claude models via your own API key.",
  },
  {
    id: "google",
    label: "Google",
    kind: "cloud",
    description: "Gemini models via your own API key.",
  },
];
