import type { ReactNode } from "react";

interface PageShellProps {
  title: string;
  subtitle?: string;
  children?: ReactNode;
}

export function PageShell({ title, subtitle, children }: PageShellProps) {
  return (
    <div className="h-full overflow-y-auto px-8 py-10">
      <div className="mx-auto max-w-4xl">
      <header className="mb-8">
        <h1 className="text-2xl font-semibold text-white">{title}</h1>
        {subtitle && <p className="mt-1 text-sm text-gray-400">{subtitle}</p>}
      </header>
      {children}
      </div>
    </div>
  );
}

export function Placeholder({ note }: { note: string }) {
  return (
    <div className="rounded-xl border border-dashed border-white/15 bg-white/5 p-8 text-center text-sm text-gray-400">
      {note}
    </div>
  );
}
