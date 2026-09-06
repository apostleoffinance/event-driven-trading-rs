"use client";

import { useEffect, useState } from "react";
import { EquityChart } from "@/components/charts/EquityChart";
import { Metric } from "@/components/data-display/Metric";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatMoney, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { PortfolioSnapshot } from "@/types/domain";

export default function PortfolioPage() {
  const [p, setP] = useState<PortfolioSnapshot | null>(null);

  useEffect(() => {
    getRepositories().portfolio.getSnapshot().then(setP);
  }, []);

  if (!p) return <div className="loading">Loading portfolio…</div>;

  return (
    <>
      <PageHeader
        title="Portfolio"
        description="Account → Positions → Exposure → P&L. Analytical view only — no rebalancing."
      />
      <div className="grid grid-kpi" style={{ marginBottom: 12 }}>
        <Metric label="TOTAL EQUITY" value={p.totalEquity} />
        <Metric label="REALIZED P&L" value={p.realizedPnl} signed />
        <Metric label="UNREALIZED P&L" value={p.unrealizedPnl} signed />
        <div className="metric panel">
          <div className="metric-label">CUMULATIVE RETURN</div>
          <div className={`metric-value ${p.cumulativeReturnPct >= 0 ? "pos" : "neg"}`}>
            {p.cumulativeReturnPct.toFixed(2)}%
          </div>
          <div className="metric-sub">{p.drawdownPct.toFixed(2)}% drawdown</div>
        </div>      </div>
      <div className="grid grid-dash" style={{ marginBottom: 12 }}>
        <div className="panel">
          <div className="panel-title">EQUITY CURVE</div>
          <EquityChart series={p.equitySeries} />
        </div>
        <div className="panel">
          <div className="panel-title">EXPOSURE</div>
          <div className="metric-sub">Gross {formatMoney(p.grossExposure)}</div>
          <div className="metric-sub">Net {formatMoney(p.netExposure)}</div>
          <div className="metric-sub">Exposure {p.exposurePct.toFixed(1)}%</div>
          <div className="panel-title" style={{ marginTop: 16 }}>
            P&L BY STRATEGY
          </div>
          {p.pnlByStrategy.map((row) => (
            <div key={row.strategyId} className="metric-sub num">
              {row.strategyId}: {formatSignedMoney(row.pnl)}
            </div>
          ))}
          <div className="panel-title" style={{ marginTop: 16 }}>
            P&L BY INSTRUMENT
          </div>
          {p.pnlByInstrument.map((row) => (
            <div key={row.instrumentId} className="metric-sub num">
              {row.instrumentId}: {formatSignedMoney(row.pnl)}
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
