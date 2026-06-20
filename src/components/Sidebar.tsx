import { NavLink } from "react-router-dom";
import {
  MessageSquare,
  Bell,
  FileText,
  Calendar,
  Network,
  Plug,
  Settings as SettingsIcon,
  FolderOpen,
} from "lucide-react";

const links = [
  { to: "/chat", label: "Chat", icon: MessageSquare },
  { to: "/projects", label: "Projects", icon: FolderOpen },
  { to: "/notifications", label: "Routines", icon: Bell },
  { to: "/docs", label: "Docs", icon: FileText },
  { to: "/calendar", label: "Calendar", icon: Calendar },
  { to: "/mind-map", label: "Memory", icon: Network },
  { to: "/integrations", label: "Integrations", icon: Plug },
  { to: "/settings", label: "Settings", icon: SettingsIcon },
];

export function Sidebar() {
  return (
    <aside className="flex w-56 flex-col border-r border-donna-border bg-donna-panel">
      <div className="flex h-14 items-center gap-2.5 border-b border-donna-border px-4">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-donna-accent text-xs font-bold text-white">
          D
        </div>
        <span className="text-sm font-semibold tracking-tight text-donna-text">Donna</span>
      </div>

      <nav className="flex flex-col gap-0.5 p-2 pt-3">
        {links.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-2.5 rounded px-2.5 py-2 text-sm transition-colors ${
                isActive
                  ? "bg-donna-accent-dim text-donna-accent-light font-medium"
                  : "text-donna-muted-light hover:bg-donna-surface-hover hover:text-donna-text"
              }`
            }
          >
            <Icon size={15} />
            {label}
          </NavLink>
        ))}
      </nav>

      <div className="mt-auto border-t border-donna-border p-4">
        <p className="text-[10px] text-donna-muted tracking-wide uppercase">Local · Private</p>
      </div>
    </aside>
  );
}
