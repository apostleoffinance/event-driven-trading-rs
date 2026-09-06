"use client";

import { useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { StatusBadge } from "@/components/data-display/StatusBadge";
import { formatPct } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { Strategy } from "@/types/domain";

export default function StrategiesPage() {
  const [rows, setRows] = useState<Strategy[]>([]);

  useEffect(() => {
    getRepositories().strategies.listStrategies().then(setRows);
  }, []);

  return (
    <>
      <PageHeader
        title="Strategies"
        description="Python strategies emit TradeIntent only — they never submit orders. Execution stays in Rust."
      />
      <div className="grid" style={{ gridTemplateColumns: "repeat(auto-fill,minmax(320px,1fr))" }}>
        {rows.map((s) => (
          <article key={s.id} className="panel">
            <div style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
              <h2 style={{ margin: 0, fontSize: 15 }}>{s.name}</h2>
              <StatusBadge tone={s.status === "RUNNING" ? "ok" : "info"}>
                {s.status}
              </StatusBadge>
            </div>
            <p className="metric-sub muted">{s.description}</p>
            <div className="metric-sub">Version {s.version} · {s.environment}</div>
            <div className="metric-sub">Instrument {s.instrumentId}</div>
            <div className="metric-sub">
              Signals {s.signals} · Trades {s.trades}
            </div>
            <div style={{ marginTop: 10 }} className="grid" >
              <div className="metric-sub num">Return {formatPct(s.returnPct)}</div>
              <div className="metric-sub num">Sharpe {s.sharpe.toFixed(2)}</div>
              <div className="metric-sub num">Max DD {s.maxDrawdownPct.toFixed(2)}%</div>
              <div className="metric-sub num">Win {s.winRatePct.toFixed(1)}%</div>
              <div className="metric-sub num">PF {s.profitFactor.toFixed(2)}</div>
            </div>
          </article>
        ))}
      </div>
    </>
  );
}
