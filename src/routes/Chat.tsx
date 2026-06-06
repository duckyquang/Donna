import { useCallback, useEffect, useRef, useState } from "react";
import { Plus, Send, Trash2 } from "lucide-react";
import { api, type Conversation, type Message } from "../lib/api";
import { useConfig } from "../lib/useConfig";
import { Spinner, ThinkingDots } from "../components/ui";
import { DonnaMessage } from "../components/DonnaMessage";
import { hasDonnaQuestions } from "../lib/donnaQuestions";

const PLACEHOLDER_TITLE = "New conversation";

export default function Chat() {
  const { config } = useConfig();
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeId, setActiveId] = useState<number | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [streamingText, setStreamingText] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const refreshConversations = async () => {
    const list = await api.listConversations();
    setConversations(list);
    return list;
  };

  const loadMessages = async (id: number) => {
    setMessages(await api.getMessages(id));
  };

  useEffect(() => {
    refreshConversations().then((list) => {
      if (list.length > 0) {
        setActiveId(list[0].id);
        loadMessages(list[0].id);
      }
    });
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [messages, streamingText]);

  const selectConversation = async (id: number) => {
    setActiveId(id);
    setError(null);
    setStreamingText("");
    await loadMessages(id);
  };

  const newConversation = () => {
    setActiveId(null);
    setMessages([]);
    setStreamingText("");
    setError(null);
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
      try {
        await api.sendChat(convId, (event) => {
          if (event.type === "token") {
            setStreamingText((prev) => prev + event.content);
          } else if (event.type === "error") {
            setError(event.message);
          }
        });
        await loadMessages(convId);
        await refreshConversations();
        api.kgExtract(convId).catch(() => {});
      } catch (e) {
        setError(String(e));
      } finally {
        setStreaming(false);
        setStreamingText("");
      }
    },
    [activeId, streaming]
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
