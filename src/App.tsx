import { Routes, Route, Navigate } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import Chat from "./routes/Chat";
import Notifications from "./routes/Notifications";
import Docs from "./routes/Docs";
import Calendar from "./routes/Calendar";
import Integrations from "./routes/Integrations";
import Settings from "./routes/Settings";

export default function App() {
  return (
    <div className="flex h-full w-full">
      <Sidebar />
      <main className="flex-1 overflow-y-auto">
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
