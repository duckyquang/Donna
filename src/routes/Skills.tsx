import { useEffect, useState } from "react";
import { BookOpen, RefreshCw, X } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { Markdown } from "../components/Markdown";
import { Spinner } from "../components/ui";
import { api, type Skill } from "../lib/api";

export default function Skills() {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [selected, setSelected] = useState<Skill | null>(null);
  const [body, setBody] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [loadingBody, setLoadingBody] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setError(null);
    try {
      setSkills(await api.skillsList());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const openSkill = async (skill: Skill) => {
    setSelected(skill);
    setLoadingBody(true);
    setError(null);
    try {
      setBody(await api.skillView(skill.name));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingBody(false);
    }
  };

  return (
    <PageShell
      title="Skills"
      subtitle="Donna can create skills herself — accept a suggestion, or ask her in chat."
    >
      {loading ? (
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <Spinner /> Loading…
        </div>
      ) : (
        <div className="space-y-4">
          {error && (
            <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
              {error}
            </p>
          )}

          <div className="flex items-center justify-between">
            <p className="text-xs text-gray-500">
              {skills.length} skill{skills.length === 1 ? "" : "s"}
            </p>
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

          {skills.length === 0 ? (
            <p className="rounded-xl border border-dashed border-white/15 bg-white/5 p-8 text-center text-sm text-gray-400">
              No skills yet. Donna will propose skills for recipes she repeats often, or
              you can ask her to write one in chat.
            </p>
          ) : (
            <div className="grid gap-4 lg:grid-cols-2">
              <div className="space-y-2">
                {skills.map((skill) => (
                  <button
                    key={skill.slug}
                    onClick={() => openSkill(skill)}
                    className={`flex w-full items-start gap-3 rounded-xl border p-3 text-left transition-colors ${
                      selected?.slug === skill.slug
                        ? "border-donna-accent/40 bg-donna-accent/10"
                        : "border-white/10 bg-donna-surface hover:bg-white/5"
                    }`}
                  >
                    <BookOpen size={18} className="mt-0.5 shrink-0 text-donna-accent-light" />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-sm font-medium text-white">
                          {skill.name}
                        </span>
                        {skill.category && (
                          <span className="shrink-0 rounded-full border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] uppercase tracking-wide text-gray-400">
                            {skill.category}
                          </span>
                        )}
                      </div>
                      {skill.description && (
                        <div className="mt-0.5 line-clamp-2 text-xs text-gray-400">
                          {skill.description}
                        </div>
                      )}
                    </div>
                  </button>
                ))}
              </div>

              <div className="rounded-xl border border-white/10 bg-donna-surface p-4 lg:min-h-[320px]">
                {loadingBody ? (
                  <div className="flex h-full items-center justify-center gap-2 text-sm text-gray-400">
                    <Spinner /> Loading…
                  </div>
                ) : selected ? (
                  <div>
                    <div className="mb-4 flex items-start justify-between gap-2">
                      <div>
                        <h2 className="text-lg font-medium text-white">{selected.name}</h2>
                        {selected.category && (
                          <p className="mt-1 text-xs text-gray-400">{selected.category}</p>
                        )}
                      </div>
                      <button
                        onClick={() => setSelected(null)}
                        className="rounded p-1 text-gray-500 hover:text-gray-300"
                        title="Close"
                      >
                        <X size={16} />
                      </button>
                    </div>
                    <div className="max-h-[60vh] overflow-y-auto text-gray-200">
                      <Markdown content={body} />
                    </div>
                  </div>
                ) : (
                  <p className="flex h-full min-h-[200px] items-center justify-center text-center text-sm text-gray-500">
                    Select a skill to view its SKILL.md.
                  </p>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </PageShell>
  );
}
