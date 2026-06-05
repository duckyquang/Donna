import type { ChatMessage, ChatOptions, ModelInfo, ModelProvider } from "./types";

/**
 * Talks to a local Ollama server. No API key required; nothing leaves the device.
 *
 * NOTE: This is a Phase-0 stub. Phase 1 implements real streaming against
 * `${host}/api/chat` and model discovery via `${host}/api/tags`.
 */
export class OllamaProvider implements ModelProvider {
  id = "ollama";

  constructor(private host = "http://localhost:11434") {}

  async listModels(): Promise<ModelInfo[]> {
    // TODO(phase-1): GET `${this.host}/api/tags`
    return [];
  }

  async *chat(_messages: ChatMessage[], _opts: ChatOptions): AsyncIterable<string> {
    // TODO(phase-1): stream from `${this.host}/api/chat`
    throw new Error("OllamaProvider.chat not implemented yet (Phase 1).");
  }
}
