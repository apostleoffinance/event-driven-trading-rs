"use client";

import { useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { healthTone, StatusBadge } from "@/components/data-display/StatusBadge";
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
        title="Connections & status"
        description="Check the availability of your trading services and connected venues."
      />
      <div
        className="grid"
        style={{ gridTemplateColumns: "repeat(auto-fill,minmax(260px,1fr))" }}
      >
        {rows.map((c) => (
          <article key={c.id} className="panel">
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <strong>{c.name}</strong>
              <StatusBadge tone={healthTone(c.status)}>{({HEALTHY:"Available",DEGRADED:"Needs attention",DISCONNECTED:"Disconnected",ERROR:"Unavailable",UNKNOWN:"Status unavailable",NOT_CONFIGURED:"Not configured"} as Record<SystemComponent["status"],string>)[c.status]}</StatusBadge>
            </div>
            <p className="metric-sub">{c.status === "HEALTHY" ? "This service is responding." : c.status === "NOT_CONFIGURED" ? "Complete setup to enable this service." : c.status === "UNKNOWN" ? "We do not have verified status information for this service yet." : "This service may need attention."}</p>
          </article>
        ))}
      </div>
    </>
  );
}
