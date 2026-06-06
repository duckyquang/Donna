import { useEffect, useState } from "react";
import { Pencil, Trash2, X } from "lucide-react";
import { api, type KgNode } from "../../lib/api";
import { Button, Spinner } from "../ui";

interface NodeDetailPanelProps {
  node: KgNode;
  childNodes?: KgNode[];
  onClose: () => void;
  onEdit: () => void;
  onSelectChild?: (id: string) => void;
  onDeleted: () => void;
}

export function NodeDetailPanel({
  node,
  childNodes = [],
  onClose,
  onEdit,
  onSelectChild,
  onDeleted,
}: NodeDetailPanelProps) {
  const isFolder = node.type === "folder";
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (!isFolder && node.hasImage) {
      api.kgNodeImage(node.folder, node.fileId).then(setImageUrl).catch(() => {});
    } else {
      setImageUrl(null);
    }
  }, [node, isFolder]);

  const remove = async () => {
    setDeleting(true);
    try {
      await api.kgDeleteNode(node.folder, node.fileId);
      onDeleted();
    } catch {
      setDeleting(false);
    }
  };

  return (
    <aside className="mindmap-detail-panel flex w-80 shrink-0 flex-col border-l border-white/10 bg-donna-surface/95 backdrop-blur">
      <div className="flex items-start justify-between gap-2 border-b border-white/10 px-4 py-3">
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-sm font-semibold text-white">{node.label}</h2>
          <p className="mt-0.5 text-xs text-gray-500">
            {node.group} · {node.type}
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="shrink-0 rounded-lg p-1 text-gray-500 hover:bg-white/5 hover:text-white"
          aria-label="Close details"
        >
          <X size={16} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {isFolder ? (
          <>
            <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-500">
              Branch
            </p>
            <p className="text-sm text-gray-300">{node.folder.join(" / ")}</p>
            <p className="mt-3 text-xs leading-relaxed text-gray-500">
              Folder branches group related facts. Click a child below or on the map to
              read Donna&apos;s notes.
            </p>
            {childNodes.length > 0 && (
              <div className="mt-4 space-y-1.5">
                <p className="text-xs font-medium uppercase tracking-wide text-gray-500">
                  Connected ({childNodes.length})
                </p>
                {childNodes.map((child) => (
                  <button
                    key={child.id}
                    type="button"
                    onClick={() => onSelectChild?.(child.id)}
                    className="flex w-full items-center gap-2 rounded-lg border border-white/10 px-3 py-2 text-left text-sm text-gray-200 hover:border-donna-accent/40 hover:bg-white/5"
                  >
                    <span
                      className={`h-2 w-2 shrink-0 rounded-full ${
                        child.type === "folder" ? "rounded-sm" : "rounded-full"
                      }`}
                      style={{
                        background:
                          child.type === "folder" ? "transparent" : "var(--donna-accent)",
                        border: child.type === "folder" ? "1.5px solid var(--donna-accent)" : undefined,
                      }}
                    />
                    <span className="truncate">{child.label}</span>
                  </button>
                ))}
              </div>
            )}
          </>
        ) : (
          <>
            {imageUrl && (
              <img
                src={imageUrl}
                alt={node.label}
                className="mb-4 max-h-48 w-full rounded-lg border border-white/10 object-contain"
              />
            )}

            <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-500">
              What Donna knows
            </p>
            <p className="whitespace-pre-wrap text-sm leading-relaxed text-gray-200">
              {node.note.trim() || "No description saved for this node yet."}
            </p>

            {node.updatedAt && (
              <p className="mt-4 text-[11px] text-gray-600">
                Updated {new Date(node.updatedAt).toLocaleString()}
              </p>
            )}
          </>
        )}
      </div>

      {!isFolder && (
        <div className="flex gap-2 border-t border-white/10 px-4 py-3">
          <Button variant="ghost" className="flex-1" onClick={onEdit}>
            <Pencil size={14} />
            Edit
          </Button>
          <Button variant="danger" onClick={remove} disabled={deleting}>
            {deleting ? <Spinner /> : <Trash2 size={14} />}
            Delete
          </Button>
        </div>
      )}
    </aside>
  );
}
