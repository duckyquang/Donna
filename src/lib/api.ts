import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { ProviderId } from "./models/providers";
import { ensureDesktopApp } from "./tauri";
import { rpc, chatStream } from "./server";

// Native commands must run in-process on the desktop (screen capture, OAuth loopback,
// editor launch, project file I/O). Everything else is routed to donna-server over RPC.
// The two streaming pairs (send_chat, quick_chat_send) go over WS via chatStream().
const NATIVE_COMMANDS = new Set([
  "quick_chat_context",
  "google_set_client",
  "google_connect",
  "export_google_secrets",
  "project_open_in_editor",
  "project_list_files",
  "project_read_file",
  "project_write_file",
  "export_server_bundle",
]);

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (NATIVE_COMMANDS.has(cmd)) {
    ensureDesktopApp();
    return tauriInvoke<T>(cmd, args);
  }
  return rpc<T>(cmd, args);
}

export type AutonomyLevel = "confirm" | "act" | "autonomous";

export interface AppConfig {
  provider: ProviderId;
  model: string;
  ollamaHost: string;
  onboarded: boolean;
  profileOnboarded: boolean;
  autonomyLevel: AutonomyLevel;
  embedModel: string;
  /** Model the nightly background review uses; empty means "use `model`". */
  reviewModel: string;
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

export interface DriveFile {
  id: string;
  name: string;
  mimeType: string | null;
  modifiedTime: string | null;
  webViewLink: string | null;
}

export interface GitHubRepo {
  id: number;
  name: string;
  fullName: string;
  private: boolean;
  htmlUrl: string;
}

export interface GitHubIssue {
  id: number;
  number: number;
  title: string;
  state: string;
  htmlUrl: string;
  repo: string;
}

export interface LinearIssue {
  id: string;
  identifier: string;
  title: string;
  state: string;
  url: string;
}

export interface NotionPage {
  id: string;
  title: string;
  url: string;
  lastEdited: string | null;
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
  | { type: "done"; message_id: number }
  | { type: "error"; message: string }
  | { type: "tool"; name: string; label: string; status: "running" | "done" | "error" }
  | { type: "approval"; approval_id: number; summary: string; tool: string };

export interface Approval {
  id: number;
  conversationId: number;
  tool: string;
  argsJson: string;
  summary: string;
  status: string;
  createdAt: string;
  resolvedAt: string | null;
}

export interface Suggestion {
  id: number;
  kind: string;
  title: string;
  body: string;
  status: string;
  createdAt: string;
}

export interface TrustPolicy {
  actionKind: string;
  mode: "ask" | "auto";
  updatedAt: string;
}

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

export interface Project {
  id: number;
  name: string;
  template: string;
  path: string;
  created_at: string;
}

export interface ProjectFile {
  name: string;
  path: string;
  is_dir: boolean;
}

export interface ReadingListItem {
  id: number;
  url: string;
  title: string;
  summary: string | null;
  tags: string | null;
  read: boolean;
  created_at: string;
}

export interface FocusSession {
  id: number;
  label: string;
  duration_min: number;
  started_at: string;
  ended_at: string | null;
}

export interface Habit {
  id: number;
  name: string;
  description: string | null;
  enabled: boolean;
  created_at: string;
}

export interface NewsItemStructured {
  id: number;
  title: string;
  url: string | null;
  score: number;
  by: string;
}

export interface QuickChatCtx {
  screenshot_b64: string | null;
  app_name: string;
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
  embed_model?: string;
  review_model?: string;
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

interface RawDriveFile {
  id: string;
  name: string;
  mime_type: string | null;
  modified_time: string | null;
  web_view_link: string | null;
}

interface RawGitHubRepo {
  id: number;
  name: string;
  full_name: string;
  private: boolean;
  html_url: string;
}

interface RawGitHubIssue {
  id: number;
  number: number;
  title: string;
  state: string;
  html_url: string;
  repo: string;
}

interface RawNotionPage {
  id: string;
  title: string;
  url: string;
  last_edited: string | null;
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

function toDriveFile(f: RawDriveFile): DriveFile {
  return {
    id: f.id,
    name: f.name,
    mimeType: f.mime_type,
    modifiedTime: f.modified_time,
    webViewLink: f.web_view_link,
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

interface RawApproval {
  id: number;
  conversation_id: number;
  tool: string;
  args_json: string;
  summary: string;
  status: string;
  created_at: string;
  resolved_at: string | null;
}

interface RawSuggestion {
  id: number;
  kind: string;
  title: string;
  body: string;
  payload_json: string | null;
  dedup_key: string;
  status: string;
  created_at: string;
  resolved_at: string | null;
}

interface RawTrustPolicy {
  action_kind: string;
  mode: "ask" | "auto";
  updated_at: string;
}

function toApproval(a: RawApproval): Approval {
  return {
    id: a.id,
    conversationId: a.conversation_id,
    tool: a.tool,
    argsJson: a.args_json,
    summary: a.summary,
    status: a.status,
    createdAt: a.created_at,
    resolvedAt: a.resolved_at,
  };
}

function toSuggestion(s: RawSuggestion): Suggestion {
  return {
    id: s.id,
    kind: s.kind,
    title: s.title,
    body: s.body,
    status: s.status,
    createdAt: s.created_at,
  };
}

function toTrustPolicy(p: RawTrustPolicy): TrustPolicy {
  return {
    actionKind: p.action_kind,
    mode: p.mode,
    updatedAt: p.updated_at,
  };
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
      embedModel: c.embed_model ?? "nomic-embed-text",
      reviewModel: c.review_model ?? "",
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
        embed_model: config.embedModel,
        review_model: config.reviewModel,
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
  sendChat(
    conversationId: number,
    onEvent: (event: ChatEvent) => void
  ): Promise<void> {
    return new Promise((resolve) => {
      chatStream("send_chat", { conversationId }, (ev) => {
        onEvent(ev as ChatEvent);
        const t = (ev as { type?: string }).type;
        if (t === "done" || t === "error") resolve();
      });
    });
  },

  // --- Approvals & trust policies ---
  async approvalsList(): Promise<Approval[]> {
    const rows = await invoke<RawApproval[]>("approvals_list");
    return rows.map(toApproval);
  },
  async approvalsPendingForConversation(conversationId: number): Promise<Approval[]> {
    const rows = await invoke<RawApproval[]>("approvals_pending_for_conversation", {
      conversationId,
    });
    return rows.map(toApproval);
  },
  approvalRespond(id: number, approve: boolean): Promise<string> {
    return invoke("approval_respond", { id, approve });
  },
  async suggestionsList(pendingOnly: boolean): Promise<Suggestion[]> {
    const rows = await invoke<RawSuggestion[]>("suggestions_list", { pendingOnly });
    return rows.map(toSuggestion);
  },
  suggestionRespond(id: number, accept: boolean): Promise<string> {
    return invoke("suggestion_respond", { id, accept });
  },
  async trustPoliciesList(): Promise<TrustPolicy[]> {
    const rows = await invoke<RawTrustPolicy[]>("trust_policies_list");
    return rows.map(toTrustPolicy);
  },
  trustPolicySet(actionKind: string, mode: "ask" | "auto"): Promise<void> {
    return invoke("trust_policy_set", { actionKind, mode });
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

  /** Write the OAuth client to both stores: desktop keychain (native google_connect,
   * export_google_secrets) and the server FileStore (integrations_status gate,
   * server-side Google API calls / token refresh). */
  async googleSetClient(clientId: string, clientSecret: string): Promise<void> {
    await invoke("google_set_client", { clientId, clientSecret });
    await rpc("google_set_client", { clientId, clientSecret });
  },
  googleConnect(): Promise<void> {
    return invoke("google_connect");
  },
  /** Run the desktop OAuth flow, then push the resulting Google secrets to the server. */
  async googleConnectAndSync(): Promise<void> {
    await invoke("google_connect");
    const { client, token } = await invoke<{ client: string; token: string }>("export_google_secrets");
    await rpc("import_google_secrets", { client, token });
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
    const [h, m] = input.dailyTime.split(":").map(Number);
    // ops::create_routine returns the new row id (i64), not a Routine — build the
    // view object locally from the id + the input we already hold.
    const id = await rpc<number>("create_routine", {
      input: {
        name: input.name,
        schedule_type: "daily",
        hour: h,
        minute: m,
        prompt: input.prompt,
      },
    });
    return {
      id: String(id),
      name: input.name,
      enabled: true,
      dailyTime: input.dailyTime,
      prompt: input.prompt,
      builtin: false,
    };
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
    const d = await invoke<RawDocDetail>("get_doc", { id: Number(id) });
    return toDocDetail(d);
  },

  deleteDoc(id: string): Promise<void> {
    return invoke("delete_doc", { id: Number(id) });
  },

  // --- Gmail & Drive ---
  async gmailListMessages(maxResults = 10): Promise<GmailMessage[]> {
    const rows = await invoke<RawGmailMessage[]>("gmail_list_messages", { maxResults });
    return rows.map(toGmailMessage);
  },

  gmailCreateDraft(to: string, subject: string, body: string): Promise<string> {
    return invoke("gmail_create_draft", { to, subject, body });
  },

  async driveListFiles(maxResults = 10): Promise<DriveFile[]> {
    const rows = await invoke<RawDriveFile[]>("drive_list_files", { maxResults });
    return rows.map(toDriveFile);
  },

  googleCreateDoc(title: string): Promise<string> {
    return invoke("google_create_doc", { title });
  },

  // --- GitHub ---
  githubSetToken(token: string): Promise<void> {
    return invoke("github_set_token", { token });
  },
  githubDisconnect(): Promise<void> {
    return invoke("github_disconnect");
  },
  async githubListRepos(maxResults = 10): Promise<GitHubRepo[]> {
    const rows = await invoke<RawGitHubRepo[]>("github_list_repos", { maxResults });
    return rows.map((r) => ({
      id: r.id,
      name: r.name,
      fullName: r.full_name,
      private: r.private,
      htmlUrl: r.html_url,
    }));
  },
  async githubListIssues(maxResults = 10): Promise<GitHubIssue[]> {
    const rows = await invoke<RawGitHubIssue[]>("github_list_issues", { maxResults });
    return rows.map((i) => ({
      id: i.id,
      number: i.number,
      title: i.title,
      state: i.state,
      htmlUrl: i.html_url,
      repo: i.repo,
    }));
  },

  // --- Linear ---
  linearSetKey(key: string): Promise<void> {
    return invoke("linear_set_key", { key });
  },
  linearDisconnect(): Promise<void> {
    return invoke("linear_disconnect");
  },
  async linearListIssues(maxResults = 10): Promise<LinearIssue[]> {
    return invoke<LinearIssue[]>("linear_list_issues", { maxResults });
  },

  // --- Notion ---
  notionSetToken(token: string): Promise<void> {
    return invoke("notion_set_token", { token });
  },
  notionDisconnect(): Promise<void> {
    return invoke("notion_disconnect");
  },
  async notionSearchPages(maxResults = 10): Promise<NotionPage[]> {
    const rows = await invoke<RawNotionPage[]>("notion_search_pages", { maxResults });
    return rows.map((p) => ({
      id: p.id,
      title: p.title,
      url: p.url,
      lastEdited: p.last_edited,
    }));
  },

  // --- Telegram ---
  telegramSetCredentials(botToken: string, chatId: string): Promise<void> {
    return invoke("telegram_set_credentials", { botToken, chatId });
  },
  telegramDisconnect(): Promise<void> {
    return invoke("telegram_disconnect");
  },
  telegramSendMessage(text: string): Promise<void> {
    return invoke("telegram_send_message", { text });
  },

  // --- WhatsApp ---
  whatsappSetCredentials(accessToken: string, phoneNumberId: string): Promise<void> {
    return invoke("whatsapp_set_credentials", { accessToken, phoneNumberId });
  },
  whatsappDisconnect(): Promise<void> {
    return invoke("whatsapp_disconnect");
  },
  whatsappSendMessage(to: string, text: string): Promise<void> {
    return invoke("whatsapp_send_message", { to, text });
  },
  whatsappSetMyNumber(number: string): Promise<void> {
    return invoke("whatsapp_set_my_number", { number });
  },
  whatsappGetMyNumber(): Promise<string | null> {
    return invoke("whatsapp_get_my_number");
  },

  kgReindexEmbeddings(): Promise<number> {
    return invoke("kg_reindex_embeddings");
  },

  // --- Projects ---
  projectList: () => invoke<Project[]>("project_list"),
  projectCreate: (name: string, template: string, path: string) =>
    invoke<Project>("project_create", { name, template, path }),
  projectDelete: (id: number) => invoke<void>("project_delete", { id }),
  projectOpenInEditor: (path: string) => invoke<void>("project_open_in_editor", { path }),
  // Native project file I/O takes the project root path directly (from project_list),
  // so the desktop never needs to open the DB to resolve it.
  projectListFiles: (projectPath: string) => invoke<ProjectFile[]>("project_list_files", { projectPath }),
  projectReadFile: (projectPath: string, path: string) => invoke<string>("project_read_file", { projectPath, path }),
  projectWriteFile: (projectPath: string, path: string, content: string) =>
    invoke<void>("project_write_file", { projectPath, path, content }),

  // --- Discord ---
  discordSetToken: (token: string) => invoke<void>("discord_set_token", { token }),
  discordDisconnect: () => invoke<void>("discord_disconnect"),

  // --- Fathom extra ---
  fathomProcessRecentMeeting: () => invoke<string>("fathom_process_recent_meeting"),

  // --- News ---
  newsFetchLatest: () => invoke<string>("news_fetch_latest"),

  // --- Reading list ---
  readingListAdd: (url: string, title: string) => invoke<ReadingListItem>("reading_list_add", { url, title }),
  readingListGet: () => invoke<ReadingListItem[]>("reading_list_get"),
  readingListSummarize: (id: number) => invoke<string>("reading_list_summarize", { id }),
  readingListDelete: (id: number) => invoke<void>("reading_list_delete", { id }),

  // --- Focus sessions ---
  focusStart: (label: string, durationMin: number) => invoke<FocusSession>("focus_start", { label, duration_min: durationMin }),
  focusEnd: (id: number) => invoke<void>("focus_end", { id }),
  focusActive: () => invoke<FocusSession | null>("focus_active"),

  // --- Habits ---
  habitCreate: (name: string, description?: string) => invoke<Habit>("habit_create", { name, description: description ?? null }),
  habitList: () => invoke<Habit[]>("habit_list"),
  habitLog: (habitId: number, note?: string) => invoke<void>("habit_log", { habit_id: habitId, note: note ?? null }),
  habitLoggedToday: (habitId: number) => invoke<boolean>("habit_logged_today", { habit_id: habitId }),

  // --- Project extras ---
  // Status report is a hybrid: the desktop walks the local project folder (native),
  // then the server generates the report from those contents and saves it as a doc +
  // notification (it holds the provider config and the shared DB).
  async projectStatusReport(projectPath: string, name: string, template: string): Promise<string> {
    ensureDesktopApp();
    const fileContents = await tauriInvoke<string>("project_status_report", { projectPath });
    return rpc<string>("project_status_report", { name, template, fileContents });
  },

  // --- News (structured) ---
  newsListItems: (limit = 10) => invoke<NewsItemStructured[]>("news_list_items", { limit }),
  newsArticleSummary: (url: string) => invoke<string>("news_article_summary", { url }),

  // --- Quick Chat (Cmd+D overlay) ---
  quickChatContext: () => invoke<QuickChatCtx>("quick_chat_context"),
  /** Streams a quick-chat reply. Returns a promise that resolves when the stream ends. */
  quickChatSend(
    message: string,
    appName: string,
    onEvent: (event: ChatEvent) => void
  ): Promise<void> {
    return new Promise((resolve) => {
      chatStream("quick_chat_send", { message, appName }, (ev) => {
        onEvent(ev as ChatEvent);
        const t = (ev as { type?: string }).type;
        if (t === "done" || t === "error") resolve();
      });
    });
  },

  // --- Migration: export old local data for donna-server import ---
  exportServerBundle: (destDir: string) => invoke<string>("export_server_bundle", { destDir }),
};
