"use client";

import { useEffect, useMemo, useState } from "react";
import { EquityChart } from "@/components/charts/EquityChart";
import { Metric } from "@/components/data-display/Metric";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatMoney, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { EquityPoint, PortfolioSnapshot } from "@/types/domain";
import { journalApi, type JournalTrade } from "@/lib/api/journal";

type JournalPortfolioAccount = {
  account_id: string;
  name: string;
  currency: string;
  initial_balance: string;
  realized_net_pnl: string;
  journal_equity: string;
  trade_count: number;
  linked_trading_account_id: string | null;
};

const emptyPortfolio: PortfolioSnapshot = {
  totalEquity: "0", balance: "0", availableCapital: "0", dailyPnl: "0",
  unrealizedPnl: "0", realizedPnl: null, cumulativeReturnPct: 0, drawdownPct: 0,
  maxDrawdownPct: null, grossExposure: "0", netExposure: "0", exposurePct: 0,
  equitySeries: [], pnlByStrategy: [], pnlByInstrument: [],
};

const amount = (value: string | null | undefined) => {
  const parsed = Number(value ?? 0);
  return Number.isFinite(parsed) ? parsed : 0;
};

export default function PortfolioPage() {
  const [brokerPortfolio, setBrokerPortfolio] = useState<PortfolioSnapshot | null>(null);
  const [brokerError, setBrokerError] = useState<string | null>(null);
  const [journalAccounts, setJournalAccounts] = useState<JournalPortfolioAccount[]>([]);
  const [journalTrades, setJournalTrades] = useState<JournalTrade[]>([]);
  const [journalError, setJournalError] = useState<string | null>(null);
  const [brokerLoading, setBrokerLoading] = useState(true);
  const [journalLoading, setJournalLoading] = useState(true);

  useEffect(() => {
    let active = true;
    getRepositories().portfolio.getSnapshot()
      .then((snapshot) => { if (active) setBrokerPortfolio(snapshot); })
      .catch((error: unknown) => { if (active) setBrokerError(error instanceof Error ? error.message : "Trading portfolio is unavailable."); })
      .finally(() => { if (active) setBrokerLoading(false); });

    Promise.all([journalApi.journalPortfolio(), journalApi.listTrades({ limit: 500, offset: 0 })])
      .then(([accounts, tradesPage]) => {
        if (!active) return;
        setJournalAccounts(accounts);
        setJournalTrades(tradesPage.data);
      })
      .catch((error: unknown) => {
        if (active) setJournalError(error instanceof Error ? error.message : "Unable to load Journal portfolio data.");
      })
      .finally(() => { if (active) setJournalLoading(false); });

    return () => { active = false; };
  }, []);

  const currencies = useMemo(() => Array.from(new Set(journalAccounts.map((account) => account.currency.toUpperCase()))), [journalAccounts]);
  const singleCurrency = currencies.length === 1;
  const journalInitial = useMemo(() => journalAccounts.reduce((sum, account) => sum + amount(account.initial_balance), 0), [journalAccounts]);
  const journalRealized = useMemo(() => journalAccounts.reduce((sum, account) => sum + amount(account.realized_net_pnl), 0), [journalAccounts]);
  const journalEquity = useMemo(() => journalAccounts.reduce((sum, account) => sum + amount(account.journal_equity), 0), [journalAccounts]);

  const closedTrades = useMemo(() => journalTrades
    .filter((trade) => trade.status.toLowerCase() === "closed" && trade.closed_at && trade.net_pnl !== null && trade.net_pnl !== undefined)
    .sort((a, b) => new Date(a.closed_at!).getTime() - new Date(b.closed_at!).getTime()), [journalTrades]);

  const journalCurve = useMemo<EquityPoint[]>(() => {
    if (!singleCurrency || journalAccounts.length === 0) return [];
    let equity = journalInitial;
    let peak = equity;
    let maxDrawdown = 0;
    const points: EquityPoint[] = [{
      time: "Starting balance", equity: equity.toFixed(2), realizedPnl: "0", drawdownPct: 0,
    }];
    for (const trade of closedTrades) {
      equity += amount(trade.net_pnl);
      peak = Math.max(peak, equity);
      const drawdownPct = peak > 0 ? ((peak - equity) / peak) * 100 : 0;
      maxDrawdown = Math.max(maxDrawdown, drawdownPct);
      points.push({
        time: new Date(trade.closed_at!).toLocaleDateString(),
        equity: equity.toFixed(2),
        realizedPnl: amount(trade.net_pnl).toFixed(2),
        drawdownPct,
      });
    }
    return points;
  }, [singleCurrency, journalAccounts.length, journalInitial, closedTrades]);

  const journalByInstrument = useMemo(() => {
    const totals = new Map<string, number>();
    for (const trade of closedTrades) totals.set(trade.symbol, (totals.get(trade.symbol) ?? 0) + amount(trade.net_pnl));
    return Array.from(totals, ([instrumentId, pnl]) => ({ instrumentId, pnl: pnl.toFixed(2) }));
  }, [closedTrades]);

  const journalBySetup = useMemo(() => {
    const totals = new Map<string, number>();
    for (const trade of closedTrades) {
      const key = trade.setup_id || "Unclassified";
      totals.set(key, (totals.get(key) ?? 0) + amount(trade.net_pnl));
    }
    return Array.from(totals, ([strategyId, pnl]) => ({ strategyId, pnl: pnl.toFixed(2) }));
  }, [closedTrades]);

  const hasBrokerEquity = brokerPortfolio !== null && amount(brokerPortfolio.totalEquity) > 0;
  const useJournalFallback = !hasBrokerEquity && journalAccounts.length > 0 && singleCurrency;
  const p = brokerPortfolio ?? emptyPortfolio;
  const currency = singleCurrency ? currencies[0] : undefined;
  const totalEquity = useJournalFallback ? journalEquity.toFixed(2) : p.totalEquity;
  const realizedPnl = useJournalFallback ? journalRealized.toFixed(2) : p.realizedPnl;
  const cumulativeReturn = useJournalFallback && journalInitial > 0 ? (journalRealized / journalInitial) * 100 : p.cumulativeReturnPct;
  const drawdown = useJournalFallback && journalCurve.length > 0 ? Math.max(...journalCurve.map((point) => point.drawdownPct)) : p.drawdownPct;
  const equitySeries = useJournalFallback ? journalCurve : p.equitySeries;
  const pnlByStrategy = useJournalFallback ? journalBySetup : p.pnlByStrategy;
  const pnlByInstrument = useJournalFallback ? journalByInstrument : p.pnlByInstrument;
  const isLoading = brokerLoading && journalLoading;

  if (isLoading) return <div className="loading">Loading portfolio…</div>;

  return (
    <>
      <PageHeader
        title="Portfolio"
        description="Account → Positions → Exposure → P&L. Analytical view only — no rebalancing."
      />
      {useJournalFallback ? (
        <p className="metric-sub" role="status">Showing Journal-ledger portfolio metrics because broker-reported equity is unavailable. Linked accounts are counted once; Journal and broker figures are never added together.</p>
      ) : null}
      {!singleCurrency && journalAccounts.length > 0 ? (
        <p className="metric-sub" role="status">Journal accounts use multiple currencies ({currencies.join(", ")}). Their balances are shown separately below and are not combined without an FX conversion source.</p>
      ) : null}
      {brokerError && !useJournalFallback ? <div className="error" role="alert">Trading portfolio unavailable: {brokerError}</div> : null}
      <div className="grid grid-kpi" style={{ marginBottom: 12 }}>
        <Metric label="TOTAL EQUITY" value={useJournalFallback && currency ? formatMoney(totalEquity, currency) : formatMoney(totalEquity)} />
        <Metric label="REALIZED P&L" value={realizedPnl === null ? "Unknown" : useJournalFallback && currency ? formatSignedMoney(realizedPnl) + " " + currency : formatSignedMoney(realizedPnl)} signed />
        <Metric label="UNREALIZED P&L" value={useJournalFallback ? "Unavailable" : formatMoney(p.unrealizedPnl)} signed />
        <div className="metric panel">
          <div className="metric-label">CUMULATIVE RETURN</div>
          <div className={`metric-value ${cumulativeReturn >= 0 ? "pos" : "neg"}`}>{cumulativeReturn.toFixed(2)}%</div>
          <div className="metric-sub">{drawdown.toFixed(2)}% max drawdown</div>
        </div>
      </div>
      <div className="panel" style={{ marginBottom: 12 }}>
        <div className="panel-title">JOURNAL ACCOUNT PERFORMANCE</div>
        <p className="metric-sub">Journal account performance is shown separately from broker-reported equity, including linked accounts. Values use initial balance plus recorded closed-trade net P&L; deposits, withdrawals and unrealized P&L are excluded.</p>
        {journalLoading ? <div className="loading">Loading Journal accounts…</div> : journalError ? <div className="error" role="alert">{journalError}</div> : journalAccounts.length === 0 ? <p className="metric-sub">No Journal accounts found.</p> : (
          <div style={{ overflowX: "auto" }}>
            <table className="data-table">
              <thead><tr><th>Account</th><th>Currency</th><th>Initial balance</th><th>Realized net P&L</th><th>Journal equity</th><th>Linked trading account</th><th>Trades</th></tr></thead>
              <tbody>{journalAccounts.map((account) => (
                <tr key={account.account_id}>
                  <td>{account.name}</td><td>{account.currency}</td>
                  <td className="num">{formatMoney(account.initial_balance, account.currency)}</td>
                  <td className={`num ${amount(account.realized_net_pnl) >= 0 ? "pos" : "neg"}`}>{formatSignedMoney(account.realized_net_pnl)} {account.currency}</td>
                  <td className="num">{formatMoney(account.journal_equity, account.currency)}</td>
                  <td>{account.linked_trading_account_id || "Journal only"}</td><td className="num">{account.trade_count}</td>
                </tr>
              ))}</tbody>
            </table>
          </div>
        )}
      </div>
      <div className="grid grid-dash" style={{ marginBottom: 12 }}>
        <div className="panel">
          <div className="panel-title">EQUITY CURVE</div>
          {equitySeries.length > 0 ? <EquityChart series={equitySeries} /> : <p className="metric-sub">No historical equity series is available yet.</p>}
        </div>
        <div className="panel">
          <div className="panel-title">EXPOSURE</div>
          {useJournalFallback ? <p className="metric-sub">Unavailable — Journal trade records do not provide broker-valued open-position exposure.</p> : <>
            <div className="metric-sub">Gross {formatMoney(p.grossExposure)}</div>
            <div className="metric-sub">Net {formatMoney(p.netExposure)}</div>
            <div className="metric-sub">Exposure {p.exposurePct.toFixed(1)}%</div>
          </>}
          <div className="panel-title" style={{ marginTop: 16 }}>P&L BY STRATEGY / SETUP</div>
          {pnlByStrategy.length ? pnlByStrategy.map((row) => <div key={row.strategyId} className="metric-sub num">{row.strategyId}: {formatSignedMoney(row.pnl)}</div>) : <p className="metric-sub">No strategy/setup P&L data available.</p>}
          <div className="panel-title" style={{ marginTop: 16 }}>P&L BY INSTRUMENT</div>
          {pnlByInstrument.length ? pnlByInstrument.map((row) => <div key={row.instrumentId} className="metric-sub num">{row.instrumentId}: {formatSignedMoney(row.pnl)}</div>) : <p className="metric-sub">No instrument P&L data available.</p>}
        </div>
      </div>
      {journalAccounts.length > 0 && singleCurrency && useJournalFallback ? (
        <p className="metric-sub">Journal portfolio: {formatMoney(journalInitial.toFixed(2), currency!)} starting balance · {formatSignedMoney(journalRealized.toFixed(2))} {currency} realized P&L · {closedTrades.length} closed trades included in the equity curve.</p>
      ) : null}
    </>
  );
}
