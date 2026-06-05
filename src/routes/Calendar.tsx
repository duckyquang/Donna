import { useEffect, useMemo, useState } from "react";
import { CalendarPlus, RefreshCw, Trash2, X } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { Button, Spinner } from "../components/ui";
import { api, type CalendarEvent } from "../lib/api";

const DAY_MS = 86_400_000;

function toLocalInput(iso: string): string {
  // Convert an ISO/RFC3339 string to a value usable in <input type="datetime-local">.
  const d = new Date(iso);
  if (isNaN(d.getTime())) return "";
  const off = d.getTimezoneOffset() * 60_000;
  return new Date(d.getTime() - off).toISOString().slice(0, 16);
}

function fromLocalInput(value: string): string {
  return new Date(value).toISOString();
}

interface Draft {
  id?: string;
  summary: string;
  description: string;
  start: string; // datetime-local value
  end: string;
}

export default function Calendar() {
  const [connected, setConnected] = useState<boolean | null>(null);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [saving, setSaving] = useState(false);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const statuses = await api.integrationsStatus();
      const isConnected = !!statuses.find((s) => s.id === "google")?.connected;
      setConnected(isConnected);
      if (isConnected) {
        const now = new Date();
        const min = new Date(now.getTime() - DAY_MS).toISOString();
        const max = new Date(now.getTime() + 30 * DAY_MS).toISOString();
        setEvents(await api.calendarListEvents(min, max));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const grouped = useMemo(() => {
    const map = new Map<string, CalendarEvent[]>();
    for (const ev of events) {
      const day = new Date(ev.start).toLocaleDateString(undefined, {
        weekday: "long",
        month: "short",
        day: "numeric",
      });
      if (!map.has(day)) map.set(day, []);
      map.get(day)!.push(ev);
    }
    return [...map.entries()];
  }, [events]);

  const newEvent = () => {
    const start = new Date(Math.ceil(Date.now() / DAY_MS) * DAY_MS + 9 * 3_600_000);
    const end = new Date(start.getTime() + 3_600_000);
    setDraft({
      summary: "",
      description: "",
      start: toLocalInput(start.toISOString()),
      end: toLocalInput(end.toISOString()),
    });
  };

  const editEvent = (ev: CalendarEvent) => {
    setDraft({
      id: ev.id,
      summary: ev.summary ?? "",
      description: ev.description ?? "",
      start: toLocalInput(ev.start),
      end: toLocalInput(ev.end),
    });
  };

  const saveDraft = async () => {
    if (!draft) return;
    setSaving(true);
    setError(null);
    try {
      const payload: CalendarEvent = {
        summary: draft.summary,
        description: draft.description || undefined,
        start: fromLocalInput(draft.start),
        end: fromLocalInput(draft.end),
      };
      if (draft.id) {
        await api.calendarUpdateEvent(draft.id, payload);
      } else {
        await api.calendarCreateEvent(payload);
      }
      setDraft(null);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const removeEvent = async (id?: string) => {
    if (!id) return;
    setError(null);
    try {
      await api.calendarDeleteEvent(id);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <PageShell
      title="Calendar"
      subtitle="Your schedule, synced two-way with Google Calendar."
    >
      {loading ? (
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <Spinner /> Loading your calendar…
        </div>
      ) : connected === false ? (
        <div className="rounded-xl border border-dashed border-white/15 bg-white/5 p-8 text-center text-sm text-gray-400">
          Connect Google in the <span className="text-donna-accent-light">Integrations</span>{" "}
          tab to sync your calendar.
        </div>
      ) : (
        <div className="space-y-4">
          <div className="flex items-center gap-2">
            <Button onClick={newEvent}>
              <CalendarPlus size={16} /> New event
            </Button>
            <Button variant="ghost" onClick={load}>
              <RefreshCw size={16} /> Refresh
            </Button>
          </div>

          {error && (
            <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
              {error}
            </p>
          )}

          {draft && (
            <div className="rounded-xl border border-white/10 bg-donna-surface p-4">
              <div className="mb-3 flex items-center justify-between">
                <h3 className="text-sm font-medium text-white">
                  {draft.id ? "Edit event" : "New event"}
                </h3>
                <button onClick={() => setDraft(null)} className="text-gray-500 hover:text-white">
                  <X size={16} />
                </button>
              </div>
              <div className="space-y-2">
                <input
                  value={draft.summary}
                  onChange={(e) => setDraft({ ...draft, summary: e.target.value })}
                  placeholder="Title"
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
                <div className="flex gap-2">
                  <label className="flex-1 text-xs text-gray-400">
                    Start
                    <input
                      type="datetime-local"
                      value={draft.start}
                      onChange={(e) => setDraft({ ...draft, start: e.target.value })}
                      className="mt-1 w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                    />
                  </label>
                  <label className="flex-1 text-xs text-gray-400">
                    End
                    <input
                      type="datetime-local"
                      value={draft.end}
                      onChange={(e) => setDraft({ ...draft, end: e.target.value })}
                      className="mt-1 w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                    />
                  </label>
                </div>
                <textarea
                  value={draft.description}
                  onChange={(e) => setDraft({ ...draft, description: e.target.value })}
                  placeholder="Description (optional)"
                  rows={2}
                  className="w-full resize-none rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
                <Button onClick={saveDraft} disabled={saving || !draft.summary.trim()}>
                  {saving ? <Spinner /> : null} {draft.id ? "Save changes" : "Create event"}
                </Button>
              </div>
            </div>
          )}

          {events.length === 0 ? (
            <p className="text-sm text-gray-500">No events in the next 30 days.</p>
          ) : (
            <div className="space-y-5">
              {grouped.map(([day, dayEvents]) => (
                <div key={day}>
                  <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-500">
                    {day}
                  </h3>
                  <div className="space-y-2">
                    {dayEvents.map((ev) => (
                      <div
                        key={ev.id}
                        className="group flex items-center justify-between rounded-lg border border-white/10 bg-donna-surface px-4 py-2.5"
                      >
                        <button
                          onClick={() => editEvent(ev)}
                          className="flex-1 text-left"
                        >
                          <div className="text-sm text-white">
                            {ev.summary || "(no title)"}
                          </div>
                          <div className="text-xs text-gray-500">
                            {new Date(ev.start).toLocaleTimeString([], {
                              hour: "2-digit",
                              minute: "2-digit",
                            })}{" "}
                            –{" "}
                            {new Date(ev.end).toLocaleTimeString([], {
                              hour: "2-digit",
                              minute: "2-digit",
                            })}
                          </div>
                        </button>
                        <button
                          onClick={() => removeEvent(ev.id)}
                          className="opacity-0 transition-opacity group-hover:opacity-100"
                          title="Delete"
                        >
                          <Trash2 size={15} className="text-gray-500 hover:text-red-400" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </PageShell>
  );
}
