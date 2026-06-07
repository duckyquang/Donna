import { useEffect, useState } from "react";
import { FileText, RefreshCw, Trash2, X } from "lucide-react";
import { PageShell } from "../components/PageShell";
import { Markdown } from "../components/Markdown";
import { Spinner } from "../components/ui";import { api, type Doc, type DocDetail } from "../lib/api";

export default function Docs() {
  const [docs, setDocs] = useState<Doc[]>([]);
  const [selected, setSelected] = useState<DocDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingDoc, setLoadingDoc] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = async () => {
    setError(null);
    try {
      setDocs(await api.listDocs());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const openDoc = async (doc: Doc) => {
    setLoadingDoc(true);
    setError(null);
    try {
      setSelected(await api.getDoc(doc.id));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingDoc(false);
    }
  };

  const remove = async (doc: Doc) => {
    setBusy(doc.id);
    setError(null);
    try {
      await api.deleteDoc(doc.id);
      setDocs((prev) => prev.filter((d) => d.id !== doc.id));
      if (selected?.id === doc.id) setSelected(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <PageShell
      title="Docs"
      subtitle="Documents Donna creates from your meetings, messages, and requests."
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
              {docs.length} document{docs.length === 1 ? "" : "s"}
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

          {docs.length === 0 ? (
            <p className="rounded-xl border border-dashed border-white/15 bg-white/5 p-8 text-center text-sm text-gray-400">
              No docs yet. Donna will create meeting recaps, message summaries, and more as
              she works for you.
            </p>
          ) : (
            <div className="grid gap-4 lg:grid-cols-2">
              <div className="space-y-2">
                {docs.map((doc) => (
                  <div
                    key={doc.id}
                    className={`flex items-center justify-between rounded-xl border p-3 transition-colors ${
                      selected?.id === doc.id
                        ? "border-donna-accent/40 bg-donna-accent/10"
                        : "border-white/10 bg-donna-surface hover:bg-white/5"
                    }`}
                  >
                    <button
                      onClick={() => openDoc(doc)}
                      className="flex min-w-0 flex-1 items-start gap-3 text-left"
                    >
                      <FileText
                        size={18}
                        className="mt-0.5 shrink-0 text-donna-accent-light"
                      />
                      <div className="min-w-0">
                        <div className="truncate text-sm font-medium text-white">
                          {doc.title}
                        </div>
                        <div className="mt-0.5 text-xs text-gray-400">
                          {new Date(doc.createdAt).toLocaleString()}
                          {doc.source ? ` · ${doc.source}` : ""}
                        </div>
                      </div>
                    </button>
                    <button
                      onClick={() => remove(doc)}
                      disabled={busy === doc.id}
                      title="Delete doc"
                      className="ml-2 shrink-0 rounded p-1.5 text-gray-500 hover:text-red-400 disabled:opacity-50"
                    >
                      {busy === doc.id ? <Spinner className="h-3.5 w-3.5" /> : <Trash2 size={14} />}
                    </button>
                  </div>
                ))}
              </div>

              <div className="rounded-xl border border-white/10 bg-donna-surface p-4 lg:min-h-[320px]">
                {loadingDoc ? (
                  <div className="flex h-full items-center justify-center gap-2 text-sm text-gray-400">
                    <Spinner /> Loading…
                  </div>
                ) : selected ? (
                  <div>
                    <div className="mb-4 flex items-start justify-between gap-2">
                      <div>
                        <h2 className="text-lg font-medium text-white">{selected.title}</h2>
                        <p className="mt-1 text-xs text-gray-400">
                          {new Date(selected.createdAt).toLocaleString()}
                          {selected.source ? ` · ${selected.source}` : ""}
                        </p>
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
                      <Markdown content={selected.content} />
                    </div>
                  </div>
                ) : (
                  <p className="flex h-full min-h-[200px] items-center justify-center text-center text-sm text-gray-500">
                    Select a doc to view its content.
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
