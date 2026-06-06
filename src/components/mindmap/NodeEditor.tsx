import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ImagePlus, Trash2, X } from "lucide-react";
import { api, type KgNode } from "../../lib/api";
import { Button, Spinner } from "../ui";

const TYPES = [
  "info",
  "routine",
  "feedback",
  "preference",
  "person",
  "project",
  "other",
];

interface NodeEditorProps {
  /** Existing node to edit, or null to create a new one. */
  node: KgNode | null;
  onClose: () => void;
  onSaved: () => void;
}

function folderToText(folder: string[]): string {
  return folder.join(" / ");
}

function parseFolder(text: string): string[] {
  const parts = text
    .split("/")
    .map((p) => p.trim())
    .filter(Boolean);
  return parts.length > 0 ? parts : ["About You"];
}

export function NodeEditor({ node, onClose, onSaved }: NodeEditorProps) {
  const [label, setLabel] = useState(node?.label ?? "");
  const [type, setType] = useState(node?.type ?? "info");
  const [category, setCategory] = useState(
    node ? folderToText(node.folder) : "About You"
  );
  const [note, setNote] = useState(node?.note ?? "");
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [pendingPath, setPendingPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (node?.hasImage) {
      api.kgNodeImage(node.folder, node.fileId).then(setImageUrl).catch(() => {});
    }
  }, [node]);

  const pickImage = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }],
      });
      if (typeof selected === "string") setPendingPath(selected);
    } catch (e) {
      setError(String(e));
    }
  };

  const removeImage = async () => {
    setPendingPath(null);
    if (node?.hasImage) {
      try {
        await api.kgRemoveNodeImage(node.folder, node.fileId);
        setImageUrl(null);
      } catch (e) {
        setError(String(e));
      }
    } else {
      setImageUrl(null);
    }
  };

  const save = async () => {
    if (!label.trim()) {
      setError("Give the node a label.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const saved = await api.kgSaveNode({
        folder: parseFolder(category),
        label: label.trim(),
        note,
        type,
        fromFolder: node?.folder,
        fromId: node?.fileId,
      });
      if (pendingPath) {
        await api.kgSetNodeImage(saved.folder, saved.fileId, pendingPath);
      }
      onSaved();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!node) return;
    setBusy(true);
    setError(null);
    try {
      await api.kgDeleteNode(node.folder, node.fileId);
      onSaved();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <>
      <button
        type="button"
        aria-label="Close"
        className="absolute inset-0 z-20 bg-black/55 backdrop-blur-[2px]"
        onClick={onClose}
      />
      <div
        role="dialog"
        aria-modal="true"
        className="absolute left-1/2 top-1/2 z-30 w-full max-w-md -translate-x-1/2 -translate-y-1/2 px-4"
      >
        <div className="max-h-[85vh] overflow-y-auto rounded-2xl border border-white/10 bg-donna-surface p-5 shadow-2xl">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-base font-semibold text-white">
              {node ? "Edit node" : "Add node"}
            </h2>
            <button
              onClick={onClose}
              className="rounded-lg p-1 text-gray-500 hover:bg-white/5 hover:text-white"
            >
              <X size={18} />
            </button>
          </div>

          <div className="space-y-3">
            <label className="block text-xs text-gray-400">
              Label
              <input
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="Short name"
                className="mt-1 w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
              />
            </label>

            <div className="flex gap-2">
              <label className="flex-1 text-xs text-gray-400">
                Category / branch
                <input
                  value={category}
                  onChange={(e) => setCategory(e.target.value)}
                  placeholder="e.g. Routines / Mornings"
                  className="mt-1 w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                />
              </label>
              <label className="text-xs text-gray-400">
                Type
                <select
                  value={type}
                  onChange={(e) => setType(e.target.value)}
                  className="mt-1 w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                >
                  {TYPES.map((t) => (
                    <option key={t} value={t}>
                      {t}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <label className="block text-xs text-gray-400">
              Note (Donna&apos;s description)
              <textarea
                value={note}
                onChange={(e) => setNote(e.target.value)}
                rows={4}
                placeholder="What to remember about this…"
                className="mt-1 w-full resize-none rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
              />
            </label>

            {/* Image */}
            <div className="rounded-lg border border-white/10 bg-donna-bg p-3">
              {imageUrl ? (
                <img
                  src={imageUrl}
                  alt={label}
                  className="mb-2 max-h-40 w-full rounded-md object-contain"
                />
              ) : null}
              {pendingPath && (
                <p className="mb-2 truncate text-[11px] text-donna-accent-light">
                  Selected: {pendingPath.split("/").pop()}
                </p>
              )}
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={pickImage}
                  className="flex items-center gap-1.5 rounded-lg border border-white/15 px-3 py-1.5 text-xs text-gray-200 hover:bg-white/5"
                >
                  <ImagePlus size={14} /> {imageUrl || pendingPath ? "Replace image" : "Attach image"}
                </button>
                {(imageUrl || pendingPath) && (
                  <button
                    type="button"
                    onClick={removeImage}
                    className="flex items-center gap-1.5 rounded-lg border border-white/15 px-3 py-1.5 text-xs text-gray-300 hover:bg-white/5"
                  >
                    <Trash2 size={14} /> Remove
                  </button>
                )}
              </div>
            </div>

            {error && (
              <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-300">
                {error}
              </p>
            )}

            <div className="flex items-center justify-between pt-1">
              {node ? (
                <Button variant="danger" onClick={remove} disabled={busy}>
                  <Trash2 size={16} /> Delete
                </Button>
              ) : (
                <span />
              )}
              <Button onClick={save} disabled={busy || !label.trim()}>
                {busy ? <Spinner /> : null} {node ? "Save changes" : "Create node"}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
