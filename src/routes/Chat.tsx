import { PageShell, Placeholder } from "../components/PageShell";

export default function Chat() {
  return (
    <PageShell
      title="Chat"
      subtitle="Talk with Donna, brainstorm, and teach her about your routines and life."
    >
      <Placeholder note="Chat interface coming in Phase 1. This will stream responses from your selected model (local Ollama or a cloud provider) and let you teach Donna facts and routines." />
    </PageShell>
  );
}
