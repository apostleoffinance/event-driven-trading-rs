"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { EquityChart } from "@/components/charts/EquityChart";
import { Metric } from "@/components/data-display/Metric";
import {
  orderTone,
  riskToneClass,
  StatusBadge,
} from "@/components/data-display/StatusBadge";
import { EventStreamList } from "@/components/events/EventStreamList";
import { PageHeader } from "@/components/layout/PageHeader";
import { TradingPipeline } from "@/components/trading/TradingPipeline";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { SimulationControls } from "@/features/simulation/SimulationControls";
import { formatMoney, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type {
  Order,
  PipelineNode,
  PortfolioSnapshot,
  Position,
  RiskOverview,
} from "@/types/domain";

export default function CommandCenterPage() {
  const repos = getRepositories();
  const { events } = useSharedEventStream();
  const [portfolio, setPortfolio] = useState<PortfolioSnapshot | null>(null);
  const [positions, setPositions] = useState<Position[]>([]);
  const [orders, setOrders] = useState<Order[]>([]);
  const [risk, setRisk] = useState<RiskOverview | null>(null);
  const [pipeline, setPipeline] = useState<PipelineNode[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [p, pos, ord, r, pipe] = await Promise.all([
          repos.portfolio.getSnapshot(),
          repos.positions.listPositions(),
          repos.orders.listOrders(),
          repos.risk.getOverview(),
          repos.system.getPipeline(),
        ]);
        if (cancelled) return;
        setPortfolio(p);
        setPositions(pos);
        setOrders(ord.slice(0, 8));
        setRisk(r);
        setPipeline(pipe);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : "Load failed");
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [repos.portfolio, repos.positions, repos.orders, repos.risk, repos.system]);

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

  const util =
    (risk.dailyLossPct / risk.dailyLossLimitPct) * 100;

  return (
    <>
      <PageHeader
        title="Command Center"
        description="What is happening in the trading system right now — PAPER simulation of Strategy → TradeIntent → Risk → OMS → Venue."
      />
      <div style={{ marginBottom: 12 }}>
        <SimulationControls />
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
            max {portfolio.maxDrawdownPct.toFixed(2)}%
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
              Risk Engine {risk.engineStatus}
            </StatusBadge>
          </div>
          <div className="metric-label">DAILY LOSS</div>
          <div className={`meter ${util > 80 ? "bad" : util > 50 ? "warn" : ""}`}>
            <span style={{ width: `${Math.min(util, 100)}%` }} />
          </div>
          <div className="metric-sub num">
            {risk.dailyLossPct.toFixed(2)}% / {risk.dailyLossLimitPct.toFixed(2)}%
          </div>
          <div className="metric-label" style={{ marginTop: 12 }}>
            MAX DRAWDOWN
          </div>
          <div className="meter">
            <span
              style={{
                width: `${(risk.maxDrawdownPct / risk.maxDrawdownLimitPct) * 100}%`,
              }}
            />
          </div>
          <div className="metric-sub">
            {risk.maxDrawdownPct.toFixed(2)}% / {risk.maxDrawdownLimitPct.toFixed(2)}%
          </div>
          <div className="metric-sub" style={{ marginTop: 12 }}>
            Open risk {risk.openRiskPct.toFixed(2)}% · Exposure{" "}
            {risk.exposurePct.toFixed(1)}% · Kill switch{" "}
            {risk.killSwitch ? "ON" : "OFF"}
          </div>
        </div>
      </div>
      <div className="panel" style={{ marginBottom: 12 }}>
        <div className="panel-title">TRADING PIPELINE</div>
        <TradingPipeline nodes={pipeline} />
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
                    <td>{p.strategyId}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
        <div className="panel">
          <div className="panel-title">EVENT STREAM</div>
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
          {formatSignedMoney(portfolio.realizedPnl)} · Gross exposure{" "}
          {formatMoney(portfolio.grossExposure)}
        </div>
      </div>
    </>
  );
}
