"use client";

import { EventStreamProvider, useSharedEventStream } from "@/features/events/EventStreamProvider";
import { QueryProvider } from "@/features/query/QueryProvider";
import { SimulationProvider } from "@/features/simulation/SimulationProvider";
import { ThemeProvider } from "@/features/theme/ThemeProvider";
import { AuthGate, AuthProvider } from "@/features/auth/AuthProvider";
import { usePathname } from "next/navigation";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { useState } from "react";

function ShellInner({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const authPage = ["/login", "/register", "/verify-email", "/forgot-password", "/reset-password"].some((path) => pathname === path || pathname.startsWith(path + "/"));
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const stream = useSharedEventStream();

  return (
    <AuthGate>
    {authPage ? <div className="auth-layout">{children}</div> : <SimulationProvider pushEvent={stream.push} resetEvents={stream.reset}>
      <div className={`app-shell${collapsed ? " collapsed" : ""}`}>
        <Topbar
          collapsed={collapsed}
          eventConnected={stream.connected}
          onToggleCollapse={() => setCollapsed((v) => !v)}
          onToggleMobile={() => setMobileOpen((v) => !v)}
        />
        <Sidebar
          collapsed={collapsed}
          mobileOpen={mobileOpen}
          onNavigate={() => setMobileOpen(false)}
        />
        <main className="main">{children}</main>
      </div>
    </SimulationProvider>}
    </AuthGate>
  );
}

export function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider>
      <QueryProvider>
        <AuthProvider>
        <EventStreamProvider>
          <ShellInner>{children}</ShellInner>
        </EventStreamProvider>
        </AuthProvider>
      </QueryProvider>
    </ThemeProvider>
  );
}
