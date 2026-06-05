import { PageShell, Placeholder } from "../components/PageShell";

const integrations = [
  "Gmail",
  "Google Calendar",
  "Google Docs / Drive",
  "Slack",
  "WhatsApp",
  "Fathom",
];

export default function Integrations() {
  return (
    <PageShell
      title="Integrations"
      subtitle="Connect your tools so Donna can work across them."
    >
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
        {integrations.map((name) => (
          <div
            key={name}
            className="flex items-center justify-between rounded-xl border border-white/10 bg-white/5 p-4"
          >
            <span className="text-sm text-white">{name}</span>
            <span className="rounded-full bg-white/10 px-2 py-0.5 text-xs text-gray-400">
              Soon
            </span>
          </div>
        ))}
      </div>
      <p className="mt-6 text-xs text-gray-500">
        One-click OAuth connections arrive in Phase 2. Tokens are stored securely in your
        OS keychain.
      </p>
    </PageShell>
  );
}
