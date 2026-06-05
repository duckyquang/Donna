import type { ChatMessage, ChatOptions, ModelInfo, ModelProvider } from "./types";
import type { ProviderId } from "./providers";

/**
 * Generic cloud provider stub for OpenAI / Anthropic / Google. Each uses the user's
 * own API key, retrieved from the OS keychain via the Rust core (never hard-coded).
 *
 * NOTE: Phase-0 stub. Phase 1 implements per-provider request shaping and streaming.
 */
export class CloudProvider implements ModelProvider {
  constructor(public id: ProviderId) {}

  async listModels(): Promise<ModelInfo[]> {
    // TODO(phase-1): return the provider's available models.
    return [];
  }

  async *chat(_messages: ChatMessage[], _opts: ChatOptions): AsyncIterable<string> {
    // TODO(phase-1): call the provider API with the user's key and stream tokens.
    throw new Error(`CloudProvider(${this.id}).chat not implemented yet (Phase 1).`);
  }
}
