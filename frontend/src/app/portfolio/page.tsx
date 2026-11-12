"use client";

import { useEffect, useState } from "react";
import { EquityChart } from "@/components/charts/EquityChart";
import { Metric } from "@/components/data-display/Metric";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatMoney, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { PortfolioSnapshot } from "@/types/domain";
import { journalApi } from "@/lib/api/journal";

export default function PortfolioPage() {
  const [p, setP] = useState<PortfolioSnapshot | null>(null);
  const [journalAccounts, setJournalAccounts] = useState<Array<{account_id:string;name:string;currency:string;initial_balance:string;realized_net_pnl:string;journal_equity:string;trade_count:number}>>([]);
  const [journalError, setJournalError] = useState<string | null>(null);

  useEffect(() => {
    getRepositories().portfolio.getSnapshot().then(setP);
    journalApi.journalPortfolio().then(setJournalAccounts).catch((error: unknown) => setJournalError(error instanceof Error ? error.message : "Unable to load Journal portfolio."));
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
        <Metric label="REALIZED P&L" value={p.realizedPnl ?? "Unknown"} signed />
        <Metric label="UNREALIZED P&L" value={p.unrealizedPnl} signed />
        <div className="metric panel">
          <div className="metric-label">CUMULATIVE RETURN</div>
          <div className={`metric-value ${p.cumulativeReturnPct >= 0 ? "pos" : "neg"}`}>
            {p.cumulativeReturnPct.toFixed(2)}%
          </div>
          <div className="metric-sub">{p.drawdownPct.toFixed(2)}% drawdown</div>
        </div>      </div>
      <div className="panel" style={{ marginBottom: 12 }}>
        <div className="panel-title">JOURNAL ACCOUNT PERFORMANCE</div>
        <p className="metric-sub">Journal-only accounts are shown separately from broker-reported equity. Linked Journal accounts are excluded here to prevent double counting. Values are calculated as initial balance plus recorded closed-trade net P&amp;L; deposits, withdrawals and unrealized P&amp;L are not included.</p>
        {journalError ? <div className="error" role="alert">{journalError}</div> : journalAccounts.length === 0 ? <p className="metric-sub">No unlinked Journal accounts to report.</p> : <div style={{overflowX:"auto"}}><table className="data-table"><thead><tr><th>Account</th><th>Currency</th><th>Initial balance</th><th>Realized net P&amp;L</th><th>Journal equity</th><th>Trades</th></tr></thead><tbody>{journalAccounts.map((account)=><tr key={account.account_id}><td>{account.name}</td><td>{account.currency}</td><td className="num">{formatMoney(account.initial_balance,account.currency)}</td><td className={`num ${Number(account.realized_net_pnl)>=0?"pos":"neg"}`}>{formatSignedMoney(account.realized_net_pnl)} {account.currency}</td><td className="num">{formatMoney(account.journal_equity,account.currency)}</td><td className="num">{account.trade_count}</td></tr>)}</tbody></table></div>}
      </div>
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
