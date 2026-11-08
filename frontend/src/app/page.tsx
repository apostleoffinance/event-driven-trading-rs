"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { EquityChart } from "@/components/charts/EquityChart";
import { JournalDashboardSummary } from "@/components/dashboard/JournalDashboardSummary";
import { Metric } from "@/components/data-display/Metric";
import {
  orderTone,
  riskToneClass,
  StatusBadge,
} from "@/components/data-display/StatusBadge";
import { EventStreamList } from "@/components/events/EventStreamList";
import { PageHeader } from "@/components/layout/PageHeader";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { SimulationControls } from "@/features/simulation/SimulationControls";
import { formatMoney, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import { frontendConfig } from "@/config/frontend";
import type {
  Account,
  Order,
  PortfolioSnapshot,
  Position,
  RiskOverview,
} from "@/types/domain";

export default function CommandCenterPage() {
  const repos = getRepositories();
  const { events } = useSharedEventStream();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [portfolio, setPortfolio] = useState<PortfolioSnapshot | null>(null);
  const [positions, setPositions] = useState<Position[]>([]);
  const [orders, setOrders] = useState<Order[]>([]);
  const [risk, setRisk] = useState<RiskOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [acct, p, pos, ord, r] = await Promise.all([
          repos.accounts.listAccounts(),
          repos.portfolio.getSnapshot(),
          repos.positions.listPositions(),
          repos.orders.listOrders(),
          repos.risk.getOverview(),
        ]);
        if (cancelled) return;
        setAccounts(acct);
        setPortfolio(p);
        setPositions(pos);
        setOrders(ord.slice(0, 8));
        setRisk(r);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : "Load failed");
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [repos.accounts, repos.portfolio, repos.positions, repos.orders, repos.risk]);

  if (loading) return <div className="loading">Loading command center…</div>;
  if (error || !portfolio || !risk)
    return (
      <div className="error">
        Unable to load data. {error}
        <div style={{ marginTop: 8 }}>
          <button type="button" className="btn" onClick={() => location.reload()}>
            Retry
          </button>
        </div>
      </div>
    );

  if (frontendConfig.dataMode === "api" && accounts.length === 0) {
    return (
      <>
        <PageHeader title="Your trading dashboard" description="Your trading accounts, performance and risk overview in one place." />
        <section className="panel" style={{ maxWidth: 760, margin: "24px auto", textAlign: "center", padding: 32 }}>
          <h2>Welcome to EdgeNest</h2>
          <p>You haven’t added a trading account yet. Create a paper account to start tracking your trading activity.</p>
          <div style={{ display: "flex", gap: 12, justifyContent: "center", flexWrap: "wrap", marginTop: 16 }}>
            <Link href="/accounts" className="btn primary">Set up a trading account</Link>
            <Link href="/journal" className="btn">Start your trading journal</Link>
          </div>
        </section>
      </>
    );
  }

  const util =
    risk.dailyLossLimitPct && risk.dailyLossLimitPct > 0 ? (risk.dailyLossPct / risk.dailyLossLimitPct) * 100 : 0;

  return (
    <>
      <PageHeader
        title="Dashboard"
        description={frontendConfig.dataMode === "api" ? "Your trading accounts, performance and risk overview." : "What is happening in your PAPER trading simulation."}
      />
      <JournalDashboardSummary />
      <div style={{ marginBottom: 12 }}>
        {frontendConfig.dataMode === "mock" ? <SimulationControls /> : null}
      </div>
      <div className="grid grid-kpi" style={{ marginBottom: 12 }}>
        <Metric
          label="TOTAL EQUITY"
          value={portfolio.totalEquity}
          pct={portfolio.cumulativeReturnPct}
        />
        <Metric label="AVAILABLE CAPITAL" value={portfolio.availableCapital} />
        <Metric label="TODAY'S P&L" value={portfolio.dailyPnl} signed />
        <div className="metric panel">
          <div className="metric-label">DRAWDOWN</div>
          <div className="metric-value neg">{portfolio.drawdownPct.toFixed(2)}%</div>
          <div className="metric-sub">
            max {portfolio.maxDrawdownPct == null ? "N/A" : `${portfolio.maxDrawdownPct.toFixed(2)}%`}
          </div>
        </div>
      </div>
      <div className="grid grid-dash" style={{ marginBottom: 12 }}>
        <div className="panel">
          <div className="panel-title">EQUITY / P&L</div>
          <EquityChart series={portfolio.equitySeries} />
        </div>
        <div className="panel">
          <div className="panel-title">RISK STATUS</div>
          <div style={{ marginBottom: 8 }}>
            <StatusBadge tone={riskToneClass(risk.tone)}>
              {risk.killSwitch ? "Trading halted by risk controls" : risk.tone === "BREACH" ? "Risk limit breached" : risk.engineStatus === "HEALTHY" ? "Risk limits configured" : risk.engineStatus === "NOT_CONFIGURED" ? "Risk limits not configured" : "Risk status unavailable"}
            </StatusBadge>
          </div>
          <div className="metric-label">DAILY LOSS</div>
          <div className={`meter ${util > 80 ? "bad" : util > 50 ? "warn" : ""}`}>
            <span style={{ width: `${Math.min(util, 100)}%` }} />
          </div>
          <div className="metric-sub num">
            {risk.dailyLossPct.toFixed(2)}% / {risk.dailyLossLimitPct == null ? "Not configured" : `${risk.dailyLossLimitPct.toFixed(2)}%`}
          </div>
          <div className="metric-label" style={{ marginTop: 12 }}>
            MAX DRAWDOWN
          </div>
          <div className="meter">
            <span
              style={{
                width: `${risk.maxDrawdownLimitPct && risk.maxDrawdownLimitPct > 0 ? (risk.maxDrawdownPct / risk.maxDrawdownLimitPct) * 100 : 0}%`,
              }}
            />
          </div>
          <div className="metric-sub">
            {risk.maxDrawdownPct.toFixed(2)}% / {risk.maxDrawdownLimitPct == null ? "Not configured" : `${risk.maxDrawdownLimitPct.toFixed(2)}%`}
          </div>
          <div className="metric-sub" style={{ marginTop: 12 }}>
            Open risk {risk.openRiskPct == null ? "Not available" : `${risk.openRiskPct.toFixed(2)}%`} · Exposure{" "}
            {risk.exposurePct == null ? "Not available" : `${risk.exposurePct.toFixed(1)}%`} · Kill switch{" "}
            {risk.killSwitch ? "ON" : "OFF"}
          </div>
        </div>
      </div>
      <div className="grid grid-dash">
        <div className="panel">
          <div
            className="panel-title"
            style={{ display: "flex", justifyContent: "space-between" }}
          >
            <span>ACTIVE POSITIONS</span>
            <Link href="/positions" className="metric-sub">
              View all →
            </Link>
          </div>
          <div className="data-table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Instrument</th>
                  <th>Side</th>
                  <th>Qty</th>
                  <th>Entry</th>
                  <th>Mark</th>
                  <th>uP&L</th>
                  <th>Strategy</th>
                </tr>
              </thead>
              <tbody>
                {!positions.length ? <tr><td colSpan={7} className="muted">No open positions yet. Your positions will appear here when trades are opened.</td></tr> : null}
                {positions.map((p) => (
                  <tr key={p.id}>
                    <td>{p.instrumentId}</td>
                    <td>{p.side}</td>
                    <td className="num">{p.quantity}</td>
                    <td className="num">{p.entryPrice}</td>
                    <td className="num">{p.markPrice}</td>
                    <td
                      className={`num ${Number(p.unrealizedPnl) >= 0 ? "pos" : "neg"}`}
                    >
                      {formatSignedMoney(p.unrealizedPnl)}
                    </td>
                    <td>{p.strategyId ?? "Unattributed"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
        <div className="panel">
          <div className="panel-title">RECENT ACTIVITY</div>
          <EventStreamList events={events} limit={8} />
        </div>
      </div>
      <div className="panel" style={{ marginTop: 12 }}>
        <div
          className="panel-title"
          style={{ display: "flex", justifyContent: "space-between" }}
        >
          <span>RECENT ORDERS</span>
          <Link href="/orders" className="metric-sub">
            View all →
          </Link>
        </div>
        <div className="data-table-wrap">
          <table className="data-table">
            <thead>
              <tr>
                <th>Time</th>
                <th>Instrument</th>
                <th>Side</th>
                <th>Qty</th>
                <th>Status</th>
                <th>Price</th>
                <th>Strategy</th>
              </tr>
            </thead>
            <tbody>
              {!orders.length ? <tr><td colSpan={7} className="muted">No orders yet. Your order history will appear here when you start trading.</td></tr> : null}
              {orders.map((o) => (
                <tr key={o.id}>
                  <td className="num">{new Date(o.updatedAt).toLocaleTimeString()}</td>
                  <td>{o.instrumentId}</td>
                  <td>{o.side}</td>
                  <td className="num">{o.quantity}</td>
                  <td>
                    <StatusBadge tone={orderTone(o.status)}>{o.status}</StatusBadge>
                  </td>
                  <td className="num">{o.price ?? "—"}</td>
                  <td>{o.strategyId}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="metric-sub muted" style={{ marginTop: 8 }}>
          Unrealized {formatMoney(portfolio.unrealizedPnl)} · Realized{" "}
          {portfolio.realizedPnl == null ? "Unknown" : formatSignedMoney(portfolio.realizedPnl)} · Gross exposure{" "}
          {formatMoney(portfolio.grossExposure)}
        </div>
      </div>
    </>
  );
}
