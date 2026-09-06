"use client";

import { EventStreamProvider, useSharedEventStream } from "@/features/events/EventStreamProvider";
import { QueryProvider } from "@/features/query/QueryProvider";
import { SimulationProvider } from "@/features/simulation/SimulationProvider";
import { ThemeProvider } from "@/features/theme/ThemeProvider";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { useState } from "react";

function ShellInner({ children }: { children: React.ReactNode }) {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const stream = useSharedEventStream();

  return (
    <SimulationProvider pushEvent={stream.push} resetEvents={stream.reset}>
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
    </SimulationProvider>
  );
}

export function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider>
      <QueryProvider>
        <EventStreamProvider>
          <ShellInner>{children}</ShellInner>
        </EventStreamProvider>
      </QueryProvider>
    </ThemeProvider>
  );
}
