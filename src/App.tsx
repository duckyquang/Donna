import { useEffect, useState } from "react";
import { Routes, Route, Navigate, useLocation } from "react-router-dom";
import { WifiOff } from "lucide-react";
import DesktopRequired from "./components/DesktopRequired";
import { Sidebar } from "./components/Sidebar";
import { useConfig } from "./lib/useConfig";
import { isDesktopApp } from "./lib/tauri";
import { serverReachable, onServerEvent } from "./lib/server";
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
  const [reachable, setReachable] = useState(true);

  // Poll server reachability on mount and every 30s so the banner reflects reality.
  useEffect(() => {
    let active = true;
    const check = () => serverReachable().then((r) => active && setReachable(r));
    check();
    const timer = setInterval(check, 30_000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);

  // Broadcast server notifications (routines firing) so the Routines page can refresh.
  useEffect(() => {
    return onServerEvent((f) => {
      if (f.type === "notification") {
        window.dispatchEvent(new CustomEvent("donna:notification"));
      }
    });
  }, []);

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
    <div className="flex h-full w-full flex-col">
      {!reachable && (
        <div className="flex items-center justify-center gap-3 border-b border-red-500/30 bg-red-500/10 px-4 py-2 text-xs text-red-300">
          <WifiOff size={14} />
          <span>Donna is unreachable — check the server.</span>
          <button
            onClick={() => serverReachable().then(setReachable)}
            className="rounded border border-red-400/40 px-2 py-0.5 font-medium text-red-200 hover:bg-red-500/20"
          >
            Retry
          </button>
        </div>
      )}
      <div className="flex min-h-0 flex-1">
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
    </div>
  );
}
