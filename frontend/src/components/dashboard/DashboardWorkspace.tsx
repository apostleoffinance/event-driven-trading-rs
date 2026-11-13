"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { journalApi, type JournalAccount, type JournalAnalytics } from "@/lib/api/journal";
import { hasSession } from "@/lib/auth/session";
import { EquityChart } from "@/components/charts/EquityChart";
import { EventStreamList } from "@/components/events/EventStreamList";
import { StatusBadge, orderTone, riskToneClass } from "@/components/data-display/StatusBadge";
import { formatMoney, formatPrice, formatQuantity, formatSignedMoney } from "@/lib/format";
import type { Account, Order, PortfolioSnapshot, Position, RiskOverview, TradingEvent, EquityPoint } from "@/types/domain";

type Period = "7d" | "30d" | "90d" | "ytd" | "all";
type WidgetKey = "performance" | "risk" | "accounts" | "positions" | "activity" | "orders";
type SavedLayout = { id: string; name: string; accountId: string; period: Period; visible: WidgetKey[] };
const WIDGETS: { key: WidgetKey; label: string }[] = [
  { key: "performance", label: "Performance" }, { key: "risk", label: "Risk overview" },
  { key: "accounts", label: "Journal accounts" }, { key: "positions", label: "Open positions" },
  { key: "activity", label: "Recent activity" }, { key: "orders", label: "Recent orders" },
];
const DEFAULT_VISIBLE: WidgetKey[] = ["performance", "risk", "accounts", "positions", "activity", "orders"];
const LAYOUTS_KEY = "edgenest.dashboard.layouts.v1";

function dateRange(period: Period) {
  const to = new Date();
  const from = new Date(to);
  if (period === "7d") from.setDate(from.getDate() - 7);
  else if (period === "30d") from.setDate(from.getDate() - 30);
  else if (period === "90d") from.setDate(from.getDate() - 90);
  else if (period === "ytd") { from.setMonth(0, 1); from.setHours(0, 0, 0, 0); }
  else return {};
  return { from: from.toISOString().slice(0, 10), to: to.toISOString().slice(0, 10) };
}
function money(value: string | number | null | undefined, currency = "USD") {
  if (value == null || !Number.isFinite(Number(value))) return "—";
  try { return new Intl.NumberFormat(undefined, { style: "currency", currency, maximumFractionDigits: 2 }).format(Number(value)); }
  catch { return Number(value).toFixed(2) + " " + currency; }
}
function finite(value: string | number | null | undefined) { const n = Number(value); return Number.isFinite(n) ? n : 0; }
function accountSummaryLabel(account: JournalAccount) { return [account.broker || "Journal", account.account_type, account.currency].filter(Boolean).join(" · "); }

export function DashboardWorkspace({
  executionAccounts, portfolio, positions, orders, risk, events, loading, error,
}: {
  executionAccounts: Account[]; portfolio: PortfolioSnapshot | null; positions: Position[];
  orders: Order[]; risk: RiskOverview | null; events: TradingEvent[]; loading: boolean; error: string | null;
}) {
  const [authenticated, setAuthenticated] = useState(false);
  const [selectedAccount, setSelectedAccount] = useState("all");
  const [period, setPeriod] = useState<Period>("30d");
  const [visible, setVisible] = useState<WidgetKey[]>(DEFAULT_VISIBLE);
  const [customizing, setCustomizing] = useState(false);
  const [layouts, setLayouts] = useState<SavedLayout[]>([]);
  const [layoutName, setLayoutName] = useState("");
  const [selectedLayout, setSelectedLayout] = useState("");
  const [layoutMessage, setLayoutMessage] = useState("");

  useEffect(() => { setAuthenticated(hasSession()); }, []);
  useEffect(() => {
    try { const raw = window.localStorage.getItem(LAYOUTS_KEY); if (raw) setLayouts(JSON.parse(raw) as SavedLayout[]); } catch { /* ignore invalid local layout data */ }
  }, []);

  const accountsQuery = useQuery({
    queryKey: ["journal", "accounts"], queryFn: journalApi.listAccounts, enabled: authenticated,
    staleTime: 30_000, refetchOnWindowFocus: true, retry: 1,
  });
  const journalAccounts = useMemo(() => (accountsQuery.data ?? []).filter(a => !a.archived), [accountsQuery.data]);
  const range = useMemo(() => dateRange(period), [period]);
  const activeAccounts = useMemo(() => selectedAccount === "all" ? journalAccounts : journalAccounts.filter(a => a.id === selectedAccount), [journalAccounts, selectedAccount]);
  const analyticsQueries = useQueries({ queries: activeAccounts.map(account => ({
    queryKey: ["journal", "analytics", account.id, "dashboard", period],
    queryFn: () => journalApi.analyticsSummary({ journal_account_id: account.id, ...range }),
    enabled: authenticated, staleTime: 20_000, refetchOnWindowFocus: true, retry: 1,
  })) });
  const ready = analyticsQueries.filter(q => q.data).map(q => q.data as JournalAnalytics);
  const currencies = [...new Set(activeAccounts.map(a => a.currency))];
  const commonCurrency = currencies.length === 1 ? currencies[0] : null;
  const totals = useMemo(() => {
    const sum = (key: "trade_count" | "wins" | "losses" | "breakeven") => ready.reduce((n, x) => n + x[key], 0);
    const grossProfit = ready.reduce((n, x) => n + finite(x.gross_profit), 0);
    const grossLoss = ready.reduce((n, x) => n + Math.abs(finite(x.gross_loss)), 0);
    const netProfit = ready.reduce((n, x) => n + finite(x.net_profit), 0);
    const trades = sum("trade_count"), wins = sum("wins");
    return { trades, wins, losses: sum("losses"), breakeven: sum("breakeven"),
      winRate: trades ? wins / trades * 100 : null,
      netProfit: commonCurrency ? netProfit : null,
      profitFactor: commonCurrency ? (grossLoss > 0 ? grossProfit / grossLoss : grossProfit > 0 ? null : null) : null,
      grossProfit, grossLoss,
      maxDrawdown: commonCurrency ? Math.max(0, ...ready.map(x => Math.abs(finite(x.max_drawdown)))) : null };
  }, [ready, commonCurrency]);
  const selectedAccountData = activeAccounts.length === 1 ? ready[0] : null;
  const equitySeries: EquityPoint[] = (selectedAccountData?.equity_curve ?? []).map(point => ({
    time: point.closed_at, equity: point.equity, realizedPnl: point.net_pnl, drawdownPct: Math.abs(finite(point.drawdown)),
  }));
  const isVisible = (key: WidgetKey) => visible.includes(key);
  const toggleWidget = (key: WidgetKey) => setVisible(current => current.includes(key) ? current.filter(x => x !== key) : [...current, key]);
  const persistLayouts = (next: SavedLayout[]) => { setLayouts(next); try { window.localStorage.setItem(LAYOUTS_KEY, JSON.stringify(next)); } catch { setLayoutMessage("Could not save layouts in this browser."); } };
  const saveLayout = () => {
    const name = layoutName.trim();
    if (!name) { setLayoutMessage("Enter a name for this layout."); return; }
    const item: SavedLayout = { id: crypto.randomUUID(), name, accountId: selectedAccount, period, visible };
    persistLayouts([...layouts, item]); setLayoutName(""); setSelectedLayout(item.id); setLayoutMessage("Layout saved on this device.");
  };
  const loadLayout = (id: string) => {
    const item = layouts.find(x => x.id === id); if (!item) return;
    setSelectedLayout(id); setSelectedAccount(item.accountId); setPeriod(item.period); setVisible(item.visible); setLayoutMessage("Layout loaded.");
  };
  const deleteLayout = () => { if (!selectedLayout) return; persistLayouts(layouts.filter(x => x.id !== selectedLayout)); setSelectedLayout(""); setLayoutMessage("Layout deleted."); };
  const executionConnected = executionAccounts.length > 0;
  const dailyUtil = risk?.dailyLossLimitPct && risk.dailyLossLimitPct > 0 ? Math.min(100, Math.max(0, risk.dailyLossPct / risk.dailyLossLimitPct * 100)) : null;
  const drawdownUtil = risk?.maxDrawdownLimitPct && risk.maxDrawdownLimitPct > 0 ? Math.min(100, Math.max(0, risk.maxDrawdownPct / risk.maxDrawdownLimitPct * 100)) : null;

  if (loading) return <div className="dashboard-loading"><div className="dashboard-skeleton dashboard-skeleton-title"/><div className="dashboard-skeleton-grid">{[1,2,3,4].map(x=><div className="dashboard-skeleton" key={x}/>)}</div><div className="dashboard-skeleton dashboard-skeleton-chart"/></div>;

  return <div className="dashboard-workspace">
    <header className="dashboard-heading">
      <div><div className="dashboard-eyebrow">COMMAND CENTER</div><h1>Your trading dashboard</h1><p>Performance, risk and account activity — in one view.</p></div>
      <div className="dashboard-toolbar">
        <label className="dashboard-filter"><span>Account</span><select value={selectedAccount} onChange={e=>{setSelectedAccount(e.target.value);setSelectedLayout("");}}><option value="all">All Journal accounts</option>{journalAccounts.map(a=><option key={a.id} value={a.id}>{a.name}</option>)}</select></label>
        <label className="dashboard-filter"><span>Period</span><select value={period} onChange={e=>{setPeriod(e.target.value as Period);setSelectedLayout("");}}><option value="7d">Last 7 days</option><option value="30d">Last 30 days</option><option value="90d">Last 90 days</option><option value="ytd">Year to date</option><option value="all">All time</option></select></label>
        <button className="btn" type="button" onClick={()=>setCustomizing(v=>!v)} aria-expanded={customizing}>{customizing ? "Done customizing" : "Customize"}</button>
      </div>
    </header>

    {customizing ? <section className="panel dashboard-customizer">
      <div><h2>Personalize your dashboard</h2><p className="muted">Choose the widgets you want to see and save this arrangement as a reusable layout on this device.</p></div>
      <div className="dashboard-widget-toggles">{WIDGETS.map(w=><label key={w.key}><input type="checkbox" checked={isVisible(w.key)} onChange={()=>toggleWidget(w.key)}/>{w.label}</label>)}</div>
      <div className="dashboard-layout-actions"><input aria-label="Layout name" placeholder="Name this layout" value={layoutName} onChange={e=>setLayoutName(e.target.value)} onKeyDown={e=>{if(e.key==="Enter")saveLayout();}}/><button className="btn primary" type="button" onClick={saveLayout}>Save layout</button><select aria-label="Saved layouts" value={selectedLayout} onChange={e=>loadLayout(e.target.value)}><option value="">Load saved layout…</option>{layouts.map(l=><option key={l.id} value={l.id}>{l.name}</option>)}</select><button className="btn" type="button" disabled={!selectedLayout} onClick={deleteLayout}>Delete</button></div>
      {layoutMessage?<p className="dashboard-inline-message" role="status">{layoutMessage}</p>:null}
    </section> : null}

    {error ? <section className="panel dashboard-alert" role="alert"><strong>Execution data unavailable</strong><p>{error}</p></section> : null}
    {accountsQuery.error ? <section className="panel dashboard-alert" role="alert"><strong>Journal accounts could not be loaded.</strong><button className="btn" onClick={()=>void accountsQuery.refetch()}>Retry</button></section> : null}

    <section className="dashboard-kpis" aria-label="Journal performance summary">
      <article className="panel dashboard-kpi"><span>JOURNAL NET P&amp;L</span><strong className={totals.netProfit == null ? "" : totals.netProfit >= 0 ? "pos" : "neg"}>{totals.netProfit == null ? (currencies.length > 1 ? "Mixed currencies" : "—") : money(totals.netProfit, commonCurrency || "USD")}</strong><small>{commonCurrency ? commonCurrency + " · realized" : "Select one currency to view monetary totals"}</small></article>
      <article className="panel dashboard-kpi"><span>CLOSED TRADES</span><strong>{totals.trades.toLocaleString()}</strong><small>{totals.wins} wins · {totals.losses} losses · {totals.breakeven} breakeven</small></article>
      <article className="panel dashboard-kpi"><span>WIN RATE</span><strong>{totals.winRate == null ? "—" : totals.winRate.toFixed(1) + "%"}</strong><small>{totals.trades ? "Across selected Journal accounts" : "No closed trades in this period"}</small></article>
      <article className="panel dashboard-kpi"><span>PROFIT FACTOR</span><strong>{totals.profitFactor == null ? "—" : totals.profitFactor.toFixed(2)}</strong><small>{totals.profitFactor == null ? "Insufficient comparable P&amp;L data" : "Gross profit ÷ gross loss"}</small></article>
      <article className="panel dashboard-kpi"><span>MAX DRAWDOWN</span><strong>{totals.maxDrawdown == null ? "—" : money(totals.maxDrawdown, commonCurrency || "USD")}</strong><small>{totals.maxDrawdown == null ? "Unavailable across mixed currencies" : "Largest reported account drawdown"}</small></article>
    </section>

    {isVisible("performance") ? <section className="dashboard-main-grid">
      <article className="panel dashboard-panel dashboard-performance">
        <div className="dashboard-panel-heading"><div><h2>Performance overview</h2><p>{selectedAccountData ? activeAccounts[0]?.name + " · " + (range.from ? range.from + " to " + range.to : "All time") : "Select a single account to inspect its equity curve."}</p></div><Link href="/analytics" className="dashboard-text-link">Open Analytics Lab ↗</Link></div>
        {analyticsQueries.some(q=>q.isLoading) ? <div className="dashboard-chart-empty">Calculating Journal performance…</div> :
          selectedAccountData && equitySeries.length ? <EquityChart series={equitySeries}/> :
          <div className="dashboard-chart-empty"><strong>{activeAccounts.length === 0 ? "No Journal accounts yet" : activeAccounts.length > 1 ? "Account comparison" : "No equity history for this period"}</strong><span>{activeAccounts.length > 1 ? "Choose an account above to view its equity curve. Account currencies are kept separate." : "Record or import closed trades to build your performance history."}</span>{activeAccounts.length === 0 ? <Link href="/journal" className="btn primary">Create Journal account</Link> : null}</div>}
      </article>
      {isVisible("risk") ? <article className="panel dashboard-panel dashboard-risk">
        <div className="dashboard-panel-heading"><div><h2>Risk overview</h2><p>Execution risk controls and account health</p></div><Link href="/risk" className="dashboard-text-link">Risk Center ↗</Link></div>
        {executionConnected && risk ? <>
          <div className="dashboard-risk-status"><StatusBadge tone={riskToneClass(risk.tone)}>{risk.killSwitch ? "Trading halted" : risk.tone === "BREACH" ? "Limit breached" : risk.engineStatus === "HEALTHY" ? "Risk controls healthy" : risk.engineStatus === "NOT_CONFIGURED" ? "Limits not configured" : "Status unavailable"}</StatusBadge></div>
          <div className="dashboard-risk-row"><div><span>Daily loss utilisation</span><strong>{dailyUtil == null ? "—" : dailyUtil.toFixed(1)+"%"}</strong></div><div className="dashboard-meter"><i className={dailyUtil != null && dailyUtil >= 80 ? "danger" : dailyUtil != null && dailyUtil >= 50 ? "warning" : ""} style={{width: dailyUtil == null ? "0%" : dailyUtil+"%"}}/></div><small>{risk.dailyLossPct.toFixed(2)}% used · limit {risk.dailyLossLimitPct == null ? "not configured" : risk.dailyLossLimitPct.toFixed(2)+"%"}</small></div>
          <div className="dashboard-risk-row"><div><span>Max drawdown utilisation</span><strong>{drawdownUtil == null ? "—" : drawdownUtil.toFixed(1)+"%"}</strong></div><div className="dashboard-meter"><i style={{width: drawdownUtil == null ? "0%" : drawdownUtil+"%"}}/></div><small>{risk.maxDrawdownPct.toFixed(2)}% used · limit {risk.maxDrawdownLimitPct == null ? "not configured" : risk.maxDrawdownLimitPct.toFixed(2)+"%"}</small></div>
          <div className="dashboard-risk-facts"><div><span>Open risk</span><strong>{risk.openRiskPct == null ? "Unavailable" : risk.openRiskPct.toFixed(2)+"%"}</strong></div><div><span>Gross exposure</span><strong>{risk.exposurePct == null ? "Unavailable" : risk.exposurePct.toFixed(1)+"%"}</strong></div><div><span>Kill switch</span><strong>{risk.killSwitch ? "ON" : "OFF"}</strong></div></div>
        </> : <div className="dashboard-chart-empty dashboard-risk-empty"><strong>No execution account connected</strong><span>Journal performance remains available above. Connect a broker account to populate live exposure, open risk and limit utilisation.</span><Link href="/accounts" className="btn primary">Connect account</Link></div>}
      </article> : null}
    </section> : null}

    {isVisible("accounts") ? <section className="panel dashboard-panel dashboard-accounts">
      <div className="dashboard-panel-heading"><div><h2>Journal accounts</h2><p>Recorded and imported trading history — separate from broker-reported balances and equity.</p></div><div className="dashboard-link-group"><Link href="/journal" className="dashboard-text-link">Open Journal ↗</Link><Link href="/analytics" className="dashboard-text-link">Analytics Lab ↗</Link></div></div>
      {accountsQuery.isLoading ? <p className="muted">Loading accounts…</p> : journalAccounts.length ? <div className="dashboard-account-grid">{journalAccounts.map(account=>{
        const index=activeAccounts.findIndex(a=>a.id===account.id); const query=index>=0?analyticsQueries[index]:undefined; const stats=query?.data;
        return <button type="button" className={"dashboard-account-card"+(selectedAccount===account.id?" selected":"")} key={account.id} onClick={()=>setSelectedAccount(selectedAccount===account.id?"all":account.id)}>
          <div className="dashboard-account-top"><div><strong>{account.name}</strong><span>{accountSummaryLabel(account)}</span></div><StatusBadge tone={query?.isError?"warning":query?.isLoading||!stats?"neutral":"success"}>{query?.isError?"Unavailable":query?.isLoading||!stats?"Loading":"Journal only"}</StatusBadge></div>
          <div className="dashboard-account-metrics"><div><small>Closed trades</small><strong>{stats?.trade_count ?? "—"}</strong></div><div><small>Net P&amp;L</small><strong className={stats ? finite(stats.net_profit)>=0?"pos":"neg":""}>{stats?money(stats.net_profit,account.currency):"—"}</strong></div><div><small>Win rate</small><strong>{stats?.win_rate_percent == null?"—":Number(stats.win_rate_percent).toFixed(1)+"%"}</strong></div></div>
          <span className="dashboard-account-footer">{selectedAccount===account.id?"Selected · click to clear":"View account performance →"}</span>
        </button>;
      })}</div> : <div className="dashboard-empty-inline"><span>No Journal accounts yet.</span><Link href="/journal" className="btn primary">Create or import history</Link></div>}
    </section> : null}

    {isVisible("positions") || isVisible("activity") ? <section className="dashboard-bottom-grid">
      {isVisible("positions") ? <article className="panel dashboard-panel"><div className="dashboard-panel-heading"><div><h2>Open positions</h2><p>Live execution positions</p></div><Link href="/positions" className="dashboard-text-link">View all ↗</Link></div>
        {executionConnected ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Instrument</th><th>Side</th><th>Quantity</th><th>Entry</th><th>Mark</th><th>Unrealized P&amp;L</th></tr></thead><tbody>{positions.length?positions.slice(0,8).map(p=><tr key={p.id}><td>{p.instrumentId}</td><td>{p.side}</td><td className="num">{formatQuantity(p.quantity)}</td><td className="num">{formatPrice(p.entryPrice)}</td><td className="num">{formatPrice(p.markPrice)}</td><td className={"num "+(finite(p.unrealizedPnl)>=0?"pos":"neg")}>{formatSignedMoney(p.unrealizedPnl)}</td></tr>):<tr><td colSpan={6} className="muted">No open execution positions.</td></tr>}</tbody></table></div> : <p className="muted">Connect an execution account to see live positions. Journal trades are available in your trade history.</p>}
      </article> : null}
      {isVisible("activity") ? <article className="panel dashboard-panel"><div className="dashboard-panel-heading"><div><h2>Recent activity</h2><p>Latest platform events</p></div><Link href="/events" className="dashboard-text-link">Activity log ↗</Link></div><EventStreamList events={events} limit={7}/></article> : null}
    </section> : null}

    {isVisible("orders") ? <section className="panel dashboard-panel dashboard-orders"><div className="dashboard-panel-heading"><div><h2>Recent orders</h2><p>Execution activity from connected accounts</p></div><Link href="/orders" className="dashboard-text-link">View all ↗</Link></div>
      {executionConnected ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Updated</th><th>Instrument</th><th>Side</th><th>Quantity</th><th>Status</th><th>Price</th><th>Strategy</th></tr></thead><tbody>{orders.length?orders.slice(0,8).map(o=><tr key={o.id}><td className="num">{new Date(o.updatedAt).toLocaleString()}</td><td>{o.instrumentId}</td><td>{o.side}</td><td className="num">{formatQuantity(o.quantity)}</td><td><StatusBadge tone={orderTone(o.status)}>{o.status}</StatusBadge></td><td className="num">{o.price??"—"}</td><td>{o.strategyId||"—"}</td></tr>):<tr><td colSpan={7} className="muted">No execution orders recorded yet.</td></tr>}</tbody></table></div> : <div className="dashboard-empty-inline"><span>No execution account connected. Orders will appear here after a broker connection is configured.</span><Link href="/accounts" className="btn">Connect account</Link></div>}
    </section> : null}
    <footer className="dashboard-footer"><span>Journal figures are calculated from recorded closed trades. Live account balances, equity and risk require an execution connection.</span><Link href="/connections">Connection status ↗</Link></footer>
  </div>;
}
