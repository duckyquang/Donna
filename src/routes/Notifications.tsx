import { PageShell, Placeholder } from "../components/PageShell";

export default function Notifications() {
  return (
    <PageShell
      title="Notifications"
      subtitle="Proactive reminders and nudges, pushed to you automatically."
    >
      <Placeholder note="Proactive notifications arrive in Phase 3, driven by the background scheduler in the Rust core." />
    </PageShell>
  );
}
