"use client";
import { EventStreamProvider, useSharedEventStream } from "@/features/events/EventStreamProvider";
import { QueryProvider } from "@/features/query/QueryProvider";
import { ThemeProvider } from "@/features/theme/ThemeProvider";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { PublicLandingPage } from "@/components/landing/PublicLandingPage";
import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import { hasSession, restoreSession } from "@/lib/auth/session";
import { EDGENEST_SYMBOL_SRC } from "@/lib/brand-assets";
import { PerformanceDisplayProvider } from "@/features/performance/PerformanceDisplayProvider";
import { OrganizationProvider } from "@/features/organization/OrganizationProvider";
import { WorkspaceProvider } from "@/features/workspace/WorkspaceProvider";

function ShellInner({ children }: { children: React.ReactNode }) {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const [authResolved, setAuthResolved] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const pathname = usePathname();
  const stream = useSharedEventStream();
  useEffect(() => {\n    let active = true;\n    const initialize = async () => {\n      await restoreSession();\n      if (active) { setAuthenticated(hasSession()); setAuthResolved(true); }\n    };\n    void initialize();\n    return () => { active = false; };\n  }, [pathname]);

  if (pathname === "/" && authResolved && !authenticated) return <PublicLandingPage />;
  if (pathname === "/" && !authResolved) return <div className="landing-boot" aria-label="Loading EdgeNest"><img className="landing-boot-symbol" src={EDGENEST_SYMBOL_SRC} alt="" width={85} height={72} /></div>;

  return <div className={`app-shell${collapsed ? " collapsed" : ""}`}>
    <Topbar collapsed={collapsed} eventConnected={stream.connected} onToggleCollapse={() => setCollapsed(v => !v)} onToggleMobile={() => setMobileOpen(v => !v)} />
    <Sidebar collapsed={collapsed} mobileOpen={mobileOpen} onNavigate={() => setMobileOpen(false)} />
    <main className="main">{children}</main>
  </div>;
}
export function AppShell({ children }: { children: React.ReactNode }) {
  return <ThemeProvider><QueryProvider><OrganizationProvider><WorkspaceProvider><EventStreamProvider><PerformanceDisplayProvider><ShellInner>{children}</ShellInner></PerformanceDisplayProvider></EventStreamProvider></WorkspaceProvider></OrganizationProvider></QueryProvider></ThemeProvider>;
}
