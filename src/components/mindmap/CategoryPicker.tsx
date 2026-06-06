import { useEffect, useRef, useState } from "react";
import { ChevronDown } from "lucide-react";

interface CategoryPickerProps {
  categories: string[];
  active: string;
  colorFor: (category: string) => string;
  onChange: (category: string) => void;
}

function CategoryDot({ color }: { color: string }) {
  return (
    <span
      className="h-2.5 w-2.5 shrink-0 rounded-full"
      style={{
        backgroundColor: color,
        boxShadow: `0 0 6px ${color}cc, 0 0 12px ${color}66`,
      }}
    />
  );
}

export function CategoryPicker({
  categories,
  active,
  colorFor,
  onChange,
}: CategoryPickerProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as globalThis.Node)) {
        setOpen(false);
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  if (categories.length === 0) return null;

  const pick = (category: string) => {
    onChange(category);
    setOpen(false);
  };

  return (
    <div ref={rootRef} className="absolute bottom-4 right-4 z-10">
      <div className="rounded-xl border border-white/10 bg-donna-surface/95 shadow-lg backdrop-blur">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="flex w-full min-w-[9.5rem] items-center gap-2 px-3 py-2.5 text-left text-xs text-gray-200 hover:bg-white/5"
          aria-expanded={open}
          aria-haspopup="listbox"
        >
          <CategoryDot color={colorFor(active)} />
          <span className="flex-1 truncate font-medium">{active}</span>
          <ChevronDown
            size={14}
            className={`shrink-0 text-gray-500 transition-transform ${open ? "rotate-180" : ""}`}
          />
        </button>

        {open && (
          <ul
            role="listbox"
            aria-label="Categories"
            className="max-h-52 overflow-y-auto border-t border-white/10 py-1"
          >
            {categories.map((g) => {
              const selected = g === active;
              return (
                <li key={g}>
                  <button
                    type="button"
                    role="option"
                    aria-selected={selected}
                    onClick={() => pick(g)}
                    className={`flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors ${
                      selected
                        ? "bg-donna-accent/15 text-donna-accent-light"
                        : "text-gray-300 hover:bg-white/5"
                    }`}
                  >
                    <CategoryDot color={colorFor(g)} />
                    <span className="truncate">{g}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
