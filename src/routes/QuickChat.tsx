/**
 * QuickChat — floating overlay window triggered by Cmd+D.
 *
 * This component renders in its own frameless, transparent Tauri window.
 * It grabs the current screen context (screenshot + frontmost app), lets
 * the user type a question, and streams Donna's reply back inline.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Monitor, Send, X } from "lucide-react";
import { api, type QuickChatCtx } from "../lib/api";
import { Spinner } from "../components/ui";

export default function QuickChat() {
  const [ctx, setCtx] = useState<QuickChatCtx>({ screenshot_b64: null, app_name: "" });
  const [message, setMessage] = useState("");
  const [response, setResponse] = useState("");
  const [sending, setSending] = useState(false);
  const [showScreenshot, setShowScreenshot] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const responseRef = useRef<HTMLDivElement>(null);

  const loadContext = useCallback(async () => {
    try {
      const c = await api.quickChatContext();
      setCtx(c);
      setResponse("");
      setMessage("");
      setShowScreenshot(false);
    } catch {
      // ignore
    }
    setTimeout(() => inputRef.current?.focus(), 80);
  }, []);

  useEffect(() => {
    loadContext();

    const win = getCurrentWebviewWindow();
    let unlisten: (() => void) | undefined;

    win.listen("quick-chat-refresh", () => loadContext()).then((fn) => {
      unlisten = fn;
    });

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") hideWindow();
    };
    window.addEventListener("keydown", onKey);

    return () => {
      window.removeEventListener("keydown", onKey);
      unlisten?.();
    };
  }, [loadContext]);

  // Auto-scroll response
  useEffect(() => {
    if (responseRef.current) {
      responseRef.current.scrollTop = responseRef.current.scrollHeight;
    }
  }, [response]);

  const hideWindow = async () => {
    const win = getCurrentWebviewWindow();
    await win.hide();
  };

  const send = async () => {
    if (!message.trim() || sending) return;
    const q = message.trim();
    setSending(true);
    setResponse("");

    try {
      await api.quickChatSend(q, ctx.app_name || "your current task", (event) => {
        if (event.type === "token") {
          setResponse((prev) => prev + event.content);
        } else if (event.type === "done") {
          setSending(false);
        } else if (event.type === "error") {
          setResponse("Sorry, something went wrong. Try again.");
          setSending(false);
        }
      });
    } catch {
      setResponse("Could not reach Donna. Make sure a model is configured.");
      setSending(false);
    }
  };

  const appLabel = ctx.app_name || "your screen";

  return (
    /* Full transparent window — the panel itself provides the background */
    <div className="qc-root">
      <div className="qc-panel">
        {/* ── Drag region header ── */}
        <div data-tauri-drag-region className="qc-header">
          <div className="flex items-center gap-2 pointer-events-none">
            <div className="flex h-6 w-6 items-center justify-center rounded-md bg-donna-accent text-[11px] font-bold text-white">
              D
            </div>
            <span className="text-xs font-semibold text-white leading-none">
              Ask Donna
            </span>
            {ctx.app_name && (
              <span className="text-[11px] text-gray-500 leading-none">
                · {ctx.app_name}
              </span>
            )}
          </div>

          <div className="flex items-center gap-1">
            {ctx.screenshot_b64 && (
              <button
                onClick={() => setShowScreenshot((v) => !v)}
                title={showScreenshot ? "Hide screenshot" : "Show screenshot"}
                className="rounded p-1 text-gray-600 hover:text-gray-300 transition-colors"
              >
                <Monitor size={13} />
              </button>
            )}
            <button
              onClick={hideWindow}
              title="Close (Esc)"
              className="rounded p-1 text-gray-600 hover:text-gray-300 transition-colors"
            >
              <X size={14} />
            </button>
          </div>
        </div>

        {/* ── Screenshot thumbnail ── */}
        {showScreenshot && ctx.screenshot_b64 && (
          <div className="qc-screenshot">
            <img
              src={`data:image/png;base64,${ctx.screenshot_b64}`}
              alt="Current screen"
              className="w-full rounded-lg opacity-75"
            />
            <p className="mt-1 text-[10px] text-gray-600 text-center">
              Donna can see your screen
            </p>
          </div>
        )}

        {/* ── Response / idle area ── */}
        <div ref={responseRef} className="qc-body">
          {response ? (
            <div className="qc-response">
              <p className="text-xs leading-relaxed text-gray-200 whitespace-pre-wrap">
                {response}
              </p>
              {sending && (
                <span className="inline-block mt-1 h-3.5 w-0.5 animate-pulse rounded-sm bg-donna-accent" />
              )}
            </div>
          ) : sending ? (
            <div className="flex items-center gap-2 text-xs text-gray-500">
              <Spinner /> Donna is thinking…
            </div>
          ) : (
            <p className="text-xs text-gray-600">
              {`What do you want to know about ${appLabel}?`}
            </p>
          )}
        </div>

        {/* ── Input row ── */}
        <div className="qc-input-row">
          <input
            ref={inputRef}
            type="text"
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
            placeholder="Ask anything… (Enter to send, Esc to close)"
            disabled={sending}
            className="qc-input"
          />
          <button
            onClick={send}
            disabled={!message.trim() || sending}
            className="qc-send-btn"
          >
            {sending ? <Spinner /> : <Send size={14} />}
          </button>
        </div>
      </div>
    </div>
  );
}
