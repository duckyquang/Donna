import { useEffect, useState } from "react";
import { Pencil, Trash2, X } from "lucide-react";
import { api, type KgNode } from "../../lib/api";
import { Button, Spinner } from "../ui";

interface NodeDetailPanelProps {
  node: KgNode;
  onClose: () => void;
  onEdit: () => void;
  onDeleted: () => void;
}

export function NodeDetailPanel({ node, onClose, onEdit, onDeleted }: NodeDetailPanelProps) {
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (node.hasImage) {
      api.kgNodeImage(node.folder, node.fileId).then(setImageUrl).catch(() => {});
    } else {
      setImageUrl(null);
    }
  }, [node]);

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

        <p className="mt-4 text-[11px] text-gray-600">
          Updated {new Date(node.updatedAt).toLocaleString()}
        </p>
      </div>

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
    </aside>
  );
}
