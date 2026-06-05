import { invoke, Channel } from "@tauri-apps/api/core";
import type { ProviderId } from "./models/providers";

export interface AppConfig {
  provider: ProviderId;
  model: string;
  ollamaHost: string;
  onboarded: boolean;
}

export interface Conversation {
  id: number;
  title: string;
  createdAt: string;
}

export interface Message {
  id: number;
  conversationId: number;
  role: "system" | "user" | "assistant";
  content: string;
  createdAt: string;
}

export type ChatEvent =
  | { type: "token"; content: string }
  | { type: "done"; messageId: number }
  | { type: "error"; message: string };

// The Rust side serializes config with snake_case; normalize to camelCase here.
interface RawConfig {
  provider: ProviderId;
  model: string;
  ollama_host: string;
  onboarded: boolean;
}
interface RawConversation {
  id: number;
  title: string;
  created_at: string;
}
interface RawMessage {
  id: number;
  conversation_id: number;
  role: Message["role"];
  content: string;
  created_at: string;
}

export const api = {
  async getConfig(): Promise<AppConfig> {
    const c = await invoke<RawConfig>("get_config");
    return {
      provider: c.provider,
      model: c.model,
      ollamaHost: c.ollama_host,
      onboarded: c.onboarded,
    };
  },

  async saveConfig(config: AppConfig): Promise<void> {
    await invoke("save_config", {
      config: {
        provider: config.provider,
        model: config.model,
        ollama_host: config.ollamaHost,
        onboarded: config.onboarded,
      },
    });
  },

  setApiKey(provider: ProviderId, key: string): Promise<void> {
    return invoke("set_api_key", { provider, key });
  },
  hasApiKey(provider: ProviderId): Promise<boolean> {
    return invoke("has_api_key", { provider });
  },
  deleteApiKey(provider: ProviderId): Promise<void> {
    return invoke("delete_api_key", { provider });
  },

  listModels(provider: ProviderId): Promise<string[]> {
    return invoke("list_models", { provider });
  },

  createConversation(title: string): Promise<number> {
    return invoke("create_conversation", { title });
  },
  async listConversations(): Promise<Conversation[]> {
    const rows = await invoke<RawConversation[]>("list_conversations");
    return rows.map((r) => ({ id: r.id, title: r.title, createdAt: r.created_at }));
  },
  renameConversation(id: number, title: string): Promise<void> {
    return invoke("rename_conversation", { id, title });
  },
  deleteConversation(id: number): Promise<void> {
    return invoke("delete_conversation", { id });
  },

  async getMessages(conversationId: number): Promise<Message[]> {
    const rows = await invoke<RawMessage[]>("get_messages", { conversationId });
    return rows.map((r) => ({
      id: r.id,
      conversationId: r.conversation_id,
      role: r.role,
      content: r.content,
      createdAt: r.created_at,
    }));
  },
  addMessage(
    conversationId: number,
    role: Message["role"],
    content: string
  ): Promise<number> {
    return invoke("add_message", { conversationId, role, content });
  },

  /**
   * Stream an assistant reply for a conversation. Returns a promise that resolves when
   * the stream ends; `onEvent` fires for each token, plus a final done/error event.
   */
  async sendChat(
    conversationId: number,
    onEvent: (event: ChatEvent) => void
  ): Promise<void> {
    const channel = new Channel<ChatEvent>();
    channel.onmessage = onEvent;
    await invoke("send_chat", { conversationId, onEvent: channel });
  },
};
