"use client";

import { useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import {
  riskDecisionTone,
  riskToneClass,
  StatusBadge,
} from "@/components/data-display/StatusBadge";
import { getRepositories } from "@/lib/repositories";
import type { RiskDecisionRecord, RiskOverview } from "@/types/domain";

export default function RiskPage() {
  const [overview, setOverview] = useState<RiskOverview | null>(null);
  const [decisions, setDecisions] = useState<RiskDecisionRecord[]>([]);

  useEffect(() => {
    const repo = getRepositories().risk;
    Promise.all([repo.getOverview(), repo.listDecisions()]).then(([o, d]) => {
      setOverview(o);
      setDecisions(d);
    });
  }, []);

  if (!overview) return <div className="loading">Loading risk…</div>;

  return (
    <>
      <PageHeader
        title="Risk Center"
        description="Risk has final authority before OMS. Hierarchy: Firm → Portfolio → Account → Strategy → Trade."
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
                width: `${(overview.dailyLossPct / overview.dailyLossLimitPct) * 100}%`,
              }}
            />
          </div>
          <div className="metric-sub num">
            {overview.dailyLossPct.toFixed(2)}% / {overview.dailyLossLimitPct.toFixed(2)}%
          </div>
        </div>
        <div className="panel">
          <div className="metric-label">MAX DRAWDOWN</div>
          <div className="meter">
            <span
              style={{
                width: `${(overview.maxDrawdownPct / overview.maxDrawdownLimitPct) * 100}%`,
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
          <div className="metric-value" style={{ fontSize: 16 }}>
            {overview.openRiskPct.toFixed(2)}%
          </div>
          <div className="metric-sub">Exposure {overview.exposurePct.toFixed(1)}%</div>
        </div>
      </div>
      <div className="panel" style={{ marginBottom: 12 }}>
        <div className="panel-title">RISK HIERARCHY</div>
        <div className="pipeline">
          {["FIRM", "PORTFOLIO", "ACCOUNT", "STRATEGY", "TRADE"].map((level, i) => (
            <div key={level} style={{ display: "contents" }}>
              {i > 0 ? <div className="pipeline-arrow">→</div> : null}
              <div className="pipeline-node">
                <div className="metric-label">{level} RISK</div>
              </div>
            </div>
          ))}
        </div>
      </div>
      <div className="data-table-wrap panel">
        <div className="panel-title">RISK DECISIONS</div>
        <table className="data-table">
          <thead>
            <tr>
              <th>Time</th>
              <th>Intent</th>
              <th>Strategy</th>
              <th>Instrument</th>
              <th>Requested</th>
              <th>Approved</th>
              <th>Decision</th>
              <th>Reason</th>
            </tr>
          </thead>
          <tbody>
            {decisions.map((d) => (
              <tr key={d.id}>
                <td className="num">{new Date(d.time).toLocaleTimeString()}</td>
                <td className="num">{d.tradeIntentId}</td>
                <td>{d.strategyId}</td>
                <td>{d.instrumentId}</td>
                <td className="num">{d.requestedQuantity ?? "—"}</td>
                <td className="num">{d.approvedQuantity ?? "—"}</td>
                <td>
                  <StatusBadge tone={riskDecisionTone(d.decision)}>
                    {d.decision}
                  </StatusBadge>
                </td>
                <td>{d.reason}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
