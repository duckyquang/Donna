export type Role = "system" | "user" | "assistant";

export interface ChatMessage {
  role: Role;
  content: string;
}

export interface ChatOptions {
  model: string;
  temperature?: number;
  signal?: AbortSignal;
}

export interface ModelInfo {
  id: string;
  label: string;
  contextLength?: number;
}

/**
 * Provider-agnostic interface for all model backends. The rest of the app depends
 * only on this contract, never on a specific provider, so local (Ollama) and cloud
 * (OpenAI/Anthropic/Google) backends are interchangeable.
 */
export interface ModelProvider {
  id: string;
  listModels(): Promise<ModelInfo[]>;
  /** Streams the assistant response token-by-token. */
  chat(messages: ChatMessage[], opts: ChatOptions): AsyncIterable<string>;
  embed?(text: string): Promise<number[]>;
}
