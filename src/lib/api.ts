import { invoke as tauriInvoke, Channel } from "@tauri-apps/api/core";
import type { ProviderId } from "./models/providers";
import { ensureDesktopApp } from "./tauri";

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  ensureDesktopApp();
  return tauriInvoke<T>(cmd, args);
}

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

export interface KgNode {
  id: string;
  label: string;
  group: string;
  note: string;
  updatedAt: string;
  /** Folder path (category + branches) the node lives in. */
  folder: string[];
  /** File id (slug) within the folder. */
  fileId: string;
  /** info | routine | feedback | preference | person | project | … */
  type: string;
  hasImage: boolean;
}

export interface KgEdge {
  source: string;
  target: string;
}

export interface KgGraph {
  nodes: KgNode[];
  edges: KgEdge[];
}

interface RawKgNode {
  id: string;
  label: string;
  group: string;
  note: string;
  updated_at: string;
  folder: string[];
  file_id: string;
  type: string;
  has_image: boolean;
}

export interface IntegrationStatus {
  id: string;
  name: string;
  connected: boolean;
  needsSetup: boolean;
}

interface RawIntegrationStatus {
  id: string;
  name: string;
  connected: boolean;
  needs_setup: boolean;
}

export interface CalendarEvent {
  id?: string;
  summary?: string;
  description?: string;
  start: string;
  end: string;
  htmlLink?: string;
}

interface RawCalendarEvent {
  id?: string;
  summary?: string;
  description?: string;
  start: string;
  end: string;
  html_link?: string;
}

export interface SlackChannel {
  id: string;
  name: string;
}

function toEvent(e: RawCalendarEvent): CalendarEvent {
  return {
    id: e.id,
    summary: e.summary,
    description: e.description,
    start: e.start,
    end: e.end,
    htmlLink: e.html_link,
  };
}

function fromEvent(e: CalendarEvent): RawCalendarEvent {
  return {
    id: e.id,
    summary: e.summary,
    description: e.description,
    start: e.start,
    end: e.end,
    html_link: e.htmlLink,
  };
}

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

  async kgGraph(): Promise<KgGraph> {
    const g = await invoke<{ nodes: RawKgNode[]; edges: KgEdge[] }>("kg_graph");
    return {
      nodes: g.nodes.map((n) => ({
        id: n.id,
        label: n.label,
        group: n.group,
        note: n.note,
        updatedAt: n.updated_at,
        folder: n.folder,
        fileId: n.file_id,
        type: n.type,
        hasImage: n.has_image,
      })),
      edges: g.edges,
    };
  },

  /** Ask Donna to curate durable knowledge from a conversation. Returns count saved. */
  kgExtract(conversationId: number): Promise<number> {
    return invoke("kg_extract", { conversationId });
  },

  /** Wipe the knowledge base and re-seed default categories. */
  kgReset(): Promise<void> {
    return invoke("kg_reset");
  },

  /** Create or edit a node. Pass fromFolder/fromId when editing/moving an existing one. */
  async kgSaveNode(input: {
    folder: string[];
    label: string;
    note: string;
    type: string;
    fromFolder?: string[];
    fromId?: string;
  }): Promise<KgNode> {
    const n = await invoke<RawKgNode>("kg_save_node", {
      folder: input.folder,
      label: input.label,
      note: input.note,
      nodeType: input.type,
      fromFolder: input.fromFolder ?? null,
      fromId: input.fromId ?? null,
    });
    return {
      id: n.id,
      label: n.label,
      group: n.group,
      note: n.note,
      updatedAt: n.updated_at,
      folder: n.folder,
      fileId: n.file_id,
      type: n.type,
      hasImage: n.has_image,
    };
  },

  kgDeleteNode(folder: string[], id: string): Promise<void> {
    return invoke("kg_delete_node", { folder, id });
  },

  /** Returns the node's image as a data URL, or null. */
  kgNodeImage(folder: string[], id: string): Promise<string | null> {
    return invoke("kg_node_image", { folder, id });
  },

  kgSetNodeImage(folder: string[], id: string, sourcePath: string): Promise<void> {
    return invoke("kg_set_node_image", { folder, id, sourcePath });
  },

  kgRemoveNodeImage(folder: string[], id: string): Promise<void> {
    return invoke("kg_remove_node_image", { folder, id });
  },

  // --- Integrations ---
  async integrationsStatus(): Promise<IntegrationStatus[]> {
    const rows = await invoke<RawIntegrationStatus[]>("integrations_status");
    return rows.map((r) => ({
      id: r.id,
      name: r.name,
      connected: r.connected,
      needsSetup: r.needs_setup,
    }));
  },

  googleSetClient(clientId: string, clientSecret: string): Promise<void> {
    return invoke("google_set_client", { clientId, clientSecret });
  },
  googleConnect(): Promise<void> {
    return invoke("google_connect");
  },
  googleDisconnect(): Promise<void> {
    return invoke("google_disconnect");
  },

  slackSetToken(token: string): Promise<void> {
    return invoke("slack_set_token", { token });
  },
  slackDisconnect(): Promise<void> {
    return invoke("slack_disconnect");
  },
  slackListChannels(): Promise<SlackChannel[]> {
    return invoke("slack_list_channels");
  },
  slackSendMessage(channel: string, text: string): Promise<void> {
    return invoke("slack_send_message", { channel, text });
  },

  fathomSetKey(key: string): Promise<void> {
    return invoke("fathom_set_key", { key });
  },
  fathomDisconnect(): Promise<void> {
    return invoke("fathom_disconnect");
  },

  // --- Calendar (two-way Google Calendar sync) ---
  async calendarListEvents(timeMin: string, timeMax: string): Promise<CalendarEvent[]> {
    const rows = await invoke<RawCalendarEvent[]>("calendar_list_events", {
      timeMin,
      timeMax,
    });
    return rows.map(toEvent);
  },
  async calendarCreateEvent(event: CalendarEvent): Promise<CalendarEvent> {
    return toEvent(
      await invoke<RawCalendarEvent>("calendar_create_event", { event: fromEvent(event) })
    );
  },
  async calendarUpdateEvent(id: string, event: CalendarEvent): Promise<CalendarEvent> {
    return toEvent(
      await invoke<RawCalendarEvent>("calendar_update_event", {
        id,
        event: fromEvent(event),
      })
    );
  },
  calendarDeleteEvent(id: string): Promise<void> {
    return invoke("calendar_delete_event", { id });
  },
};
