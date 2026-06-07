import { invoke as tauriInvoke, Channel } from "@tauri-apps/api/core";
import type { ProviderId } from "./models/providers";
import { ensureDesktopApp } from "./tauri";

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  ensureDesktopApp();
  return tauriInvoke<T>(cmd, args);
}

export type AutonomyLevel = "confirm" | "act" | "autonomous";

export interface AppConfig {
  provider: ProviderId;
  model: string;
  ollamaHost: string;
  onboarded: boolean;
  profileOnboarded: boolean;
  autonomyLevel: AutonomyLevel;
}

export interface Routine {
  id: string;
  name: string;
  enabled: boolean;
  dailyTime: string | null;
  prompt: string;
  builtin: boolean;
}

export interface Notification {
  id: number;
  title: string;
  body: string;
  read: boolean;
  createdAt: string;
}

export interface Doc {
  id: string;
  title: string;
  createdAt: string;
  source?: string;
}

export interface DocDetail extends Doc {
  content: string;
}

export interface GmailMessage {
  id: string;
  subject: string;
  from: string;
  snippet: string;
  date: string;
}

export interface BasicFieldStatus {
  id: string;
  label: string;
  promptHint: string;
  known: boolean;
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
  profile_onboarded: boolean;
  autonomy_level?: AutonomyLevel;
}

interface RawRoutine {
  id: string;
  name: string;
  enabled: boolean;
  daily_time: string | null;
  prompt: string;
  builtin: boolean;
}

interface RawNotification {
  id: number;
  title: string;
  body: string;
  read: boolean;
  created_at: string;
}

interface RawDoc {
  id: string;
  title: string;
  created_at: string;
  source?: string;
}

interface RawDocDetail extends RawDoc {
  content: string;
}

interface RawGmailMessage {
  id: string;
  subject: string;
  from: string;
  snippet: string;
  date: string;
}

function toRoutine(r: RawRoutine): Routine {
  return {
    id: r.id,
    name: r.name,
    enabled: r.enabled,
    dailyTime: r.daily_time,
    prompt: r.prompt,
    builtin: r.builtin,
  };
}

function toNotification(n: RawNotification): Notification {
  return {
    id: n.id,
    title: n.title,
    body: n.body,
    read: n.read,
    createdAt: n.created_at,
  };
}

function toDoc(d: RawDoc): Doc {
  return {
    id: d.id,
    title: d.title,
    createdAt: d.created_at,
    source: d.source,
  };
}

function toDocDetail(d: RawDocDetail): DocDetail {
  return { ...toDoc(d), content: d.content };
}

function toGmailMessage(m: RawGmailMessage): GmailMessage {
  return {
    id: m.id,
    subject: m.subject,
    from: m.from,
    snippet: m.snippet,
    date: m.date,
  };
}
interface RawBasicFieldStatus {
  id: string;
  label: string;
  prompt_hint: string;
  known: boolean;
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
      profileOnboarded: c.profile_onboarded ?? false,
      autonomyLevel: c.autonomy_level ?? "confirm",
    };
  },

  async saveConfig(config: AppConfig): Promise<void> {
    await invoke("save_config", {
      config: {
        provider: config.provider,
        model: config.model,
        ollama_host: config.ollamaHost,
        onboarded: config.onboarded,
        profile_onboarded: config.profileOnboarded,
        autonomy_level: config.autonomyLevel,
      },
    });
  },

  async basicsStatus(): Promise<BasicFieldStatus[]> {
    const rows = await invoke<RawBasicFieldStatus[]>("basics_status");
    return rows.map((r) => ({
      id: r.id,
      label: r.label,
      promptHint: r.prompt_hint,
      known: r.known,
    }));
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

  // --- Routines & notifications ---
  async listRoutines(): Promise<Routine[]> {
    const rows = await invoke<RawRoutine[]>("list_routines");
    return rows.map(toRoutine);
  },

  toggleRoutine(id: string, enabled: boolean): Promise<void> {
    return invoke("toggle_routine", { id, enabled });
  },

  async createRoutine(input: {
    name: string;
    dailyTime: string;
    prompt: string;
  }): Promise<Routine> {
    return toRoutine(
      await invoke<RawRoutine>("create_routine", {
        name: input.name,
        dailyTime: input.dailyTime,
        prompt: input.prompt,
      })
    );
  },

  deleteRoutine(id: string): Promise<void> {
    return invoke("delete_routine", { id });
  },

  async listNotifications(): Promise<Notification[]> {
    const rows = await invoke<RawNotification[]>("list_notifications");
    return rows.map(toNotification);
  },

  markNotificationRead(id: number): Promise<void> {
    return invoke("mark_notification_read", { id });
  },

  // --- Docs ---
  async listDocs(): Promise<Doc[]> {
    const rows = await invoke<RawDoc[]>("list_docs");
    return rows.map(toDoc);
  },

  async getDoc(id: string): Promise<DocDetail> {
    return toDocDetail(await invoke<RawDocDetail>("get_doc", { id }));
  },

  deleteDoc(id: string): Promise<void> {
    return invoke("delete_doc", { id });
  },

  // --- Gmail ---
  async gmailListMessages(): Promise<GmailMessage[]> {
    const rows = await invoke<RawGmailMessage[]>("gmail_list_messages");
    return rows.map(toGmailMessage);
  },
};
