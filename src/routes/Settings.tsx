import { PageShell, Placeholder } from "../components/PageShell";
import { PROVIDERS } from "../lib/models/providers";

export default function Settings() {
  return (
    <PageShell
      title="Settings"
      subtitle="Choose how Donna thinks — a free local model or your own API key."
    >
      <div className="space-y-3">
        {PROVIDERS.map((p) => (
          <div
            key={p.id}
            className="flex items-center justify-between rounded-xl border border-white/10 bg-white/5 p-4"
          >
            <div>
              <div className="text-sm font-medium text-white">{p.label}</div>
              <div className="text-xs text-gray-400">{p.description}</div>
            </div>
            <span className="rounded-full bg-white/10 px-2 py-0.5 text-xs text-gray-400">
              {p.kind === "local" ? "Free · Local" : "API key"}
            </span>
          </div>
        ))}
      </div>
      <div className="mt-6">
        <Placeholder note="Provider/model selection and secure key storage are wired up in Phase 1." />
      </div>
    </PageShell>
  );
}
