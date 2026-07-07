// Server client: the desktop UI talks to donna-server over HTTP (RPC) and WebSocket
// (streaming chat + broadcast notifications) instead of Tauri `invoke`. Connection
// config lives in localStorage so the same build works against localhost or a remote box.

export interface ServerConfig {
  url: string;
  token: string;
}

export function serverConfig(): ServerConfig {
  return {
    url: localStorage.getItem("donna.serverUrl") ?? "http://localhost:8377",
    token: localStorage.getItem("donna.serverToken") ?? "",
  };
}

export function setServerConfig(c: ServerConfig) {
  localStorage.setItem("donna.serverUrl", c.url);
  localStorage.setItem("donna.serverToken", c.token);
}

/** POST /rpc/:command with the same camelCase args object the old Tauri invoke used. */
export async function rpc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { url, token } = serverConfig();
  const res = await fetch(`${url}/rpc/${cmd}`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error ?? `rpc ${cmd} failed (${res.status})`);
  }
  return res.json();
}

export async function serverReachable(): Promise<boolean> {
  try {
    const res = await fetch(`${serverConfig().url}/health`, { signal: AbortSignal.timeout(3000) });
    return res.ok;
  } catch {
    return false;
  }
}

type Frame = { type: string; id?: string; event?: unknown; title?: string; body?: string };

let socket: WebSocket | null = null;
const chatHandlers = new Map<string, (ev: unknown) => void>();
const eventHandlers = new Set<(f: Frame) => void>();
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

// ponytail: fixed 5s retry cadence while subscribers exist — no backoff ceiling needed,
// this already caps at "one attempt per 5s forever" which is cheap enough to run indefinitely.
const RECONNECT_MS = 5000;

function ensureSocket(): WebSocket {
  if (socket && socket.readyState <= WebSocket.OPEN) return socket;
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  const { url, token } = serverConfig();
  socket = new WebSocket(`${url.replace(/^http/, "ws")}/ws?token=${encodeURIComponent(token)}`);
  socket.onmessage = (m) => {
    const f: Frame = JSON.parse(m.data);
    if (f.type === "chat_event" && f.id) chatHandlers.get(f.id)?.(f.event);
    else eventHandlers.forEach((h) => h(f));
  };
  socket.onclose = () => {
    socket = null;
    chatHandlers.clear();
    // Only keep trying while someone's actually listening for push notifications
    // (routines/scheduler). Chat re-opens the socket lazily on send anyway.
    if (eventHandlers.size > 0 && !reconnectTimer) {
      reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        ensureSocket();
      }, RECONNECT_MS);
    }
  };
  return socket;
}

/**
 * Stream a chat/quick-chat reply over WS. `onEvent` receives each `ChatEvent`
 * (the server's `event` field, tagged `{type:"token"|"done"|"error", ...}`).
 * The handler is cleaned up on the terminal done/error event.
 */
export function chatStream(
  cmd: "send_chat" | "quick_chat_send",
  payload: Record<string, unknown>,
  onEvent: (ev: unknown) => void,
): void {
  const id = crypto.randomUUID();
  chatHandlers.set(id, (ev) => {
    onEvent(ev);
    const e = ev as { type?: string };
    if (e?.type === "done" || e?.type === "error") chatHandlers.delete(id);
  });
  const ws = ensureSocket();
  const frame = JSON.stringify({ type: "chat", id, cmd, payload });
  if (ws.readyState === WebSocket.OPEN) ws.send(frame);
  else ws.addEventListener("open", () => ws.send(frame), { once: true });
}

/** Subscribe to non-chat server frames (notification broadcasts). Returns an unsubscribe. */
export function onServerEvent(cb: (f: Frame) => void): () => void {
  eventHandlers.add(cb);
  ensureSocket();
  return () => eventHandlers.delete(cb);
}
