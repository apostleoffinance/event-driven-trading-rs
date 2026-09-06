"use client";

import { useQuery } from "@tanstack/react-query";
import { PageHeader } from "@/components/layout/PageHeader";
import { StatusBadge } from "@/components/data-display/StatusBadge";
import { formatDateTime } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";

export default function ReconciliationPage() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["reconciliation"],
    queryFn: () => getRepositories().reconciliation.getLatest(),
  });

  if (isLoading) return <div className="loading">Loading reconciliation…</div>;
  if (isError || !data)
    return <div className="error">Failed to load reconciliation.</div>;

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
        <div className="metric-sub muted" style={{ fontFamily: "var(--font-sans)" }}>
          Account ≠ Venue. Action on mismatch is ALERT ONLY — no auto-fix control.
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
                    {line.status === "MATCH" ? "MATCH" : "MISMATCH"}
                  </StatusBadge>
                </td>
                <td className="num">{line.difference ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {data.alerts.length === 0 ? (
        <div className="empty">No reconciliation alerts.</div>
      ) : (
        data.alerts.map((a) => (
          <div key={a.title} className="panel" style={{ marginBottom: 8 }}>
            <div className="panel-title">{a.title}</div>
            <p className="metric-sub" style={{ fontFamily: "var(--font-sans)" }}>
              {a.detail}
            </p>
            <StatusBadge tone="warn">ALERT ONLY</StatusBadge>
          </div>
        ))
      )}
    </>
  );
}
