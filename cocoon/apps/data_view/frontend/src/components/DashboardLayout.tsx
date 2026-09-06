import { Link, useLocation, Outlet } from "@tanstack/react-router";
import { cn } from "@/lib/utils";
import ThemeToggle from "@/components/themeToggle";
import CelestialBackground from "@/components/CelestialBackground";
import CyberBackground from "@/components/CyberBackground";
import { dataViewRoutes } from "@/routes";

export default function DashboardLayout() {
  const { pathname } = useLocation();

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <CelestialBackground />
      <CyberBackground />
      {/* Top navigation bar */}
      <header className="sticky top-0 z-20 flex h-12 items-center gap-6 border-b bg-card px-4">
        <Link
          to="/"
          aria-label="Data View home"
          className="flex shrink-0 items-center rounded-md text-sm font-semibold outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        >
          Data View
        </Link>

        <nav className="flex h-full items-stretch gap-1" aria-label="Primary">
          {dataViewRoutes.map((item) => {
            const isActive =
              item.path === "/"
                ? pathname === "/"
                : pathname.startsWith(item.path);
            return (
              <Link
                key={item.path}
                to={item.path}
                aria-current={isActive ? "page" : undefined}
                className={cn(
                  "-mb-px flex h-full items-center border-b-2 px-3 text-sm transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
                  isActive
                    ? "border-primary font-medium text-foreground"
                    : "border-transparent text-muted-foreground hover:text-foreground",
                )}
              >
                {item.label}
              </Link>
            );
          })}
        </nav>

        <div className="ml-auto flex items-center gap-2">
          <ThemeToggle />
        </div>
      </header>

      {/* Main content — full width */}
      <main className="flex flex-1 flex-col overflow-y-auto bg-background p-6">
        <Outlet />
      </main>
    </div>
  );
}
