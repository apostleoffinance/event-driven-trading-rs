"use client";

import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "@/components/data-display/DataTable";
import {
  riskDecisionTone,
  riskToneClass,
  StatusBadge,
} from "@/components/data-display/StatusBadge";
import { PageHeader } from "@/components/layout/PageHeader";
import { getRepositories } from "@/lib/repositories";
import type { RiskDecisionRecord } from "@/types/domain";

export default function RiskPage() {
  const overviewQ = useQuery({
    queryKey: ["risk", "overview"],
    queryFn: () => getRepositories().risk.getOverview(),
  });
  const decisionsQ = useQuery({
    queryKey: ["risk", "decisions"],
    queryFn: () => getRepositories().risk.listDecisions(),
  });

  const columns = useMemo<ColumnDef<RiskDecisionRecord>[]>(
    () => [
      {
        accessorKey: "time",
        header: "Time",
        cell: ({ getValue }) => (
          <span className="num">
            {new Date(String(getValue())).toLocaleTimeString()}
          </span>
        ),
      },
      {
        accessorKey: "tradeIntentId",
        header: "Intent",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      { accessorKey: "strategyId", header: "Strategy" },
      { accessorKey: "instrumentId", header: "Instrument" },
      {
        accessorKey: "requestedQuantity",
        header: "Requested",
        cell: ({ row }) => (
          <span className="num">{row.original.requestedQuantity ?? "—"}</span>
        ),
      },
      {
        accessorKey: "approvedQuantity",
        header: "Approved",
        cell: ({ row }) => (
          <span className="num">{row.original.approvedQuantity ?? "—"}</span>
        ),
      },
      {
        accessorKey: "decision",
        header: "Decision",
        cell: ({ row }) => (
          <StatusBadge tone={riskDecisionTone(row.original.decision)}>
            {row.original.decision}
          </StatusBadge>
        ),
      },
      { accessorKey: "reason", header: "Reason" },
    ],
    [],
  );

  if (overviewQ.isLoading || decisionsQ.isLoading)
    return <div className="loading">Loading risk…</div>;
  if (overviewQ.isError || decisionsQ.isError)
    return <div className="error">Failed to load risk center.</div>;

  const overview = overviewQ.data!;
  const decisions = decisionsQ.data ?? [];

  return (
    <>
      <PageHeader
        title="Risk Center"
        description="Risk has final authority before OMS. Hierarchy: Firm → Portfolio → Account → Strategy → Trade. No bypass controls."
      />
      <div className="grid grid-kpi" style={{ marginBottom: 12 }}>
        <div className="panel">
          <div className="metric-label">RISK ENGINE</div>
          <StatusBadge tone={riskToneClass(overview.tone)}>
            {overview.engineStatus}
          </StatusBadge>
          <div className="metric-sub" style={{ marginTop: 8 }}>
            Kill switch {overview.killSwitch ? "ON" : "OFF"}
          </div>
        </div>
        <div className="panel">
          <div className="metric-label">DAILY LOSS</div>
          <div className="meter">
            <span
              style={{
                width: `${Math.min(
                  100,
                  (overview.dailyLossPct / overview.dailyLossLimitPct) * 100,
                )}%`,
              }}
            />
          </div>
          <div className="metric-sub num">
            {overview.dailyLossPct.toFixed(2)}% /{" "}
            {overview.dailyLossLimitPct.toFixed(2)}%
          </div>
        </div>
        <div className="panel">
          <div className="metric-label">MAX DRAWDOWN</div>
          <div className="meter">
            <span
              style={{
                width: `${Math.min(
                  100,
                  (overview.maxDrawdownPct / overview.maxDrawdownLimitPct) * 100,
                )}%`,
              }}
            />
          </div>
          <div className="metric-sub num">
            {overview.maxDrawdownPct.toFixed(2)}% /{" "}
            {overview.maxDrawdownLimitPct.toFixed(2)}%
          </div>
        </div>
        <div className="panel">
          <div className="metric-label">OPEN RISK / EXPOSURE</div>
          <div className="metric-value">{overview.openRiskPct.toFixed(2)}%</div>
          <div className="metric-sub">
            Exposure {overview.exposurePct.toFixed(1)}%
          </div>
        </div>
      </div>

      <div className="panel" style={{ marginBottom: 12 }}>
        <div className="panel-title">RISK HIERARCHY</div>
        <div className="grid hierarchy-grid">
          {overview.hierarchy.map((level) => (
            <div key={level.id} className="pipeline-node hierarchy-card">
              <div className="metric-label">{level.label}</div>
              <div className="metric-sub">{level.scope}</div>
              <StatusBadge tone={riskToneClass(level.tone)}>
                {level.status}
              </StatusBadge>
              <div className="meter" style={{ marginTop: 8 }}>
                <span style={{ width: `${Math.min(100, level.utilizationPct)}%` }} />
              </div>
              <div className="metric-sub num">{level.limitLabel}</div>
              <div className="metric-sub muted" style={{ fontFamily: "var(--font-sans)" }}>
                {level.note}
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="panel">
        <div className="panel-title">RISK DECISIONS</div>
        <DataTable
          data={decisions}
          columns={columns}
          emptyMessage="No risk decisions."
        />
      </div>
    </>
  );
}
