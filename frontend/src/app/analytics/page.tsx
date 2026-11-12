"use client";

import "./analytics.css";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { journalApi, type JournalAnalytics } from "@/lib/api/journal";
import { ApiError } from "@/lib/api/client";
import { getSessionUser, hasSession } from "@/lib/auth/session";
import { AnalyticsEquityChart } from "@/components/charts/AnalyticsEquityChart";
import { AnalyticsDrawdownChart } from "@/components/charts/AnalyticsDrawdownChart";
import { AnalyticsBreakdownChart } from "@/components/charts/AnalyticsBreakdownChart";
import { ExecutionQualityChart } from "@/components/charts/ExecutionQualityChart";
import { CalendarAnalytics } from "@/components/analytics/CalendarAnalytics";
import { BehaviourDashboard } from "@/components/analytics/BehaviourDashboard";
import { HourlyPerformanceHeatmap } from "@/components/analytics/HourlyPerformanceHeatmap";
import { AnalyticsInsights } from "@/components/analytics/AnalyticsInsights";

function money(value: string | number | null | undefined, currency = "USD") {
  if (value == null) return "—";
  const n = Number(value);
  return Number.isFinite(n) ? n.toLocaleString("en-US", { style: "currency", currency, maximumFractionDigits: 2 }) : "—";
}
function percent(value: string | null | undefined) {
  if (value == null) return "—";
  const n = Number(value);
  return Number.isFinite(n) ? n.toFixed(1) + "%" : "—";
}

export default function AnalyticsLabPage() {
  const [authenticated, setAuthenticated] = useState(false);
  const [accountId, setAccountId] = useState("");
  const [symbolFilter, setSymbolFilter] = useState("");
  const [setupFilter, setSetupFilter] = useState("");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");
  const [datePreset, setDatePreset] = useState("all");
  const [filtersExpanded, setFiltersExpanded] = useState(true);
  const [filtersRestored, setFiltersRestored] = useState(false);
  const [savedViews, setSavedViews] = useState<Array<{name:string;accountId:string;fromDate:string;toDate:string;datePreset:string;symbolFilter:string;setupFilter:string}>>([]);
  const [viewName, setViewName] = useState("");
  const [selectedGroup, setSelectedGroup] = useState<{kind:"session"|"weekday"|"hour";key:string}|null>(null);
  const [selectedTrade, setSelectedTrade] = useState<NonNullable<JournalAnalytics["trade_metrics"]>[number] | null>(null);
  const [userEmail, setUserEmail] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"overview" | "performance" | "edge" | "execution" | "behaviour" | "risk" | "calendar">("overview");
  useEffect(() => {
    setAuthenticated(hasSession());
    setUserEmail(getSessionUser()?.email ?? null);
    try {
      const raw = window.localStorage.getItem("quantos.analytics.filters.v1");
      if (raw) {
        const saved = JSON.parse(raw) as {accountId?:string;fromDate?:string;toDate?:string;datePreset?:string;symbolFilter?:string;setupFilter?:string};
        if (saved.accountId) setAccountId(saved.accountId);
        if (saved.fromDate) setFromDate(saved.fromDate);
        if (saved.toDate) setToDate(saved.toDate);
        if (saved.datePreset) setDatePreset(saved.datePreset);
        if (saved.symbolFilter) setSymbolFilter(saved.symbolFilter);
        if (saved.setupFilter) setSetupFilter(saved.setupFilter);
      }
      const views = window.localStorage.getItem("quantos.analytics.saved-views.v1");
      if (views) setSavedViews(JSON.parse(views));
    } catch { /* Ignore malformed local preferences and use safe defaults. */ }
    setFiltersRestored(true);
  }, []);
  useEffect(() => {
    if (!filtersRestored) return;
    window.localStorage.setItem("quantos.analytics.filters.v1", JSON.stringify({accountId,fromDate,toDate,datePreset,symbolFilter,setupFilter}));
  }, [filtersRestored,accountId,fromDate,toDate,datePreset,symbolFilter,setupFilter]);
  useEffect(() => {
    if (filtersRestored) window.localStorage.setItem("quantos.analytics.saved-views.v1", JSON.stringify(savedViews));
  }, [filtersRestored,savedViews]);
  useEffect(() => {
    const allowed = ["overview", "performance", "edge", "execution", "behaviour", "risk", "calendar"];
    const syncTabFromUrl = () => {
      const candidate = window.location.hash.slice(1);
      if (allowed.includes(candidate)) setActiveTab(candidate as typeof activeTab);
    };
    syncTabFromUrl();
    window.addEventListener("hashchange", syncTabFromUrl);
    return () => window.removeEventListener("hashchange", syncTabFromUrl);
  }, []);

  const accounts = useQuery({
    queryKey: ["journal", "accounts"],
    queryFn: journalApi.listAccounts,
    enabled: authenticated,
    staleTime: 30_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });
  const accountCurrencies = [...new Set((accounts.data ?? []).filter(x => !x.archived).map(x => x.currency.toUpperCase()))];
  const mixedCurrencies = !accountId && accountCurrencies.length > 1;
  const selectedAccount = accounts.data?.find(x => x.id === accountId);
  const displayCurrency = selectedAccount?.currency ?? accountCurrencies[0] ?? "USD"; 
  const dateKey = (date: Date) => date.toISOString().slice(0, 10);
  const applyDatePreset = (preset: string) => {
    setDatePreset(preset);
    const now = new Date();
    const end = dateKey(now);
    const startOfMonth = dateKey(new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1)));
    let start = "";
    if (preset === "today") start = end;
    else if (preset === "yesterday") { const d = new Date(now); d.setUTCDate(d.getUTCDate()-1); start = dateKey(d); setToDate(dateKey(d)); }
    else if (preset === "7d") { const d = new Date(now); d.setUTCDate(d.getUTCDate()-6); start = dateKey(d); }
    else if (preset === "30d") { const d = new Date(now); d.setUTCDate(d.getUTCDate()-29); start = dateKey(d); }
    else if (preset === "90d") { const d = new Date(now); d.setUTCDate(d.getUTCDate()-89); start = dateKey(d); }
    else if (preset === "month") start = startOfMonth;
    else if (preset === "lastmonth") { const d = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth()-1, 1)); start = dateKey(d); setToDate(dateKey(new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 0)))); }
    else if (preset === "ytd") start = dateKey(new Date(Date.UTC(now.getUTCFullYear(),0,1)));
    if (preset === "all") { setFromDate(""); setToDate(""); return; }
    setFromDate(start);
    if (preset !== "yesterday" && preset !== "lastmonth") setToDate(end);
  };
  const dateFilter = { from: fromDate ? fromDate + "T00:00:00.000Z" : undefined, to: toDate ? toDate + "T23:59:59.999Z" : undefined };
  const setups = useQuery({
    queryKey: ["journal", "setups"],
    queryFn: journalApi.listSetups,
    enabled: authenticated && accounts.isSuccess,
    staleTime: 30_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });
  const analyticsOptions = useQuery({
    queryKey: ["journal", "analytics-filter-options", accountId, fromDate, toDate],
    queryFn: () => journalApi.analyticsSummary({ journal_account_id: accountId || undefined, ...dateFilter }),
    enabled: authenticated && accounts.isSuccess && !mixedCurrencies,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });
  const analytics = useQuery({
    queryKey: ["journal", "analytics", accountId, fromDate, toDate, symbolFilter, setupFilter],
    queryFn: () => journalApi.analyticsSummary({ journal_account_id: accountId || undefined, ...dateFilter, symbol: symbolFilter || undefined, setup_id: setupFilter || undefined }),
    enabled: authenticated && accounts.isSuccess && !mixedCurrencies,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });

  const recentTrades = useQuery({
    queryKey: ["journal", "recent-closed-trades", accountId, fromDate, toDate, symbolFilter, setupFilter],
    queryFn: () => journalApi.listTrades({ journal_account_id: accountId || undefined, limit: 10, offset: 0, closed_from: dateFilter.from, closed_to: dateFilter.to, symbol: symbolFilter || undefined, setup_id: setupFilter || undefined }),
    enabled: authenticated && accounts.isSuccess && !mixedCurrencies,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });

  const intelligence = useQuery({
    queryKey: ["journal", "analytics-intelligence", accountId, fromDate, toDate, symbolFilter, setupFilter],
    queryFn: () => journalApi.analyticsIntelligence({ journal_account_id: accountId || undefined, ...dateFilter, symbol: symbolFilter || undefined, setup_id: setupFilter || undefined }),
    enabled: authenticated && accounts.isSuccess && !mixedCurrencies,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });

  if (!authenticated) return <div>
    <header className="page-header"><div><h1>Analytics Lab</h1><p>Performance analytics based on your Journal records.</p></div><span className="badge warn">Sign-in required</span></header>
    <section className="panel"><h2>Authentication required</h2><p>Sign in to see your private Journal analytics.</p><Link className="btn" href="/login">Sign in</Link></section>
  </div>;

  const a = analytics.data;
  const weekdayOrder = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
  const weekdaysOrdered = (a?.by_weekday ?? []).slice().sort((x,y) => weekdayOrder.indexOf(x.key)-weekdayOrder.indexOf(y.key));
  const investigatedTrades = (a?.trade_metrics ?? []).filter(trade => {
    if (!selectedGroup) return false;
    if (selectedGroup.kind === "session") return trade.session === selectedGroup.key;
    if (selectedGroup.kind === "hour") return new Date(trade.closed_at).toISOString().slice(11,13) + ":00" === selectedGroup.key;
    return new Date(trade.closed_at).toLocaleDateString("en-US",{weekday:"long",timeZone:"UTC"}) === selectedGroup.key;
  });
  const metrics: [string, string][] = [
    ["Closed trades", String(a?.trade_count ?? "—")],
    ["Win rate", percent(a?.win_rate_percent)],
    ["Net P&L", money(a?.net_profit, displayCurrency)],
    ["Expectancy / trade", money(a?.expectancy, displayCurrency)],
    ["Profit factor", a?.profit_factor == null ? "—" : Number(a.profit_factor).toFixed(2)],
    ["Max drawdown", money(a?.max_drawdown, displayCurrency)],
    ["Average winner", money(a?.average_win, displayCurrency)],
    ["Average loser", money(a?.average_loss, displayCurrency)],
    ["Average R", a?.average_r == null ? "—" : `${Number(a.average_r).toFixed(2)}R`],
    ["Avg. MFE", a?.average_mfe_r == null ? "—" : `${Number(a.average_mfe_r).toFixed(2)}R`],
    ["Avg. MAE", a?.average_mae_r == null ? "—" : `${Number(a.average_mae_r).toFixed(2)}R`],
    ["Rules followed", percent(a?.rules_followed_percent)],
    ["Valid setup rate", percent(a?.setup_valid_percent)],
    ["Avg. discipline", a?.average_discipline_score == null ? "—" : Number(a.average_discipline_score).toFixed(0) + "/100"],
    ["Emotional trades", String(a?.emotional_trade_count ?? "—")],
    ["Mistakes", String(a?.mistake_count ?? "—")],
    ["Gross profit", money(a?.gross_profit, displayCurrency)],
    ["Gross loss", money(a?.gross_loss, displayCurrency)],
    ["Breakeven", String(a?.breakeven ?? "—")],
  ];
  return <div className="analytics-page">
    <header className="page-header analytics-page-header"><div><span className="analytics-eyebrow">TRADING INTELLIGENCE WORKSPACE</span><h1>Analytics Lab</h1><p>Understand your trading edge across profitability, execution, behaviour and risk. {userEmail ? `Signed in as ${userEmail}.` : ""}</p></div><div style={{display:"flex",gap:10,alignItems:"center",flexWrap:"wrap"}}><Link className="btn" href="/market-data">Open TradingView Market Analysis</Link><span className="badge ok">Journal-connected · {accounts.data?.length ?? 0} accounts</span></div></header>
    <section className="panel analytics-filter-panel">
      <div className="analytics-filter-header"><div><span className="analytics-eyebrow">ANALYSIS CONTROLS</span><h2 className="analytics-filter-title">Filters</h2><p className="muted">All workspaces use the same account, date, instrument and setup filters.</p></div><div className="analytics-filter-actions"><span className="analytics-filter-count">{a?.trade_count ?? 0} qualifying trades</span><button className="btn" type="button" onClick={()=>setFiltersExpanded(value=>!value)} aria-expanded={filtersExpanded}>{filtersExpanded?"Collapse filters":"Expand filters"}</button><button className="btn" type="button" onClick={()=>{setAccountId("");setFromDate("");setToDate("");setDatePreset("all");setSymbolFilter("");setSetupFilter("");}}>Reset</button></div></div>
      {filtersExpanded ? <div className="analytics-filter-grid">
        <label className="analytics-filter-field"><span>Journal account</span><select value={accountId} onChange={e=>setAccountId(e.target.value)}><option value="">All accounts</option>{accounts.data?.filter(x=>!x.archived).map(x=><option key={x.id} value={x.id}>{x.name} · {x.currency}</option>)}</select></label>
        <label className="analytics-filter-field"><span>Date range</span><select value={datePreset} onChange={e=>applyDatePreset(e.target.value)}><option value="all">All time</option><option value="today">Today</option><option value="yesterday">Yesterday</option><option value="7d">Last 7 days</option><option value="30d">Last 30 days</option><option value="90d">Last 90 days</option><option value="month">This month</option><option value="lastmonth">Last month</option><option value="ytd">Year to date</option><option value="custom">Custom range</option></select></label>
        {datePreset === "custom" ? <><label className="analytics-filter-field"><span>From (UTC)</span><input type="date" value={fromDate} onChange={e=>setFromDate(e.target.value)} /></label><label className="analytics-filter-field"><span>To (UTC)</span><input type="date" min={fromDate || undefined} value={toDate} onChange={e=>setToDate(e.target.value)} /></label></> : null}
        <label className="analytics-filter-field"><span>Instrument</span><select value={symbolFilter} onChange={e=>setSymbolFilter(e.target.value)}><option value="">All instruments</option>{analyticsOptions.data?.by_symbol.map(x=><option key={x.key} value={x.key}>{x.key}</option>)}</select></label>
        <label className="analytics-filter-field"><span>Setup</span><select value={setupFilter} onChange={e=>setSetupFilter(e.target.value)}><option value="">All setups</option>{setups.data?.filter(x=>!x.archived).map(x=><option key={x.id} value={x.id}>{x.name}</option>)}</select></label>
      </div> : null}
      <div className="analytics-filter-grid" style={{marginTop:12}}>
        <label className="analytics-filter-field"><span>Saved views</span><select value="" onChange={e=>{const view=savedViews.find(v=>v.name===e.target.value);if(view){setAccountId(view.accountId);setFromDate(view.fromDate);setToDate(view.toDate);setDatePreset(view.datePreset);setSymbolFilter(view.symbolFilter);setSetupFilter(view.setupFilter);}}}><option value="">Load a saved view…</option>{savedViews.map(view=><option key={view.name} value={view.name}>{view.name}</option>)}</select></label>
        <label className="analytics-filter-field"><span>Save current filters</span><input value={viewName} onChange={e=>setViewName(e.target.value)} placeholder="Name this view"/></label>
        <div className="analytics-filter-field" style={{justifyContent:"end"}}><span>&nbsp;</span><button className="btn" type="button" disabled={!viewName.trim()} onClick={()=>{const name=viewName.trim();setSavedViews(current=>[...current.filter(v=>v.name!==name),{name,accountId,fromDate,toDate,datePreset,symbolFilter,setupFilter}]);setViewName("");}}>Save view</button></div>
      </div>
      <div className="analytics-active-filters">{accountId ? <button type="button" onClick={()=>setAccountId("")}>Account: {selectedAccount?.name ?? "Selected"} ×</button> : null}{fromDate || toDate ? <button type="button" onClick={()=>{setFromDate("");setToDate("");setDatePreset("all");}}>Date: {fromDate || "Start"} – {toDate || "Today"} ×</button> : null}{symbolFilter ? <button type="button" onClick={()=>setSymbolFilter("")}>Instrument: {symbolFilter} ×</button> : null}{setupFilter ? <button type="button" onClick={()=>setSetupFilter("")}>Setup: {setups.data?.find(x=>x.id===setupFilter)?.name ?? "Selected"} ×</button> : null}<span>Reporting timezone: UTC · Currency: {displayCurrency}</span></div>
      {accounts.isLoading ? <p className="muted analytics-inline-status">Loading your Journal accounts…</p> : null}
      {accounts.error ? <div className="error analytics-inline-status" role="alert"><p>Could not load your Journal accounts. Check your connection and try again.</p><button className="btn" onClick={()=>void accounts.refetch()}>Retry accounts</button></div> : null}
      {mixedCurrencies ? <div className="error analytics-inline-status" role="status"><strong>Select a single account to continue.</strong><p>These accounts use different currencies ({accountCurrencies.join(", ")}). EdgeNest will not combine unlike currencies into misleading P&amp;L totals. Choose an account above to see its analytics.</p></div> : null}
      {analytics.isLoading && !mixedCurrencies ? <p className="muted analytics-inline-status">Calculating your Journal analytics…</p> : null}
      {analytics.error ? <div className="error analytics-inline-status" role="alert"><p>{analytics.error instanceof ApiError && analytics.error.code === "mixed_currency_analytics" ? "Your selected Journal records contain different P&amp;L currencies. Select a single account/currency or correct trade currency fields before calculating monetary analytics." : "Could not load analytics from your Journal. Check your connection and retry."}</p><button className="btn" onClick={()=>void analytics.refetch()}>Retry</button></div> : null}
    </section>
    {a && a.trade_count > 0 && !mixedCurrencies ? <><nav aria-label="Analytics workspaces" className="analytics-tabs">{(["overview","performance","edge","execution","behaviour","risk","calendar"] as const).map(tab=><button key={tab} type="button" className={activeTab===tab?"analytics-tab active":"analytics-tab"} aria-current={activeTab===tab?"page":undefined} onClick={()=>{setActiveTab(tab);if(window.location.hash!==`#${tab}`)window.location.hash=tab;}}>{tab==="edge"?"Edge Explorer":tab==="behaviour"?"Behaviour":tab==="calendar"?"Calendar":tab.charAt(0).toUpperCase()+tab.slice(1)}</button>)}</nav>
      {activeTab==="calendar" ? <CalendarAnalytics curve={a.equity_curve} trades={a.trade_metrics ?? []} currency={displayCurrency} onSelectDate={date=>{setFromDate(date);setToDate(date);setDatePreset("custom");}} /> : null}
      {activeTab==="behaviour" && intelligence.data ? <div style={{marginTop:14}}><BehaviourDashboard data={intelligence.data} /></div> : activeTab==="behaviour" && intelligence.isLoading ? <section className="panel" style={{marginTop:14}}>Calculating process intelligence…</section> : activeTab==="behaviour" && intelligence.error ? <section className="panel error" style={{marginTop:14}}>Process intelligence could not be loaded. <button className="btn" onClick={()=>void intelligence.refetch()}>Retry</button></section> : activeTab==="behaviour" ? <section className="panel" style={{marginTop:14}}><h2 className="panel-title">BEHAVIOUR</h2><p>Behavioral analytics require recorded rule adherence, setup validity, emotion, mistake and discipline fields. No missing data is inferred.</p><Link href="/journal" className="btn">Review Journal records</Link></section> : null}
      {activeTab==="execution" ? <div className="grid grid-dash" style={{marginTop:14}}>
        <section className="panel"><h2 className="panel-title">EXECUTION ANALYSIS</h2>
          <div className="grid grid-kpi">
            <div className="panel analytics-kpi-card"><span className="metric-label">Average MFE</span><strong className="metric-value pos">{a.average_mfe_r == null ? "Not recorded" : Number(a.average_mfe_r).toFixed(2)+"R"}</strong><span className="metric-sub">Maximum favorable excursion</span></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Average MAE</span><strong className="metric-value neg">{a.average_mae_r == null ? "Not recorded" : Number(a.average_mae_r).toFixed(2)+"R"}</strong><span className="metric-sub">Maximum adverse excursion</span></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Average realized R</span><strong className="metric-value">{a.average_r == null ? "Not recorded" : Number(a.average_r).toFixed(2)+"R"}</strong></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Average MFE capture</span><strong className="metric-value">{a.average_mfe_capture_percent == null ? "Not available" : Number(a.average_mfe_capture_percent).toFixed(1)+"%"}</strong><span className="metric-sub">{a.execution_coverage_count ?? 0} trades with both realized R and positive MFE</span></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Average holding time</span><strong className="metric-value">{a.average_holding_seconds == null ? "—" : (a.average_holding_seconds/3600).toFixed(1)+"h"}</strong><span className="metric-sub">Median {a.median_holding_seconds == null ? "—" : (a.median_holding_seconds/3600).toFixed(1)+"h"}</span></div>
          </div>
          <div style={{marginTop:12}}><ExecutionQualityChart mfe={a.average_mfe_r} mae={a.average_mae_r} averageR={a.average_r}/></div>
          <h3 style={{marginTop:20}}>Trade-level execution review</h3>
          <p className="muted">MFE capture is calculated per trade only when realized R and positive MFE are both recorded. Holding time uses opened_at to closed_at. No tick-level slippage or spread is inferred.</p>
          {(a.trade_metrics ?? []).length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed (UTC)</th><th>Trade</th><th>Net P&amp;L</th><th>Realized R</th><th>MFE</th><th>MAE</th><th>Capture</th><th>Held</th></tr></thead><tbody>{(a.trade_metrics ?? []).map(t=><tr key={t.trade_id}><td>{new Date(t.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})}</td><td><button type="button" className="btn" onClick={()=>setSelectedTrade(t)}>{t.symbol} · {t.direction.toUpperCase()}</button></td><td className={Number(t.net_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl,displayCurrency)}</td><td>{t.realized_r == null ? "—" : Number(t.realized_r).toFixed(2)+"R"}</td><td>{t.mfe_r == null ? "—" : Number(t.mfe_r).toFixed(2)+"R"}</td><td>{t.mae_r == null ? "—" : Number(t.mae_r).toFixed(2)+"R"}</td><td>{t.mfe_capture_percent == null ? "—" : Number(t.mfe_capture_percent).toFixed(1)+"%"}</td><td>{(t.holding_seconds/3600).toFixed(1)}h</td></tr>)}</tbody></table></div> : <p className="muted">No trade-level execution facts are available for this filter.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">EXECUTION DATA QUALITY</h2><div className="grid grid-kpi">
          <div className="panel"><span className="metric-label">MFE capture sample</span><strong className="metric-value">{a.execution_coverage_count ?? 0}/{a.trade_count}</strong></div>
          <div className="panel"><span className="metric-label">SL recorded</span><strong className="metric-value">{(a.trade_metrics ?? []).filter(t=>t.stop_loss_recorded).length}/{a.trade_count}</strong></div>
          <div className="panel"><span className="metric-label">TP recorded</span><strong className="metric-value">{(a.trade_metrics ?? []).filter(t=>t.take_profit_recorded).length}/{a.trade_count}</strong></div>
        </div><p className="muted">Stop-loss and take-profit presence is a coverage indicator, not proof of correct placement or execution. Candle-derived excursions are not tick-level execution records.</p></section>
      </div> : null}
      {activeTab==="risk" ? <div className="grid grid-dash" style={{marginTop:14}}>
        <section className="panel"><h2 className="panel-title">JOURNAL RISK REVIEW</h2>
          <div className="grid grid-kpi">
            <div className="panel analytics-kpi-card"><span className="metric-label">Maximum realized drawdown</span><strong className="metric-value neg">{money(a.max_drawdown,displayCurrency)}</strong></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Risk coverage</span><strong className="metric-value">{Number(a.risk_coverage_percent ?? 0).toFixed(1)}%</strong><span className="metric-sub">{a.risk_coverage_count ?? 0} of {a.trade_count} trades have a positive recorded risk amount</span></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Average recorded risk</span><strong className="metric-value">{money(a.average_risk_amount,displayCurrency)}</strong></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Largest recorded risk</span><strong className="metric-value">{money(a.maximum_risk_amount,displayCurrency)}</strong></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Maximum win streak</span><strong className="metric-value pos">{a.maximum_win_streak ?? 0}</strong></div>
            <div className="panel analytics-kpi-card"><span className="metric-label">Maximum loss streak</span><strong className="metric-value neg">{a.maximum_loss_streak ?? 0}</strong></div>
          </div>
          <h3 style={{marginTop:14}}>Realized equity curve</h3>{a.equity_curve.length?<AnalyticsEquityChart curve={a.equity_curve} currency={displayCurrency}/>:<p className="muted">No closed-trade equity history.</p>}
          <h3 style={{marginTop:22}}>Underwater drawdown</h3><p className="muted">Peak-to-trough realized journal drawdown, shown separately from cumulative P&amp;L.</p><AnalyticsDrawdownChart curve={a.equity_curve} currency={displayCurrency}/>
          <h3 style={{marginTop:22}}>Trades with highest recorded risk</h3>
          {(a.trade_metrics ?? []).filter(t=>t.risk_amount != null).sort((x,y)=>Number(y.risk_amount)-Number(x.risk_amount)).slice(0,10).length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed (UTC)</th><th>Instrument</th><th>Recorded risk</th><th>Realized R</th><th>Net P&amp;L</th></tr></thead><tbody>{(a.trade_metrics ?? []).filter(t=>t.risk_amount != null).sort((x,y)=>Number(y.risk_amount)-Number(x.risk_amount)).slice(0,10).map(t=><tr key={t.trade_id}><td>{new Date(t.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})}</td><td><button type="button" className="btn" onClick={()=>setSelectedTrade(t)}>{t.symbol}</button></td><td>{money(t.risk_amount,displayCurrency)}</td><td>{t.realized_r == null?"—":Number(t.realized_r).toFixed(2)+"R"}</td><td className={Number(t.net_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl,displayCurrency)}</td></tr>)}</tbody></table></div> : <p className="muted">No positive risk amounts have been recorded for these trades.</p>}
          <p className="muted">This view reflects realized Journal outcomes only. It is not broker equity, open exposure, floating P&amp;L, or the MT5 risk engine’s official drawdown. Missing risk fields are not estimated.</p>
        </section>
        <section className="panel"><h2 className="panel-title">RISK REVIEW NOTES</h2><ul><li>Review risk coverage before drawing conclusions from average R.</li><li>Keep risk per trade consistent with your written trading plan.</li><li>Review the largest recorded risk and longest loss streak.</li><li>Separate drawdown caused by strategy expectancy from process violations.</li><li>Do not treat this analytics report as a broker-side risk limit or execution kill switch.</li></ul><Link href="/journal" className="btn">Review risk records in Journal</Link></section>
      </div> : null}
      {activeTab==="overview" || activeTab==="performance" ? <section className="analytics-summary"><div className="analytics-section-heading"><div><span className="analytics-eyebrow">PERFORMANCE SNAPSHOT</span><h2>Performance at a glance</h2><p className="muted">Six core measures for the selected account and period.</p></div></div><div className="grid grid-kpi">{metrics.slice(0,6).map(([label,value])=><section className="panel analytics-kpi-card" key={label}><div className="metric"><span className="metric-label">{label}</span><span className="metric-value">{value}</span></div></section>)}</div></section> : null}
      {activeTab==="overview" ? <><AnalyticsInsights symbols={a.by_symbol} sessions={a.by_session} setups={a.by_setup ?? []} currency={displayCurrency} /><section className="panel" style={{marginTop:14}}><div style={{display:"flex",justifyContent:"space-between",gap:12,alignItems:"center",flexWrap:"wrap"}}><h2 className="panel-title">RECENT CLOSED TRADES</h2><Link href="/journal" className="btn">Open Journal</Link></div>{recentTrades.isLoading ? <p className="muted">Loading recent trades…</p> : recentTrades.error ? <div className="error">Could not load recent trades. <button className="btn" onClick={()=>void recentTrades.refetch()}>Retry</button></div> : <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed</th><th>Symbol</th><th>Side</th><th>Source</th><th>Net P&amp;L</th><th>R</th></tr></thead><tbody>{(recentTrades.data?.data ?? []).filter(t=>t.status==="closed"&&t.closed_at).slice(0,10).map(t=><tr key={t.id}><td>{t.closed_at ? new Date(t.closed_at).toLocaleString() : "—"}</td><td><button type="button" className="btn" onClick={()=>setSelectedTrade(t)}>{t.symbol}</button></td><td>{t.direction.toUpperCase()}</td><td>{t.source_system ?? t.source}</td><td className={Number(t.net_pnl ?? t.gross_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl ?? t.gross_pnl, t.pnl_currency)}</td><td>{t.r_multiple == null ? "—" : Number(t.r_multiple).toFixed(2)+"R"}</td></tr>)}</tbody></table>{(recentTrades.data?.data ?? []).filter(t=>t.status==="closed"&&t.closed_at).length===0 ? <p className="muted">No closed trades in the selected period.</p> : null}</div>}</section></> : null}
      
      {activeTab==="edge" ? <div className="grid grid-dash" style={{marginTop:14}}><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by instrument" groups={a.by_symbol} currency={displayCurrency} onSelect={key=>setSymbolFilter(key)} /></section><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by setup" groups={a.by_setup ?? []} currency={displayCurrency} onSelect={key=>{const setup=setups.data?.find(x=>x.name===key);if(setup)setSetupFilter(setup.id);}} /></section><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by session" groups={a.by_session} currency={displayCurrency} onSelect={key=>setSelectedGroup({kind:"session",key})} /></section><section className="panel"><AnalyticsBreakdownChart title="Net P&L by weekday (UTC)" groups={weekdaysOrdered} currency={displayCurrency} onSelect={key=>setSelectedGroup({kind:"weekday",key})} /></section><section className="panel analytics-hourly-panel"><div className="analytics-chart-heading"><h2>Hourly performance heatmap</h2><p className="muted">Net realized P&amp;L by UTC hour. Empty cells indicate no recorded trades.</p></div><HourlyPerformanceHeatmap groups={a.by_hour_utc ?? []} currency={displayCurrency} onSelect={hour=>setSelectedGroup({kind:"hour",key:hour})} /></section></div> : null}
      {activeTab==="edge" && selectedGroup ? <section className="panel" style={{marginTop:14}}><div style={{display:"flex",justifyContent:"space-between",gap:12,alignItems:"center",flexWrap:"wrap"}}><div><h2 className="panel-title">TRADE INVESTIGATION · {selectedGroup.kind.toUpperCase()} · {selectedGroup.key}</h2><p className="muted">{investigatedTrades.length} matching trade records in the selected filters.</p></div><button className="btn" type="button" onClick={()=>setSelectedGroup(null)}>Clear selection</button></div>{investigatedTrades.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed (UTC)</th><th>Instrument</th><th>Side</th><th>Net P&amp;L</th><th>R</th><th>Risk</th><th>MFE / MAE</th></tr></thead><tbody>{investigatedTrades.map(t=><tr key={t.trade_id}><td>{new Date(t.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})}</td><td>{t.symbol}</td><td>{t.direction.toUpperCase()}</td><td className={Number(t.net_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl,displayCurrency)}</td><td>{t.realized_r==null?"—":Number(t.realized_r).toFixed(2)+"R"}</td><td>{money(t.risk_amount,displayCurrency)}</td><td>{t.mfe_r==null?"—":Number(t.mfe_r).toFixed(2)+"R"} / {t.mae_r==null?"—":Number(t.mae_r).toFixed(2)+"R"}</td></tr>)}</tbody></table></div>:<p className="muted">No trade-level records match this chart selection.</p>}</section>:null}
      <div className="grid grid-dash" style={{marginTop:14,display:activeTab==="edge"?"":"none"}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SYMBOL</h2>
          {a.by_symbol.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Symbol</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_symbol.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl, displayCurrency)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy, displayCurrency)}</td></tr>)}</tbody></table></div>:<p>No symbol data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SESSION</h2>
          {a.by_session.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Session</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_session.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy)}</td></tr>)}</tbody></table></div>:<p>No session data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SETUP</h2>
          {a.by_setup?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Setup</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_setup.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy)}</td></tr>)}</tbody></table></div>:<p>No setup grouping yet.</p>}
        </section>
      </div>

      <div className="grid grid-dash" style={{marginTop:14,display:activeTab==="edge"?"":"none"}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY WEEKDAY</h2>{a.by_weekday?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Weekday</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th></tr></thead><tbody>{a.by_weekday.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"pos":"neg"}>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No weekday data.</p>}</section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY HOUR (UTC)</h2>{a.by_hour_utc?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Hour</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th></tr></thead><tbody>{a.by_hour_utc.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"pos":"neg"}>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No hour data.</p>}</section>
      </div>
      {activeTab==="performance" ? <><section className="panel analytics-secondary-metrics"><span className="analytics-eyebrow">DEEPER METRICS</span><h2 className="analytics-filter-title">Trade quality &amp; execution stats</h2><div className="analytics-secondary-grid">{metrics.slice(6).map(([label,value])=><div className="analytics-secondary-metric" key={label}><span className="metric-label">{label}</span><strong className="metric-value">{value}</strong></div>)}</div></section><div className="grid grid-dash" style={{marginTop:14}}><section className="panel">{a.by_day?.length ? <AnalyticsBreakdownChart title="Daily realized P&L" groups={a.by_day} currency={displayCurrency} /> : <p className="muted">No daily P&amp;L points for this range.</p>}</section><section className="panel">{a.by_month?.length ? <AnalyticsBreakdownChart title="Monthly realized P&L" groups={a.by_month} currency={displayCurrency} /> : <p className="muted">No monthly P&amp;L points for this range.</p>}</section></div></> : null}
      <section className="panel" style={{marginTop:14,display:activeTab==="overview"||activeTab==="performance"?"":"none"}}><h2 className="panel-title">CUMULATIVE JOURNAL P&amp;L · CLOSED TRADES</h2><p className="muted">Realized Journal P&amp;L accumulated from closed trades. This is not broker-reported account equity.</p>
        {a.equity_curve.length ? <><AnalyticsEquityChart curve={a.equity_curve} currency={displayCurrency} /><details><summary>Inspect underlying trade series</summary><div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed</th><th>Trade</th><th>Net P&amp;L</th><th>Cumulative P&amp;L</th><th>Drawdown</th></tr></thead><tbody>{a.equity_curve.map((x,i)=><tr key={x.trade_id}><td>{new Date(x.closed_at).toLocaleString()}</td><td>{i+1}</td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl)}</td><td className={Number(x.equity)>=0?"num pos":"num neg"}>{money(x.equity)}</td><td className={Number(x.drawdown)<0?"num neg":"num"}>{money(x.drawdown)}</td></tr>)}</tbody></table></div></details></>:<p>No closed trades with realized P&amp;L yet. Add trades or import historical records in the Journal.</p>}
      </section>
      <section className="panel" style={{marginTop:14,display:activeTab==="overview"||activeTab==="risk"?"":"none"}}><p className="muted">{a.notice ?? "Analytics are descriptive historical calculations, not forecasts or trading advice."}</p></section>
    </>:null}
    {a && a.trade_count === 0 && !mixedCurrencies ? <section className="panel" style={{marginTop:14}}><h2>No closed trades in this range</h2><p className="muted">Analytics Lab only calculates performance from closed Journal trades with realized P&amp;L. Analytics will appear once eligible closed trades are available.</p><div style={{display:"flex",gap:8,flexWrap:"wrap"}}><Link href="/journal" className="btn primary">Add or import Journal trades</Link><button type="button" className="btn" onClick={()=>{setFromDate("");setToDate("");setDatePreset("all");}}>View all time</button></div></section> : null}
    {selectedTrade ? <div role="presentation" onClick={()=>setSelectedTrade(null)} style={{position:"fixed",inset:0,zIndex:1000,background:"rgba(0,0,0,.55)",display:"grid",placeItems:"center",padding:16}}>
      <section role="dialog" aria-modal="true" aria-labelledby="analytics-trade-detail-title" className="panel" onClick={event=>event.stopPropagation()} style={{width:"min(760px,100%)",maxHeight:"85vh",overflowY:"auto",boxShadow:"0 20px 60px rgba(0,0,0,.28)"}}>
        <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",gap:12}}><div><span className="analytics-eyebrow">TRADE-LEVEL INVESTIGATION</span><h2 id="analytics-trade-detail-title">{selectedTrade.symbol} · {selectedTrade.direction.toUpperCase()}</h2></div><button className="btn" type="button" onClick={()=>setSelectedTrade(null)}>Close</button></div>
        <p className="muted">Opened {new Date(selectedTrade.opened_at).toLocaleString("en-GB",{timeZone:"UTC"})} UTC · Closed {new Date(selectedTrade.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})} UTC</p>
        <div className="grid grid-kpi">
          <div className="panel"><span className="metric-label">Net P&amp;L</span><strong className={"metric-value "+(Number(selectedTrade.net_pnl)>=0?"pos":"neg")}>{money(selectedTrade.net_pnl,displayCurrency)}</strong></div>
          <div className="panel"><span className="metric-label">Risk amount</span><strong className="metric-value">{money(selectedTrade.risk_amount,displayCurrency)}</strong></div>
          <div className="panel"><span className="metric-label">Realized R</span><strong className="metric-value">{selectedTrade.realized_r==null?"—":Number(selectedTrade.realized_r).toFixed(2)+"R"}</strong></div>
          <div className="panel"><span className="metric-label">Holding time</span><strong className="metric-value">{(selectedTrade.holding_seconds/3600).toFixed(2)}h</strong></div>
          <div className="panel"><span className="metric-label">MFE / MAE</span><strong className="metric-value">{selectedTrade.mfe_r==null?"—":Number(selectedTrade.mfe_r).toFixed(2)+"R"} / {selectedTrade.mae_r==null?"—":Number(selectedTrade.mae_r).toFixed(2)+"R"}</strong></div>
          <div className="panel"><span className="metric-label">MFE capture</span><strong className="metric-value">{selectedTrade.mfe_capture_percent==null?"—":Number(selectedTrade.mfe_capture_percent).toFixed(1)+"%"}</strong></div>
        </div>
        <h3 style={{marginTop:18}}>Trade context</h3><p>Session: {selectedTrade.session ?? "Not recorded"} · Setup: {selectedTrade.setup ?? "Unassigned"}</p><p>Entry: {selectedTrade.entry_price} · Exit: {selectedTrade.exit_price ?? "Not recorded"} · Stop loss recorded: {selectedTrade.stop_loss_recorded?"Yes":"No"} · Take profit recorded: {selectedTrade.take_profit_recorded?"Yes":"No"}</p>
        <h3 style={{marginTop:18}}>Psychology record</h3><p>Before: {selectedTrade.psychology.emotion_before ?? "Not recorded"} · During: {selectedTrade.psychology.emotion_during ?? "Not recorded"} · After: {selectedTrade.psychology.emotion_after ?? "Not recorded"}</p><p>Confidence: {selectedTrade.psychology.confidence ?? "—"}/10 · Fear: {selectedTrade.psychology.fear ?? "—"}/10 · FOMO: {selectedTrade.psychology.fomo ?? "—"}/10 · Frustration: {selectedTrade.psychology.frustration ?? "—"}/10 · Revenge: {selectedTrade.psychology.revenge_intensity ?? "—"}/10 · Boredom: {selectedTrade.psychology.boredom ?? "—"}/10</p>
        <p className="muted">Trade ID: {selectedTrade.trade_id}. Missing values are displayed as not recorded; no psychology or risk values are inferred.</p>
      </section>
    </div> : null}
  </div>;
}
