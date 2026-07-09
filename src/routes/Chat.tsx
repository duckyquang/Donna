import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Mic, Plus, Send, Square, Trash2, X } from "lucide-react";
import { api, type Approval, type Conversation, type Message } from "../lib/api";
import { useConfig } from "../lib/useConfig";
import { Button, Spinner, ThinkingDots } from "../components/ui";
import { DonnaMessage } from "../components/DonnaMessage";
import { hasDonnaQuestions } from "../lib/donnaQuestions";
import * as voice from "../lib/voice";
import ProfileOnboarding from "./ProfileOnboarding";

const PLACEHOLDER_TITLE = "New conversation";

interface ToolEvent {
  name: string;
  label: string;
  status: "running" | "done" | "error";
}

export default function Chat() {
  const { config, save } = useConfig();
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeId, setActiveId] = useState<number | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [streamingText, setStreamingText] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needsProfile, setNeedsProfile] = useState<boolean | null>(null);
  const [toolEvents, setToolEvents] = useState<ToolEvent[]>([]);
  const [pendingApprovals, setPendingApprovals] = useState<Approval[]>([]);
  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const recordingRef = useRef<voice.Recording | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const refreshConversations = async () => {
    const list = await api.listConversations();
    setConversations(list);
    return list;
  };

  const loadMessages = async (id: number): Promise<Message[]> => {
    const rows = await api.getMessages(id);
    setMessages(rows);
    return rows;
  };

  const loadPendingApprovals = async (id: number) => {
    setPendingApprovals(await api.approvalsPendingForConversation(id));
  };

  useEffect(() => {
    if (!config) return;
    refreshConversations().then(async (list) => {
      if (list.length > 0 && !config.profileOnboarded) {
        await save({ ...config, profileOnboarded: true });
        setNeedsProfile(false);
        setActiveId(list[0].id);
        await loadMessages(list[0].id);
        await loadPendingApprovals(list[0].id);
        return;
      }
      const showProfile = !config.profileOnboarded && list.length === 0;
      setNeedsProfile(showProfile);
      if (!showProfile && list.length > 0) {
        setActiveId(list[0].id);
        await loadMessages(list[0].id);
        await loadPendingApprovals(list[0].id);
      }
    });
  }, [config, save]);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [messages, streamingText]);

  const selectConversation = async (id: number) => {
    setActiveId(id);
    setError(null);
    setStreamingText("");
    await loadMessages(id);
    await loadPendingApprovals(id);
  };

  const newConversation = () => {
    setActiveId(null);
    setMessages([]);
    setStreamingText("");
    setError(null);
    setPendingApprovals([]);
  };

  const removeConversation = async (id: number) => {
    await api.deleteConversation(id);
    const list = await refreshConversations();
    if (activeId === id) {
      if (list.length > 0) {
        selectConversation(list[0].id);
      } else {
        newConversation();
      }
    }
  };

  const sendMessage = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || streaming) return;
      setError(null);

      let convId = activeId;
      if (convId === null) {
        convId = await api.createConversation(PLACEHOLDER_TITLE);
        setActiveId(convId);
        await refreshConversations();
      }

      await api.addMessage(convId, "user", trimmed);
      await loadMessages(convId);

      setStreaming(true);
      setStreamingText("");
      setToolEvents([]);
      try {
        await api.sendChat(convId, (event) => {
          if (event.type === "token") {
            setStreamingText((prev) => prev + event.content);
          } else if (event.type === "error") {
            setError(event.message);
          } else if (event.type === "tool") {
            const { name, label, status } = event;
            setToolEvents((prev) => {
              const idx = prev.findIndex((t) => t.name === name && t.label === label);
              if (idx === -1) return [...prev, { name, label, status }];
              const next = [...prev];
              next[idx] = { name, label, status };
              return next;
            });
          } else if (event.type === "approval") {
            loadPendingApprovals(convId);
          }
        });
        const finalMessages = await loadMessages(convId);
        await refreshConversations();
        await loadPendingApprovals(convId);
        api.kgExtract(convId).catch(() => {});
        if (config?.speakReplies) {
          const last = finalMessages[finalMessages.length - 1];
          if (last?.role === "assistant") voice.speak(last.content);
        }
      } catch (e) {
        setError(String(e));
      } finally {
        setStreaming(false);
        setStreamingText("");
        setToolEvents([]);
      }
    },
    [activeId, streaming, config?.speakReplies]
  );

  const handleSend = () => {
    const text = input.trim();
    if (!text) return;
    setInput("");
    sendMessage(text);
  };

  const handleQuestionAnswer = (answer: string) => {
    sendMessage(answer);
  };

  const handleMicClick = async () => {
    if (recording) {
      setRecording(false);
      const rec = recordingRef.current;
      recordingRef.current = null;
      if (!rec) return;
      setTranscribing(true);
      try {
        const blob = await rec.stop();
        const transcript = await voice.transcribeBlob(blob);
        if (transcript.trim()) sendMessage(transcript);
      } catch (e) {
        setError(String(e));
      } finally {
        setTranscribing(false);
      }
      return;
    }

    setError(null);
    try {
      recordingRef.current = await voice.recordAudio();
      setRecording(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleProfileComplete = async (conversationId: number) => {
    setNeedsProfile(false);
    await refreshConversations();
    setActiveId(conversationId);
    await loadMessages(conversationId);
  };

  if (needsProfile === null) {
    return (
      <div className="flex h-full items-center justify-center text-gray-400">
        <Spinner />
      </div>
    );
  }

  if (needsProfile) {
    return <ProfileOnboarding onComplete={handleProfileComplete} />;
  }

  return (
    <div className="flex h-full">
      <div className="flex w-64 flex-col border-r border-white/10 bg-donna-panel">
        <div className="flex items-center justify-between p-3">
          <span className="text-xs font-medium uppercase tracking-wide text-gray-500">
            Conversations
          </span>
          <button
            onClick={newConversation}
            className="rounded-md p-1 text-gray-400 hover:bg-white/10 hover:text-white"
            title="New conversation"
          >
            <Plus size={16} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto px-2 pb-2">
          {conversations.length === 0 && (
            <p className="px-2 py-4 text-xs text-gray-500">No conversations yet.</p>
          )}
          {conversations.map((c) => (
            <div
              key={c.id}
              className={`group mb-1 flex items-center gap-2 rounded-lg px-3 py-2 text-sm ${
                activeId === c.id
                  ? "bg-donna-accent/15 text-donna-accent-light"
                  : "text-gray-400 hover:bg-white/5"
              }`}
            >
              <button
                onClick={() => selectConversation(c.id)}
                className="flex-1 truncate text-left"
              >
                {c.title || "Untitled"}
              </button>
              <button
                onClick={() => removeConversation(c.id)}
                className="opacity-0 transition-opacity group-hover:opacity-100"
                title="Delete"
              >
                <Trash2 size={14} className="text-gray-500 hover:text-red-400" />
              </button>
            </div>
          ))}
        </div>
      </div>

      <div className="flex flex-1 flex-col">
        <header className="flex items-center justify-between border-b border-white/10 px-6 py-3">
          <h1 className="text-sm font-semibold text-white">Chat</h1>
          <span className="text-xs text-gray-500">
            {config ? `${config.provider} · ${config.model || "no model"}` : ""}
          </span>
        </header>

        <div ref={scrollRef} className="flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto max-w-2xl space-y-4">
            {messages.length === 0 && !streaming && (
              <div className="mt-20 text-center text-sm text-gray-500">
                Say hello to Donna — she&apos;ll ask when she needs to know more about you.
              </div>
            )}
            {messages.map((m) => (
              <Bubble
                key={m.id}
                role={m.role}
                content={m.content}
                onQuestionAnswer={m.role === "assistant" ? handleQuestionAnswer : undefined}
              />
            ))}
            {streaming && toolEvents.length > 0 && <ToolStatusList events={toolEvents} />}
            {streaming && !streamingText && <StreamingPlaceholder />}
            {streaming && streamingText && (
              <Bubble
                role="assistant"
                content={streamingText}
                streaming
                onQuestionAnswer={handleQuestionAnswer}
              />
            )}
            {error && (
              <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
                {error}
              </p>
            )}
            {!streaming &&
              pendingApprovals.map((a) => (
                <ApprovalCard
                  key={a.id}
                  approval={a}
                  onResolved={() => {
                    setPendingApprovals((prev) => prev.filter((p) => p.id !== a.id));
                    if (activeId !== null) {
                      const convId = activeId;
                      setTimeout(() => {
                        loadMessages(convId);
                        loadPendingApprovals(convId);
                      }, 800);
                    }
                  }}
                />
              ))}
          </div>
        </div>

        <div className="border-t border-white/10 p-4">
          <div className="mx-auto flex max-w-2xl items-end gap-2">
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleSend();
                }
              }}
              rows={1}
              placeholder="Message Donna…"
              className="max-h-40 flex-1 resize-none rounded-xl border border-white/10 bg-donna-bg px-4 py-3 text-sm text-white outline-none focus:border-donna-accent"
            />
            <button
              onClick={handleMicClick}
              disabled={transcribing}
              title={recording ? "Stop recording" : "Record a voice message"}
              className={`flex h-11 w-11 items-center justify-center rounded-xl border transition-colors disabled:opacity-40 ${
                recording
                  ? "animate-pulse border-red-500 bg-red-500/20 text-red-400"
                  : "border-white/10 text-gray-300 hover:bg-white/10"
              }`}
            >
              {transcribing ? <Spinner /> : recording ? <Square size={16} /> : <Mic size={18} />}
            </button>
            <button
              onClick={handleSend}
              disabled={streaming || !input.trim()}
              className="flex h-11 w-11 items-center justify-center rounded-xl bg-donna-accent text-white transition-colors hover:bg-donna-accent-hover disabled:opacity-40"
            >
              {streaming ? <Spinner /> : <Send size={18} />}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function StreamingPlaceholder() {
  return (
    <div className="flex justify-start">
      <div className="rounded-2xl border border-white/10 bg-donna-surface px-4 py-3">
        <ThinkingDots />
      </div>
    </div>
  );
}

function ToolStatusList({ events }: { events: ToolEvent[] }) {
  return (
    <div className="flex flex-col gap-1.5">
      {events.map((t, i) => (
        <div
          key={`${t.name}-${t.label}-${i}`}
          className="flex w-fit items-center gap-2 rounded-lg border border-white/10 bg-donna-surface px-3 py-1.5 text-xs text-gray-300"
        >
          {t.status === "running" && <Spinner className="h-3 w-3" />}
          {t.status === "done" && <Check size={12} className="text-green-400" />}
          {t.status === "error" && <X size={12} className="text-red-400" />}
          <span>{t.label}</span>
        </div>
      ))}
    </div>
  );
}

function ApprovalCard({
  approval,
  onResolved,
}: {
  approval: Approval;
  onResolved: () => void;
}) {
  const [busy, setBusy] = useState(false);

  const respond = async (approve: boolean) => {
    setBusy(true);
    try {
      await api.approvalRespond(approval.id, approve);
      onResolved();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="rounded-xl border border-donna-accent/30 bg-donna-accent/5 p-4">
      <p className="text-sm text-white">{approval.summary}</p>
      <div className="mt-3 flex gap-2">
        <Button size="sm" variant="success" disabled={busy} onClick={() => respond(true)}>
          Approve
        </Button>
        <Button size="sm" variant="danger" disabled={busy} onClick={() => respond(false)}>
          Reject
        </Button>
      </div>
    </div>
  );
}

function Bubble({
  role,
  content,
  streaming,
  onQuestionAnswer,
}: {
  role: Message["role"];
  content: string;
  streaming?: boolean;
  onQuestionAnswer?: (answer: string) => void;
}) {
  const isUser = role === "user";
  const showQuestions =
    !streaming && !isUser && onQuestionAnswer && hasDonnaQuestions(content);

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[80%] rounded-2xl px-4 py-2.5 text-sm ${
          isUser
            ? "whitespace-pre-wrap bg-donna-accent text-white"
            : "border border-white/10 bg-donna-surface text-gray-100"
        } ${streaming ? "opacity-90" : ""}`}
      >
        {isUser ? (
          content
        ) : (
          <DonnaMessage
            content={content}
            streaming={streaming}
            onAnswer={showQuestions ? onQuestionAnswer : undefined}
          />
        )}
      </div>
    </div>
  );
}
