import { Routes, Route, Navigate, useLocation } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import { useConfig } from "./lib/useConfig";
import Chat from "./routes/Chat";
import Notifications from "./routes/Notifications";
import Docs from "./routes/Docs";
import Calendar from "./routes/Calendar";
import Integrations from "./routes/Integrations";
import Settings from "./routes/Settings";
import Onboarding from "./routes/Onboarding";

export default function App() {
  const { config, loading } = useConfig();
  const location = useLocation();

  if (loading) {
    return (
      <div className="flex h-full w-full items-center justify-center text-gray-400">
        Loading Donna…
      </div>
    );
  }

  // First run: send the user through onboarding before showing the app shell.
  const needsOnboarding = !config?.onboarded;
  if (needsOnboarding && location.pathname !== "/onboarding") {
    return <Navigate to="/onboarding" replace />;
  }

  if (location.pathname === "/onboarding") {
    return <Onboarding />;
  }

  return (
    <div className="flex h-full w-full">
      <Sidebar />
      <main className="flex-1 overflow-hidden">
        <Routes>
          <Route path="/" element={<Navigate to="/chat" replace />} />
          <Route path="/chat" element={<Chat />} />
          <Route path="/notifications" element={<Notifications />} />
          <Route path="/docs" element={<Docs />} />
          <Route path="/calendar" element={<Calendar />} />
          <Route path="/integrations" element={<Integrations />} />
          <Route path="/settings" element={<Settings />} />
        </Routes>
      </main>
    </div>
  );
}
