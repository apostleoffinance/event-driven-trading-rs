"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { ThemeProvider } from "@/features/theme/ThemeProvider";
import { QueryProvider } from "@/features/query/QueryProvider";
import { EventStreamProvider, useEventStream } from "@/features/events/EventStreamProvider";
import { SimulationProvider } from "@/features/simulation/SimulationProvider";
import { AuthGate, AuthProvider } from "@/features/auth/AuthProvider";

function AuthenticatedShell({ children }: { children: React.ReactNode }) {
  const stream = useEventStream();
  const [collapsed, setCollapsed] = useState(false);

  useEffect(() => {
    const stored = localStorage.getItem("quant-os-sidebar");
    if (stored === "collapsed") setCollapsed(true);
  }, []);

  const toggleSidebar = () => {
    setCollapsed((previous) => {
      const next = !previous;
      localStorage.setItem("quant-os-sidebar", next ? "collapsed" : "expanded");
      return next;
    });
  };

  return (
    <SimulationProvider pushEvent={stream.push} resetEvents={stream.reset}>
      <div className={`app-shell${collapsed ? " collapsed" : ""}`}>
        <Sidebar collapsed={collapsed} onToggle={toggleSidebar} />
        <div className="app-main">
          <Topbar onToggleSidebar={toggleSidebar} />
          <main className="app-content">{children}</main>
        </div>
      </div>
    </SimulationProvider>
  );
}

function ShellInner({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const authPage = ["/login", "/register", "/verify-email", "/forgot-password", "/reset-password"]
    .some((path) => pathname === path || pathname.startsWith(path + "/"));

  return (
    <AuthGate>
      {authPage ? <div className="auth-layout">{children}</div> : (
        <EventStreamProvider>
          <AuthenticatedShell>{children}</AuthenticatedShell>
        </EventStreamProvider>
      )}
    </AuthGate>
  );
}

export default function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider>
      <QueryProvider>
        <AuthProvider>
          <ShellInner>{children}</ShellInner>
        </AuthProvider>
      </QueryProvider>
    </ThemeProvider>
  );
}
