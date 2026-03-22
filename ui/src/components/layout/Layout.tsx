import { NavLink, Outlet } from "react-router-dom";
import {
  LayoutDashboard,
  Server,
  Route,
  ScrollText,
  Settings,
  Moon,
  Sun,
  Zap,
} from "lucide-react";
import { useAppStore } from "@/stores/store";
import { cn } from "@/lib/utils";

const navItems = [
  { to: "/", label: "Overview", icon: LayoutDashboard },
  { to: "/providers", label: "Providers", icon: Server },
  { to: "/routing", label: "Routing", icon: Route },
  { to: "/logs", label: "Logs", icon: ScrollText },
  { to: "/config", label: "Config", icon: Settings },
];

/**
 * Main application layout with top navigation bar.
 */
export default function Layout() {
  const darkMode = useAppStore((s) => s.darkMode);
  const toggleDarkMode = useAppStore((s) => s.toggleDarkMode);

  return (
    <div className="h-screen bg-background flex flex-col overflow-hidden">
      <header className="shrink-0 z-50 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="flex h-14 items-center px-6">
          <div className="flex items-center gap-2 mr-8">
            <Zap className="h-5 w-5 text-chart-1" />
            <span className="font-bold text-lg tracking-tight">ZZ</span>
          </div>

          <nav className="flex items-center gap-1">
            {navItems.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.to === "/"}
                className={({ isActive }) =>
                  cn(
                    "flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors",
                    isActive
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
                  )
                }
              >
                <item.icon className="h-4 w-4" />
                {item.label}
              </NavLink>
            ))}
          </nav>

          <div className="ml-auto flex items-center gap-2">
            <div className="flex items-center gap-2 text-xs text-muted-foreground mr-4">
              <span className="inline-block h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
              Proxy Running
            </div>
            <button
              onClick={toggleDarkMode}
              className="inline-flex items-center justify-center h-9 w-9 rounded-md hover:bg-accent transition-colors"
              aria-label="Toggle dark mode"
            >
              {darkMode ? (
                <Sun className="h-4 w-4" />
              ) : (
                <Moon className="h-4 w-4" />
              )}
            </button>
          </div>
        </div>
      </header>

      <main className="p-6 flex-1 flex flex-col overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}
