import { useState } from "react";
import { Link, useLocation, Outlet } from "@tanstack/react-router";
import { useSnowflakeInfo } from "@/hooks/useSnowflakeData";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import ThemeToggle from "@/components/themeToggle.tsx";
import CelestialBackground from "@/components/CelestialBackground";
import CyberBackground from "@/components/CyberBackground";
import { FaSnowflake, FaBars } from "react-icons/fa";
import { snowflakeRoutes, type RouteConfig } from "@/routes";

function DatabaseBadge() {
  const { data: info, isLoading } = useSnowflakeInfo();
  if (isLoading) {
    return <div className="h-10 w-20 animate-pulse rounded bg-muted" />;
  }
  return (
    <Badge variant="secondary" className="font-mono h-10 ml-2 bg-transparent">
      DB: {info?.database ?? "Unknown"}
    </Badge>
  );
}

function NavSection({
  label,
  items,
  sidebarOpen,
  pathname,
}: {
  label: string;
  items: RouteConfig[];
  sidebarOpen: boolean;
  pathname: string;
}) {
  return (
    <div>
      {sidebarOpen && (
        <div className="px-4 py-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          {label}
        </div>
      )}
      <ul className="space-y-1">
        {items.map((item) => {
          const isActive =
            item.path === "/"
              ? pathname === "/"
              : pathname === item.path;
          return (
            <li key={item.path} style={{ marginLeft: "12px" }}>
              <Link
                to={item.path}
                className={cn(
                  "flex items-center gap-3 px-2 py-2 rounded-md text-sm transition-colors",
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "hover:bg-accent hover:text-accent-foreground",
                )}
                title={!sidebarOpen ? item.label : undefined}
              >
                <span className="shrink-0">{item.icon}</span>
                {sidebarOpen && <span>{item.label}</span>}
              </Link>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export default function DashboardLayout() {
  const location = useLocation();
  const [sidebarOpen, setSidebarOpen] = useState(true);

  return (
    <div className="min-h-screen flex">
      <CelestialBackground />
      <CyberBackground />
      {/* Sidebar */}
      <aside
        aria-label="Warehouse navigation"
        className={cn(
          "border-r bg-card transition-[width] duration-300 flex flex-col shrink-0",
          sidebarOpen ? "w-56" : "w-14",
        )}
      >
        {/* Database + theme toggle */}
        {sidebarOpen && (
          <div className="border-b flex items-center gap-1 px-2 h-12">
            <FaSnowflake className="text-muted-foreground shrink-0 ml-1" size={14} />
            <DatabaseBadge />
            <div className="ml-auto">
              <ThemeToggle />
            </div>
          </div>
        )}

        {/* Navigation */}
        <nav className="flex-1 py-3 overflow-y-auto pr-3 space-y-4">
          <NavSection
            label="Snowflake Analytics"
            items={snowflakeRoutes}
            sidebarOpen={sidebarOpen}
            pathname={location.pathname}
          />
        </nav>

        {/* Collapse Button */}
        <div className="p-2 border-t">
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-center"
            onClick={() => setSidebarOpen(!sidebarOpen)}
            aria-label={sidebarOpen ? "Collapse sidebar" : "Expand sidebar"}
            aria-expanded={sidebarOpen}
          >
            <FaBars />
          </Button>
        </div>
      </aside>

      {/* Main Content */}
      <div className="flex-1 flex flex-col min-h-screen overflow-hidden">
        <main className="flex-1 overflow-y-auto p-6 bg-background">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
