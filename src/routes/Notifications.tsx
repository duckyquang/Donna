import { useEffect, useState } from "react";
import { Bell, Plus, RefreshCw, Trash2 } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { Button, Spinner } from "../components/ui";
import { api, type Notification, type Routine } from "../lib/api";

const BUILTIN_LABELS: Record<string, string> = {
  morning_briefing: "Morning Briefing",
  meeting_briefing: "Meeting Briefing",
  relationship_reconnect: "Relationship Reconnect",
};

function routineLabel(r: Routine): string {
  return BUILTIN_LABELS[r.id] ?? r.name;
}

export default function Notifications() {
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const [name, setName] = useState("");
  const [dailyTime, setDailyTime] = useState("09:00");
  const [prompt, setPrompt] = useState("");
  const [creating, setCreating] = useState(false);

  const load = async () => {
    setError(null);
    try {
      const [r, n] = await Promise.all([api.listRoutines(), api.listNotifications()]);
      setRoutines(r);
      setNotifications(n);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
    // Refresh when the server broadcasts a notification (e.g. a routine fired).
    const onNotify = () => load();
    window.addEventListener("donna:notification", onNotify);
    return () => window.removeEventListener("donna:notification", onNotify);
  }, []);

  const toggle = async (routine: Routine) => {
    setBusy(routine.id);
    try {
      await api.toggleRoutine(routine.id, !routine.enabled);
      setRoutines((prev) =>
        prev.map((r) => (r.id === routine.id ? { ...r, enabled: !r.enabled } : r))
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const remove = async (routine: Routine) => {
    if (routine.builtin) return;
    setBusy(routine.id);
    try {
      await api.deleteRoutine(routine.id);
      setRoutines((prev) => prev.filter((r) => r.id !== routine.id));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const create = async () => {
    if (!name.trim() || !prompt.trim()) return;
    setCreating(true);
    setError(null);
    try {
      const created = await api.createRoutine({
        name: name.trim(),
        dailyTime,
        prompt: prompt.trim(),
      });
      setRoutines((prev) => [...prev, created]);
      setName("");
      setPrompt("");
      setDailyTime("09:00");
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  };

  const markRead = async (n: Notification) => {
    if (n.read) return;
    try {
      await api.markNotificationRead(n.id);
      setNotifications((prev) =>
        prev.map((item) => (item.id === n.id ? { ...item, read: true } : item))
      );
    } catch (e) {
      setError(String(e));
    }
  };

  const builtins = routines.filter((r) => r.builtin);
  const custom = routines.filter((r) => !r.builtin);

  return (
    <PageShell
      title="Notifications"
      subtitle="Proactive reminders and nudges, driven by routines you control."
    >
      {loading ? (
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <Spinner /> Loading…
        </div>
      ) : (
        <div className="space-y-8">
          {error && (
            <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
              {error}
            </p>
          )}

          <section>
            <div className="mb-3 flex items-center justify-between">
              <h2 className="text-sm font-medium text-gray-300">Routines</h2>
              <button
                onClick={() => {
                  setLoading(true);
                  load();
                }}
                className="flex items-center gap-1 text-xs text-gray-500 hover:text-gray-300"
              >
                <RefreshCw size={12} /> Refresh
              </button>
            </div>

            {routines.length === 0 ? (
              <p className="rounded-xl border border-dashed border-white/15 bg-white/5 p-6 text-center text-sm text-gray-400">
                No routines yet. Built-in routines appear here once the scheduler is running.
              </p>
            ) : (
              <div className="space-y-2">
                {builtins.length > 0 && (
                  <p className="text-xs text-gray-500">Built-in</p>
                )}
                {builtins.map((r) => (
                  <RoutineRow
                    key={r.id}
                    routine={r}
                    label={routineLabel(r)}
                    busy={busy === r.id}
                    onToggle={() => toggle(r)}
                  />
                ))}

                {custom.length > 0 && (
                  <p className="pt-2 text-xs text-gray-500">Custom</p>
                )}
                {custom.map((r) => (
                  <RoutineRow
                    key={r.id}
                    routine={r}
                    label={routineLabel(r)}
                    busy={busy === r.id}
                    onToggle={() => toggle(r)}
                    onDelete={() => remove(r)}
                  />
                ))}
              </div>
            )}
          </section>

          <section className="rounded-xl border border-white/10 bg-donna-surface p-4">
            <h2 className="mb-3 text-sm font-medium text-gray-300">Add custom routine</h2>
            <div className="space-y-3">
              <label className="block">
                <span className="mb-1 block text-xs text-gray-400">Name</span>
                <input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Weekly task summary"
                  className="w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
              </label>
              <label className="block">
                <span className="mb-1 block text-xs text-gray-400">Daily time</span>
                <input
                  type="time"
                  value={dailyTime}
                  onChange={(e) => setDailyTime(e.target.value)}
                  className="rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
              </label>
              <label className="block">
                <span className="mb-1 block text-xs text-gray-400">Prompt</span>
                <textarea
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  rows={3}
                  placeholder="Every Friday at this time, summarize my open tasks and suggest priorities."
                  className="w-full resize-none rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
              </label>
              <Button
                onClick={create}
                disabled={creating || !name.trim() || !prompt.trim()}
              >
                {creating ? <Spinner /> : <Plus size={16} />}
                Add routine
              </Button>
            </div>
          </section>

          <section>
            <h2 className="mb-3 text-sm font-medium text-gray-300">Recent notifications</h2>
            {notifications.length === 0 ? (
              <p className="rounded-xl border border-dashed border-white/15 bg-white/5 p-6 text-center text-sm text-gray-400">
                No notifications yet. Donna will nudge you here when routines run.
              </p>
            ) : (
              <div className="space-y-2">
                {notifications.map((n) => (
                  <div
                    key={n.id}
                    className={`rounded-xl border p-4 transition-colors ${
                      n.read
                        ? "border-white/5 bg-white/[0.02]"
                        : "border-donna-accent/30 bg-donna-accent/5"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex gap-3">
                        <Bell
                          size={16}
                          className={`mt-0.5 shrink-0 ${
                            n.read ? "text-gray-500" : "text-donna-accent-light"
                          }`}
                        />
                        <div>
                          <div
                            className={`text-sm font-medium ${
                              n.read ? "text-gray-400" : "text-white"
                            }`}
                          >
                            {n.title}
                          </div>
                          <p className="mt-1 text-xs text-gray-400">{n.body}</p>
                          <p className="mt-2 text-[11px] text-gray-500">
                            {new Date(n.createdAt).toLocaleString()}
                          </p>
                        </div>
                      </div>
                      {!n.read && (
                        <Button variant="ghost" className="shrink-0 px-3 py-1 text-xs" onClick={() => markRead(n)}>
                          Mark read
                        </Button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>
        </div>
      )}
    </PageShell>
  );
}

function RoutineRow({
  routine,
  label,
  busy,
  onToggle,
  onDelete,
}: {
  routine: Routine;
  label: string;
  busy: boolean;
  onToggle: () => void;
  onDelete?: () => void;
}) {
  return (
    <div className="flex items-center justify-between rounded-xl border border-white/10 bg-donna-surface px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-white">{label}</div>
        <div className="mt-0.5 truncate text-xs text-gray-400">
          {routine.dailyTime ? `Daily at ${routine.dailyTime}` : "On schedule"}
          {routine.prompt ? ` · ${routine.prompt}` : ""}
        </div>
      </div>
      <div className="ml-3 flex shrink-0 items-center gap-2">
        {onDelete && (
          <button
            onClick={onDelete}
            disabled={busy}
            title="Delete routine"
            className="rounded p-1 text-gray-500 hover:text-red-400 disabled:opacity-50"
          >
            <Trash2 size={14} />
          </button>
        )}
        <button
          onClick={onToggle}
          disabled={busy}
          role="switch"
          aria-checked={routine.enabled}
          className={`relative h-6 w-11 rounded-full transition-colors disabled:opacity-50 ${
            routine.enabled ? "bg-donna-accent" : "bg-white/15"
          }`}
        >
          {busy ? (
            <span className="absolute inset-0 flex items-center justify-center">
              <Spinner className="h-3 w-3" />
            </span>
          ) : (
            <span
              className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
                routine.enabled ? "left-5" : "left-0.5"
              }`}
            />
          )}
        </button>
      </div>
    </div>
  );
}
