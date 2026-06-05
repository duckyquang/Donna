import { NavLink } from "react-router-dom";
import {
  MessageSquare,
  Bell,
  FileText,
  Calendar,
  Plug,
  Settings as SettingsIcon,
} from "lucide-react";

const links = [
  { to: "/chat", label: "Chat", icon: MessageSquare },
  { to: "/notifications", label: "Notifications", icon: Bell },
  { to: "/docs", label: "Docs", icon: FileText },
  { to: "/calendar", label: "Calendar", icon: Calendar },
  { to: "/integrations", label: "Integrations", icon: Plug },
  { to: "/settings", label: "Settings", icon: SettingsIcon },
];

export function Sidebar() {
  return (
    <aside className="flex w-60 flex-col border-r border-white/10 bg-donna-surface p-4">
      <div className="mb-8 flex items-center gap-2 px-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-donna-accent font-bold text-white">
          D
        </div>
        <span className="text-lg font-semibold">Donna</span>
      </div>

      <nav className="flex flex-col gap-1">
        {links.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors ${
                isActive
                  ? "bg-donna-accent/15 text-donna-accent-light"
                  : "text-gray-400 hover:bg-white/5 hover:text-white"
              }`
            }
          >
            <Icon size={18} />
            {label}
          </NavLink>
        ))}
      </nav>

      <div className="mt-auto px-2 text-xs text-gray-500">
        Local-first · Private · Open source
      </div>
    </aside>
  );
}
