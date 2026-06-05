import { Monitor } from "lucide-react";

export default function DesktopRequired() {
  return (
    <div className="flex h-full w-full items-center justify-center bg-[#0b0b0f] p-6">
      <div className="w-full max-w-lg rounded-2xl border border-amber-500/30 bg-[#15151c] p-8">
        <div className="mb-4 flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500/15 text-amber-400">
            <Monitor size={20} />
          </div>
          <div>
            <h1 className="text-xl font-semibold text-white">Open Donna as a desktop app</h1>
            <p className="text-sm text-gray-400">
              Browser previews cannot talk to Donna&apos;s backend.
            </p>
          </div>
        </div>

        <p className="text-sm leading-relaxed text-gray-300">
          You&apos;re viewing the Vite dev server in a browser. Donna is a Tauri desktop app —
          settings, model detection, and chat all go through the native shell.
        </p>

        <div className="mt-4 rounded-lg border border-white/10 bg-[#0b0b0f] p-4">
          <p className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-500">
            Run this in your terminal
          </p>
          <code className="block text-sm text-[#7c5cff]">npm run tauri:dev</code>
        </div>

        <p className="mt-4 text-xs text-gray-500">
          Use the Donna window that opens — not{" "}
          <code className="text-gray-400">http://localhost:1420</code> in a browser tab.
        </p>
      </div>
    </div>
  );
}
