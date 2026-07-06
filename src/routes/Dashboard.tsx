import { useCallback, useEffect, useState, type ElementType, type ReactNode } from "react";
import {
  RefreshCw,
  Mail,
  CalendarDays,
  Newspaper,
  Bell,
  ExternalLink,
  MessageSquare,
  ChevronDown,
  ChevronUp,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Spinner } from "../components/ui";
import {
  api,
  type GmailMessage,
  type CalendarEvent,
  type Notification,
  type NewsItemStructured,
} from "../lib/api";

function greeting() {
  const h = new Date().getHours();
  if (h < 12) return "Good morning";
  if (h < 18) return "Good afternoon";
  return "Good evening";
}

function todayRange(): { min: string; max: string } {
  const now = new Date();
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  const end = new Date(now);
  end.setHours(23, 59, 59, 999);
  return { min: start.toISOString(), max: end.toISOString() };
}

function formatTime(iso: string) {
  try {
    return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  } catch {
    return iso;
  }
}

function formatDate() {
  return new Date().toLocaleDateString([], {
    weekday: "long",
    month: "long",
    day: "numeric",
  });
}

// ── News card with inline summary ─────────────────────────────────────────────

function NewsCard({ item, rank }: { item: NewsItemStructured; rank: number }) {
  const [expanded, setExpanded] = useState(false);
  const [summary, setSummary] = useState<string | null>(null);
  const [loadingSummary, setLoadingSummary] = useState(false);

  const articleUrl = item.url ?? `https://news.ycombinator.com/item?id=${item.id}`;
  const hnUrl = `https://news.ycombinator.com/item?id=${item.id}`;

  const handleExpand = async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (summary !== null) return; // already loaded

    if (!item.url) {
      // HN-native text post — no external article to summarise
      setSummary("This is a Hacker News discussion post with no external article.");
      return;
    }

    setLoadingSummary(true);
    try {
      const text = await api.newsArticleSummary(item.url);
      setSummary(text);
    } catch {
      setSummary("Couldn't load summary — try again later.");
    } finally {
      setLoadingSummary(false);
    }
  };

  return (
    <div className="border-b border-white/5 py-2.5 last:border-0 last:pb-0 first:pt-0">
      {/* Title row */}
      <div className="flex items-start gap-2.5">
        <span className="mt-0.5 flex-shrink-0 w-4 text-[11px] text-gray-600 tabular-nums">
          {rank}.
        </span>
        <div className="min-w-0 flex-1">
          <button
            onClick={handleExpand}
            className="text-left text-xs font-medium text-gray-200 hover:text-white leading-snug transition-colors"
          >
            {item.title}
          </button>
          {/* Meta row */}
          <div className="mt-0.5 flex items-center gap-2 text-[10px] text-gray-600">
            <span>↑{item.score}</span>
            <span>by {item.by}</span>
            <a
              href={hnUrl}
              target="_blank"
              rel="noreferrer"
              className="flex items-center gap-0.5 text-gray-600 hover:text-donna-accent-light"
              onClick={(e) => e.stopPropagation()}
            >
              discuss <ExternalLink size={9} />
            </a>
            {item.url && (
              <a
                href={articleUrl}
                target="_blank"
                rel="noreferrer"
                className="flex items-center gap-0.5 text-gray-600 hover:text-donna-accent-light"
                onClick={(e) => e.stopPropagation()}
              >
                open <ExternalLink size={9} />
              </a>
            )}
            <button
              onClick={handleExpand}
              className="ml-auto flex items-center gap-0.5 text-gray-600 hover:text-gray-400"
            >
              {expanded ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
              {expanded ? "hide" : "read"}
            </button>
          </div>

          {/* Expandable summary */}
          {expanded && (
            <div className="mt-2 rounded-lg bg-white/[0.03] border border-white/[0.06] px-3 py-2.5">
              {loadingSummary ? (
                <div className="flex items-center gap-2 text-xs text-gray-500">
                  <Spinner /> Summarising…
                </div>
              ) : (
                <p className="text-xs leading-relaxed text-gray-400 whitespace-pre-wrap">
                  {summary}
                </p>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Generic section card ───────────────────────────────────────────────────────

function SectionCard({
  icon: Icon,
  title,
  badge,
  loading,
  empty,
  action,
  children,
}: {
  icon: ElementType;
  title: string;
  badge?: number;
  loading?: boolean;
  empty?: string;
  action?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="rounded-xl border border-white/10 bg-donna-surface flex flex-col">
      <div className="flex items-center justify-between border-b border-white/8 px-4 py-3">
        <div className="flex items-center gap-2.5">
          <Icon size={14} className="text-donna-accent-light" />
          <span className="text-sm font-medium text-white">{title}</span>
          {badge !== undefined && badge > 0 && (
            <span className="rounded-full bg-donna-accent/20 px-1.5 py-0.5 text-[10px] font-semibold text-donna-accent-light">
              {badge}
            </span>
          )}
        </div>
        {action}
      </div>
      <div className="flex-1 p-4">
        {loading ? (
          <div className="flex items-center gap-2 text-xs text-gray-500">
            <Spinner /> Loading…
          </div>
        ) : empty && !children ? (
          <p className="text-xs text-gray-500 italic">{empty}</p>
        ) : (
          children
        )}
      </div>
    </div>
  );
}

// ── Dashboard page ─────────────────────────────────────────────────────────────

export default function Dashboard() {
  const navigate = useNavigate();

  const [newsItems, setNewsItems] = useState<NewsItemStructured[]>([]);
  const [newsLoading, setNewsLoading] = useState(true);
  const [newsError, setNewsError] = useState(false);

  const [emails, setEmails] = useState<GmailMessage[]>([]);
  const [emailsLoading, setEmailsLoading] = useState(false);
  const [emailsSkipped, setEmailsSkipped] = useState(false);

  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [eventsLoading, setEventsLoading] = useState(false);
  const [eventsSkipped, setEventsSkipped] = useState(false);

  const [notifs, setNotifs] = useState<Notification[]>([]);
  const [notifsLoading, setNotifsLoading] = useState(true);

  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const loadAll = useCallback(async () => {
    // News — structured items
    setNewsLoading(true);
    setNewsError(false);
    api
      .newsListItems(10)
      .then((items) => setNewsItems(items))
      .catch(() => setNewsError(true))
      .finally(() => setNewsLoading(false));

    // Notifications
    setNotifsLoading(true);
    api
      .listNotifications()
      .then((n) => setNotifs(n.filter((x) => !x.read).slice(0, 5)))
      .catch(() => setNotifs([]))
      .finally(() => setNotifsLoading(false));

    // Google-gated data
    const statuses = await api.integrationsStatus().catch(() => []);
    const googleOn = statuses.find((s) => s.id === "google")?.connected;

    if (googleOn) {
      setEmailsLoading(true);
      setEventsLoading(true);
      setEmailsSkipped(false);
      setEventsSkipped(false);

      api
        .gmailListMessages(5)
        .then(setEmails)
        .catch(() => setEmails([]))
        .finally(() => setEmailsLoading(false));

      const { min, max } = todayRange();
      api
        .calendarListEvents(min, max)
        .then(setEvents)
        .catch(() => setEvents([]))
        .finally(() => setEventsLoading(false));
    } else {
      setEmailsSkipped(true);
      setEventsSkipped(true);
      setEmailsLoading(false);
      setEventsLoading(false);
    }

    setLastRefresh(new Date());
  }, []);

  const handleRefresh = async () => {
    setRefreshing(true);
    await loadAll().catch(() => {});
    setRefreshing(false);
  };

  useEffect(() => {
    loadAll();
  }, [loadAll]);

  return (
    <div className="h-full overflow-y-auto px-8 py-10">
      <div className="mx-auto max-w-5xl">
        {/* Header */}
        <div className="mb-8 flex items-end justify-between">
          <div>
            <h1 className="text-2xl font-semibold text-white">{greeting()} 👋</h1>
            <p className="mt-1 text-sm text-gray-500">{formatDate()}</p>
          </div>
          <div className="flex items-center gap-3">
            {lastRefresh && (
              <span className="text-[11px] text-gray-600">
                Updated{" "}
                {lastRefresh.toLocaleTimeString([], {
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </span>
            )}
            <button
              onClick={handleRefresh}
              disabled={refreshing}
              className="flex items-center gap-1.5 rounded-lg border border-white/10 px-3 py-1.5 text-xs text-gray-400 hover:bg-white/5 hover:text-gray-200 disabled:opacity-40 transition-colors"
            >
              {refreshing ? <Spinner /> : <RefreshCw size={12} />}
              Refresh
            </button>
          </div>
        </div>

        {/* Grid */}
        <div className="grid grid-cols-5 gap-4">
          {/* News — wide left column */}
          <div className="col-span-3">
            <SectionCard
              icon={Newspaper}
              title="Tech & AI News"
              loading={newsLoading}
              empty={
                newsError
                  ? "Couldn't load news — check your connection."
                  : "No news loaded yet."
              }
              action={
                !newsLoading && newsItems.length > 0 ? (
                  <span className="text-[10px] text-gray-600">
                    click title to read · open to visit
                  </span>
                ) : undefined
              }
            >
              {newsItems.length > 0 ? (
                <div>
                  {newsItems.map((item, i) => (
                    <NewsCard key={item.id} item={item} rank={i + 1} />
                  ))}
                </div>
              ) : null}
            </SectionCard>
          </div>

          {/* Right column */}
          <div className="col-span-2 flex flex-col gap-4">
            {/* Inbox */}
            <SectionCard
              icon={Mail}
              title="Inbox"
              badge={emails.length}
              loading={emailsLoading}
              empty={
                emailsSkipped
                  ? "Connect Google to see emails."
                  : "No recent messages."
              }
              action={
                emailsSkipped ? (
                  <button
                    onClick={() => navigate("/integrations")}
                    className="text-[11px] text-donna-accent-light hover:underline"
                  >
                    Connect →
                  </button>
                ) : undefined
              }
            >
              {emails.length > 0 ? (
                <ul className="space-y-2">
                  {emails.map((m) => (
                    <li
                      key={m.id}
                      className="border-b border-white/5 pb-2 last:border-0 last:pb-0"
                    >
                      <p className="truncate text-xs font-medium text-gray-200">
                        {m.subject || "(No subject)"}
                      </p>
                      <p className="truncate text-[11px] text-gray-500">{m.from}</p>
                      <p className="mt-0.5 line-clamp-2 text-[11px] text-gray-600">
                        {m.snippet}
                      </p>
                    </li>
                  ))}
                </ul>
              ) : null}
            </SectionCard>

            {/* Today's Schedule */}
            <SectionCard
              icon={CalendarDays}
              title="Today's Schedule"
              badge={events.length}
              loading={eventsLoading}
              empty={
                eventsSkipped
                  ? "Connect Google to see your calendar."
                  : "Nothing on the calendar today."
              }
              action={
                eventsSkipped ? (
                  <button
                    onClick={() => navigate("/integrations")}
                    className="text-[11px] text-donna-accent-light hover:underline"
                  >
                    Connect →
                  </button>
                ) : undefined
              }
            >
              {events.length > 0 ? (
                <ul className="space-y-2">
                  {events.map((e, i) => (
                    <li key={e.id ?? i} className="flex items-start gap-2.5">
                      <div className="mt-0.5 flex-shrink-0 rounded bg-donna-accent/15 px-1.5 py-0.5 text-[10px] font-mono text-donna-accent-light">
                        {formatTime(e.start)}
                      </div>
                      <p className="text-xs text-gray-200 leading-relaxed">
                        {e.summary || "Untitled event"}
                      </p>
                    </li>
                  ))}
                </ul>
              ) : null}
            </SectionCard>

            {/* Pending notifications */}
            {(notifsLoading || notifs.length > 0) && (
              <SectionCard
                icon={Bell}
                title="Pending"
                badge={notifs.length}
                loading={notifsLoading}
                empty="Nothing pending."
              >
                {notifs.length > 0 ? (
                  <ul className="space-y-2">
                    {notifs.map((n) => (
                      <li
                        key={n.id}
                        className="border-b border-white/5 pb-2 last:border-0 last:pb-0"
                      >
                        <p className="text-xs font-medium text-gray-200">{n.title}</p>
                        <p className="mt-0.5 text-[11px] text-gray-500 line-clamp-2">
                          {n.body}
                        </p>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </SectionCard>
            )}

            {/* Cmd+D hint */}
            <div className="rounded-xl border border-white/[0.06] bg-donna-surface/50 px-4 py-3">
              <div className="flex items-center gap-2">
                <MessageSquare size={13} className="text-donna-accent-light flex-shrink-0" />
                <div>
                  <p className="text-xs font-medium text-gray-300">Quick Ask</p>
                  <p className="mt-0.5 text-[11px] text-gray-600 leading-relaxed">
                    Press{" "}
                    <kbd className="rounded border border-white/10 bg-white/5 px-1 py-0.5 font-mono text-[10px] text-gray-400">
                      ⌘D
                    </kbd>{" "}
                    anywhere to ask Donna about what's on your screen.
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
