import { Routes, Route, Navigate, useLocation } from "react-router-dom";
import DesktopRequired from "./components/DesktopRequired";
import { Sidebar } from "./components/Sidebar";
import { useConfig } from "./lib/useConfig";
import { isDesktopApp } from "./lib/tauri";
import Chat from "./routes/Chat";
import Dashboard from "./routes/Dashboard";
import QuickChat from "./routes/QuickChat";
import Projects from "./routes/Projects";
import Productivity from "./routes/Productivity";
import Notifications from "./routes/Notifications";
import Docs from "./routes/Docs";
import Calendar from "./routes/Calendar";
import MindMap from "./routes/MindMap";
import Integrations from "./routes/Integrations";
import Settings from "./routes/Settings";
import Onboarding from "./routes/Onboarding";

export default function App() {
  const { config, loading } = useConfig();
  const location = useLocation();

  if (!isDesktopApp()) {
    return <DesktopRequired />;
  }

  // Quick-chat window — standalone overlay, no Sidebar or onboarding checks
  if (window.location.pathname === "/quick-chat") {
    return (
      <Routes>
        <Route path="/quick-chat" element={<QuickChat />} />
      </Routes>
    );
  }

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
          <Route path="/" element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<Dashboard />} />
          <Route path="/chat" element={<Chat />} />
          <Route path="/projects" element={<Projects />} />
          <Route path="/productivity" element={<Productivity />} />
          <Route path="/notifications" element={<Notifications />} />
          <Route path="/docs" element={<Docs />} />
          <Route path="/calendar" element={<Calendar />} />
          <Route path="/mind-map" element={<MindMap />} />
          <Route path="/integrations" element={<Integrations />} />
          <Route path="/settings" element={<Settings />} />
        </Routes>
      </main>
    </div>
  );
}
