import { useEffect, useState, useRef } from "react";
import {
  BookMarked,
  Plus,
  Trash2,
  Play,
  Square,
  CheckCircle2,
  Circle,
  Newspaper,
  Timer,
  Repeat2,
  ExternalLink,
} from "lucide-react";
import { Button, Badge, Card, EmptyState, Input, Spinner } from "../components/ui";
import {
  api,
  type ReadingListItem,
  type FocusSession,
  type Habit,
} from "../lib/api";
import { DonnaMessage } from "../components/DonnaMessage";

type Tab = "news" | "reading" | "focus" | "habits";

export default function Productivity() {
  const [tab, setTab] = useState<Tab>("news");
  const [news, setNews] = useState<string>("");
  const [newsLoading, setNewsLoading] = useState(false);

  const [readingList, setReadingList] = useState<ReadingListItem[]>([]);
  const [addUrl, setAddUrl] = useState("");
  const [addTitle, setAddTitle] = useState("");
  const [summarizing, setSummarizing] = useState<number | null>(null);

  const [activeSession, setActiveSession] = useState<FocusSession | null>(null);
  const [focusLabel, setFocusLabel] = useState("");
  const [focusDuration, setFocusDuration] = useState(25);
  const [elapsed, setElapsed] = useState(0);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const [habits, setHabits] = useState<Habit[]>([]);
  const [loggedToday, setLoggedToday] = useState<Record<number, boolean>>({});
  const [newHabitName, setNewHabitName] = useState("");

  // Load reading list
  const loadReading = async () => {
    try { setReadingList(await api.readingListGet()); } catch {}
  };

  // Load habits + today's logs
  const loadHabits = async () => {
    try {
      const list = await api.habitList();
      setHabits(list);
      const logged: Record<number, boolean> = {};
      for (const h of list) {
        logged[h.id] = await api.habitLoggedToday(h.id).catch(() => false);
      }
      setLoggedToday(logged);
    } catch {}
  };

  // Load active focus session
  const loadFocus = async () => {
    try {
      const session = await api.focusActive();
      setActiveSession(session);
      if (session) {
        const started = new Date(session.started_at).getTime();
        setElapsed(Math.floor((Date.now() - started) / 1000));
      }
    } catch {}
  };

  useEffect(() => { loadReading(); loadHabits(); loadFocus(); }, []);

  // Focus timer tick
  useEffect(() => {
    if (activeSession && !timerRef.current) {
      timerRef.current = setInterval(() => setElapsed((e) => e + 1), 1000);
    } else if (!activeSession && timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
      setElapsed(0);
    }
    return () => { if (timerRef.current) clearInterval(timerRef.current); };
  }, [activeSession]);

  const loadNews = async () => {
    setNewsLoading(true);
    try { setNews(await api.newsFetchLatest()); } catch { setNews("Could not fetch news. Check your internet connection."); }
    finally { setNewsLoading(false); }
  };

  useEffect(() => { if (tab === "news" && !news) loadNews(); }, [tab]);

  const addToReadingList = async () => {
    if (!addUrl.trim()) return;
    const title = addTitle.trim() || addUrl;
    try {
      const item = await api.readingListAdd(addUrl.trim(), title);
      setReadingList((prev) => [item, ...prev]);
      setAddUrl(""); setAddTitle("");
    } catch {}
  };

  const summarizeItem = async (id: number) => {
    setSummarizing(id);
    try {
      const summary = await api.readingListSummarize(id);
      setReadingList((prev) => prev.map((i) => i.id === id ? { ...i, summary, read: true } : i));
    } catch {} finally { setSummarizing(null); }
  };

  const deleteReadingItem = async (id: number) => {
    await api.readingListDelete(id).catch(() => {});
    setReadingList((prev) => prev.filter((i) => i.id !== id));
  };

  const startFocus = async () => {
    if (!focusLabel.trim()) return;
    try {
      const session = await api.focusStart(focusLabel.trim(), focusDuration);
      setActiveSession(session); setElapsed(0);
    } catch {}
  };

  const endFocus = async () => {
    if (!activeSession) return;
    await api.focusEnd(activeSession.id).catch(() => {});
    setActiveSession(null);
  };

  const logHabit = async (id: number) => {
    await api.habitLog(id).catch(() => {});
    setLoggedToday((prev) => ({ ...prev, [id]: true }));
  };

  const createHabit = async () => {
    if (!newHabitName.trim()) return;
    const h = await api.habitCreate(newHabitName.trim()).catch(() => null);
    if (h) { setHabits((prev) => [...prev, h]); setNewHabitName(""); }
  };

  const formatElapsed = (s: number) => {
    const m = Math.floor(s / 60); const sec = s % 60;
    return `${m}:${sec.toString().padStart(2, "0")}`;
  };
  const focusProgress = activeSession ? Math.min(elapsed / (activeSession.duration_min * 60) * 100, 100) : 0;
  const remaining = activeSession ? Math.max(activeSession.duration_min * 60 - elapsed, 0) : 0;

  const tabs: { id: Tab; label: string; icon: typeof Newspaper }[] = [
    { id: "news", label: "News", icon: Newspaper },
    { id: "reading", label: "Reading", icon: BookMarked },
    { id: "focus", label: "Focus", icon: Timer },
    { id: "habits", label: "Habits", icon: Repeat2 },
  ];

  return (
    <div className="flex h-full flex-col">
      {/* Tab bar */}
      <div className="flex h-14 items-center gap-1 border-b border-donna-border px-4">
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`flex items-center gap-1.5 rounded px-3 py-1.5 text-sm transition-colors ${
              tab === id
                ? "bg-donna-accent-dim text-donna-accent-light font-medium"
                : "text-donna-muted-light hover:bg-donna-surface-hover hover:text-donna-text"
            }`}
          >
            <Icon size={14} />
            {label}
          </button>
        ))}
        {activeSession && (
          <div className="ml-auto flex items-center gap-2 rounded border border-donna-accent/30 bg-donna-accent-dim px-3 py-1 text-xs text-donna-accent-light">
            <Timer size={12} className="animate-pulse" />
            <span>{focusLabel || activeSession.label} — {formatElapsed(remaining)} left</span>
          </div>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {/* NEWS */}
        {tab === "news" && (
          <div className="mx-auto max-w-2xl p-6">
            <div className="mb-4 flex items-center justify-between">
              <h2 className="text-sm font-semibold text-donna-text">Today's Tech News</h2>
              <Button size="sm" variant="ghost" onClick={loadNews} disabled={newsLoading}>
                {newsLoading ? <Spinner /> : <Repeat2 size={12} />} Refresh
              </Button>
            </div>
            {newsLoading ? (
              <div className="flex justify-center py-16"><Spinner /></div>
            ) : news ? (
              <DonnaMessage content={news} />
            ) : (
              <EmptyState icon={<Newspaper size={24} />} title="No news loaded" description="Click Refresh to fetch today's Hacker News top stories" action={<Button size="sm" onClick={loadNews}>Load News</Button>} />
            )}
          </div>
        )}

        {/* READING LIST */}
        {tab === "reading" && (
          <div className="mx-auto max-w-2xl p-6">
            <div className="mb-4">
              <div className="flex gap-2">
                <Input value={addUrl} onChange={(e) => setAddUrl(e.target.value)} placeholder="https://..." className="flex-1" onKeyDown={(e) => e.key === "Enter" && addToReadingList()} />
                <Input value={addTitle} onChange={(e) => setAddTitle(e.target.value)} placeholder="Title (optional)" className="w-48" />
                <Button size="sm" onClick={addToReadingList} disabled={!addUrl.trim()}>
                  <Plus size={12} /> Add
                </Button>
              </div>
            </div>
            {readingList.length === 0 ? (
              <EmptyState icon={<BookMarked size={24} />} title="Reading list is empty" description="Add URLs above to save articles for later" />
            ) : (
              <div className="space-y-3">
                {readingList.map((item) => (
                  <Card key={item.id} className="p-4">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <a href={item.url} target="_blank" rel="noopener noreferrer" className="truncate text-sm font-medium text-donna-text hover:text-donna-accent-light">
                            {item.title}
                          </a>
                          <ExternalLink size={11} className="shrink-0 text-donna-muted" />
                          {item.read && <Badge variant="success">Read</Badge>}
                        </div>
                        <p className="mt-0.5 truncate text-xs text-donna-muted">{item.url}</p>
                        {item.summary && (
                          <div className="mt-2 text-xs text-donna-text-secondary leading-relaxed">
                            <DonnaMessage content={item.summary} />
                          </div>
                        )}
                      </div>
                      <div className="flex shrink-0 gap-1">
                        {!item.summary && (
                          <Button size="sm" variant="ghost" onClick={() => summarizeItem(item.id)} disabled={summarizing === item.id}>
                            {summarizing === item.id ? <Spinner /> : "Summarize"}
                          </Button>
                        )}
                        <button onClick={() => deleteReadingItem(item.id)} className="rounded p-1 text-donna-muted hover:text-red-400 transition-colors">
                          <Trash2 size={13} />
                        </button>
                      </div>
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </div>
        )}

        {/* FOCUS */}
        {tab === "focus" && (
          <div className="mx-auto max-w-md p-8 text-center">
            {activeSession ? (
              <div className="flex flex-col items-center gap-6">
                {/* Circular progress */}
                <div className="relative h-40 w-40">
                  <svg className="h-full w-full -rotate-90" viewBox="0 0 100 100">
                    <circle cx="50" cy="50" r="44" fill="none" stroke="rgba(255,255,255,0.05)" strokeWidth="8" />
                    <circle
                      cx="50" cy="50" r="44" fill="none"
                      stroke="#c9742a" strokeWidth="8" strokeLinecap="round"
                      strokeDasharray={`${2 * Math.PI * 44}`}
                      strokeDashoffset={`${2 * Math.PI * 44 * (1 - focusProgress / 100)}`}
                      className="transition-all duration-1000"
                    />
                  </svg>
                  <div className="absolute inset-0 flex flex-col items-center justify-center">
                    <span className="text-2xl font-mono font-semibold text-donna-text">{formatElapsed(remaining)}</span>
                    <span className="text-xs text-donna-muted">remaining</span>
                  </div>
                </div>
                <div>
                  <p className="text-base font-medium text-donna-text">{activeSession.label}</p>
                  <p className="text-xs text-donna-muted">{activeSession.duration_min}-minute focus block</p>
                </div>
                <Button variant="danger" onClick={endFocus}>
                  <Square size={14} /> End session
                </Button>
              </div>
            ) : (
              <div className="flex flex-col gap-4">
                <div className="mb-2">
                  <Timer size={36} className="mx-auto mb-3 text-donna-muted" />
                  <p className="text-base font-medium text-donna-text">Start a Focus Session</p>
                  <p className="mt-1 text-sm text-donna-muted">Block time for deep work. Donna tracks it and notifies you when done.</p>
                </div>
                <Input value={focusLabel} onChange={(e) => setFocusLabel(e.target.value)} placeholder="What are you focusing on?" onKeyDown={(e) => e.key === "Enter" && startFocus()} />
                <div className="flex items-center justify-center gap-3">
                  {[25, 45, 90].map((d) => (
                    <button key={d} onClick={() => setFocusDuration(d)}
                      className={`rounded-full border px-4 py-1.5 text-sm transition-colors ${focusDuration === d ? "border-donna-accent/40 bg-donna-accent-dim text-donna-accent-light" : "border-donna-border text-donna-muted hover:text-donna-text"}`}>
                      {d}m
                    </button>
                  ))}
                </div>
                <Button onClick={startFocus} disabled={!focusLabel.trim()}>
                  <Play size={14} /> Start {focusDuration}m session
                </Button>
              </div>
            )}
          </div>
        )}

        {/* HABITS */}
        {tab === "habits" && (
          <div className="mx-auto max-w-md p-6">
            <div className="mb-4 flex gap-2">
              <Input value={newHabitName} onChange={(e) => setNewHabitName(e.target.value)} placeholder="New habit…" onKeyDown={(e) => e.key === "Enter" && createHabit()} />
              <Button size="sm" onClick={createHabit} disabled={!newHabitName.trim()}><Plus size={12} /> Add</Button>
            </div>
            {habits.length === 0 ? (
              <EmptyState icon={<Repeat2 size={24} />} title="No habits tracked" description="Add a daily habit to track your streaks" />
            ) : (
              <div className="space-y-2">
                {habits.map((h) => {
                  const done = loggedToday[h.id] ?? false;
                  return (
                    <Card key={h.id} className={`flex items-center gap-3 p-4 transition-colors ${done ? "opacity-60" : ""}`}>
                      <button onClick={() => !done && logHabit(h.id)} className={`shrink-0 transition-colors ${done ? "text-donna-success" : "text-donna-muted hover:text-donna-accent"}`}>
                        {done ? <CheckCircle2 size={20} /> : <Circle size={20} />}
                      </button>
                      <div className="flex-1 min-w-0">
                        <p className={`text-sm font-medium ${done ? "line-through text-donna-muted" : "text-donna-text"}`}>{h.name}</p>
                        {h.description && <p className="text-xs text-donna-muted truncate">{h.description}</p>}
                      </div>
                      {done && <Badge variant="success">Done today</Badge>}
                    </Card>
                  );
                })}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
