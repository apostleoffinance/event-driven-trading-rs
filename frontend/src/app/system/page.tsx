"use client";

import { useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { healthTone, StatusBadge } from "@/components/data-display/StatusBadge";
import { formatDateTime } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { SystemComponent } from "@/types/domain";

export default function SystemPage() {
  const [rows, setRows] = useState<SystemComponent[]>([]);

  useEffect(() => {
    getRepositories().system.listComponents().then(setRows);
  }, []);

  return (
    <>
      <PageHeader
        title="System Health"
        description="Infrastructure components for the modular Rust stack + Python strategy path."
      />
      <div
        className="grid"
        style={{ gridTemplateColumns: "repeat(auto-fill,minmax(260px,1fr))" }}
      >
        {rows.map((c) => (
          <article key={c.id} className="panel">
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <strong>{c.name}</strong>
              <StatusBadge tone={healthTone(c.status)}>{c.status}</StatusBadge>
            </div>
            <div className="metric-sub">Uptime {c.uptime}</div>
            <div className="metric-sub">
              Heartbeat {formatDateTime(c.lastHeartbeat)}
            </div>
            <div className="metric-sub">Latency {c.latencyMs} ms</div>
            <div className="metric-sub">Errors {c.errorCount}</div>
            {c.eventsPerSec !== undefined ? (
              <div className="metric-sub">Events/sec {c.eventsPerSec}</div>
            ) : null}
            {c.lastEvent ? (
              <div className="metric-sub muted">Last: {c.lastEvent}</div>
            ) : null}
          </article>
        ))}
      </div>
    </>
  );
}
