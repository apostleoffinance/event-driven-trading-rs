"use client";

import { useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { StatusBadge } from "@/components/data-display/StatusBadge";
import { formatDateTime } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { ReconciliationResult } from "@/types/domain";

export default function ReconciliationPage() {
  const [data, setData] = useState<ReconciliationResult | null>(null);

  useEffect(() => {
    getRepositories().reconciliation.getLatest().then(setData);
  }, []);

  if (!data) return <div className="loading">Loading reconciliation…</div>;

  return (
    <>
      <PageHeader
        title="Reconciliation"
        description="Alert-only compare of internal vs venue state. Mismatches never silently overwrite books."
      />
      <div className="panel" style={{ marginBottom: 12 }}>
        <StatusBadge tone={data.status === "HEALTHY" ? "ok" : "warn"}>
          {data.status}
        </StatusBadge>
        <div className="metric-sub" style={{ marginTop: 8 }}>
          Account {data.accountId} · Venue {data.venueId} · Last run{" "}
          {formatDateTime(data.lastRunAt)}
        </div>
      </div>
      <div className="data-table-wrap panel" style={{ marginBottom: 12 }}>
        <table className="data-table">
          <thead>
            <tr>
              <th>Field</th>
              <th>Internal</th>
              <th>Venue</th>
              <th>Status</th>
              <th>Difference</th>
            </tr>
          </thead>
          <tbody>
            {data.lines.map((line) => (
              <tr key={line.field}>
                <td>{line.field}</td>
                <td className="num">{line.internal}</td>
                <td className="num">{line.venue}</td>
                <td>
                  <StatusBadge tone={line.status === "MATCH" ? "ok" : "warn"}>
                    {line.status === "MATCH" ? "✓ MATCH" : "⚠ MISMATCH"}
                  </StatusBadge>
                </td>
                <td className="num">{line.difference ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {data.alerts.map((a) => (
        <div key={a.title} className="panel" style={{ marginBottom: 8 }}>
          <div className="panel-title">⚠ {a.title}</div>
          <p className="metric-sub">{a.detail}</p>
          <StatusBadge tone="warn">ACTION: {a.action.replace("_", " ")}</StatusBadge>
        </div>
      ))}
    </>
  );
}
