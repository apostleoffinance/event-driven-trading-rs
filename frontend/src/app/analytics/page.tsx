"use client";

import "./analytics.css";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { journalApi, type JournalAnalytics } from "@/lib/api/journal";
import { JournalTradePlayback } from "@/components/journal/JournalTradePlayback";
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
function formatInstrumentPrice(value: number | string | null | undefined, symbol: string) {
  if (value == null || value === "") return "Not recorded";
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return "Not recorded";
  // Standard spot-FX display precision: fractional pip for non-JPY pairs, pip for JPY pairs.
  if (/^[A-Z]{6}(?:\.[A-Z0-9]+)?$/i.test(symbol)) {
    const precision = symbol.toUpperCase().includes("JPY") ? 3 : 5;
    return numeric.toFixed(precision);
  }
  // Non-FX instruments retain useful precision without exposing binary-float padding.
  return numeric.toLocaleString("en-US", { maximumFractionDigits: 8, useGrouping: false });
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
  const [savedViewError, setSavedViewError] = useState<string | null>(null);
  const [viewName, setViewName] = useState("");
  const [selectedGroup, setSelectedGroup] = useState<{kind:"symbol"|"setup"|"session"|"weekday"|"hour"|"month"|"week"|"date";key:string}|null>(null);
  const [selectedTrade, setSelectedTrade] = useState<NonNullable<JournalAnalytics["trade_metrics"]>[number] | null>(null);
  const [userEmail, setUserEmail] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"overview" | "performance" | "edge" | "execution" | "behaviour" | "risk" | "calendar">("overview");
  useEffect(() => {
    if (!selectedTrade) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSelectedTrade(null);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [selectedTrade]);
  useEffect(() => {
    setAuthenticated(hasSession());
    setUserEmail(getSessionUser()?.email ?? null);
    try {
      const preferenceScope = encodeURIComponent(getSessionUser()?.email ?? "unknown");
      const raw = window.localStorage.getItem(`quantos.analytics.filters.v1:${preferenceScope}`);
      if (raw) {
        const saved = JSON.parse(raw) as {accountId?:string;fromDate?:string;toDate?:string;datePreset?:string;symbolFilter?:string;setupFilter?:string};
        if (saved.accountId) setAccountId(saved.accountId);
        if (saved.fromDate) setFromDate(saved.fromDate);
        if (saved.toDate) setToDate(saved.toDate);
        if (saved.datePreset) setDatePreset(saved.datePreset);
        if (saved.symbolFilter) setSymbolFilter(saved.symbolFilter);
        if (saved.setupFilter) setSetupFilter(saved.setupFilter);
      }
    } catch { /* Ignore malformed local preferences and use safe defaults. */ }
    setFiltersRestored(true);
  }, []);
  useEffect(() => {
    if (!filtersRestored) return;
    const preferenceScope = encodeURIComponent(userEmail ?? "unknown");
    window.localStorage.setItem(`quantos.analytics.filters.v1:${preferenceScope}`, JSON.stringify({accountId,fromDate,toDate,datePreset,symbolFilter,setupFilter}));
  }, [filtersRestored,userEmail,accountId,fromDate,toDate,datePreset,symbolFilter,setupFilter]);
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
  const savedViewsQuery = useQuery({
    queryKey: ["journal", "analytics-saved-views", accountId],
    queryFn: () => journalApi.listAnalyticsSavedViews(accountId || undefined),
    enabled: authenticated && accounts.isSuccess,
    staleTime: 30_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });
  const savedViews = savedViewsQuery.data ?? [];

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

  const selectedTradeRecord = useQuery({
    queryKey: ["journal", "trade-detail", selectedTrade?.trade_id],
    queryFn: () => journalApi.getTrade(selectedTrade!.trade_id),
    enabled: authenticated && Boolean(selectedTrade?.trade_id),
    staleTime: 15_000,
    retry: 1,
  });
  const selectedTradeAttachments = useQuery({
    queryKey: ["journal", "attachments", selectedTrade?.trade_id],
    queryFn: () => journalApi.listAttachments(selectedTrade!.trade_id),
    enabled: authenticated && Boolean(selectedTrade?.trade_id),
    staleTime: 30_000,
    retry: 1,
  });
  const selectedTradeAttachmentUrls = useQuery({
    queryKey: ["journal", "attachment-download-urls", selectedTrade?.trade_id, selectedTradeAttachments.data?.map(item => item.id).join("|") ?? ""],
    queryFn: async () => {
      const items = selectedTradeAttachments.data ?? [];
      const entries = await Promise.all(items.map(async item => {
        try {
          const result = await journalApi.getAttachmentDownloadUrl(selectedTrade!.trade_id, item.id);
          return [item.id, result.download_url] as const;
        } catch {
          return [item.id, item.storage_provider === "external" ? item.storage_key : ""] as const;
        }
      }));
      return Object.fromEntries(entries);
    },
    enabled: authenticated && Boolean(selectedTrade?.trade_id) && selectedTradeAttachments.isSuccess,
    staleTime: 10 * 60_000,
    retry: 1,
  });

  useEffect(() => {
    if (!selectedTrade) return;
    const refreshed = analytics.data?.trade_metrics?.find(item => item.trade_id === selectedTrade.trade_id);
    if (!refreshed) return;
    if (refreshed.mfe_r !== selectedTrade.mfe_r
      || refreshed.mae_r !== selectedTrade.mae_r
      || refreshed.mfe_capture_percent !== selectedTrade.mfe_capture_percent
      || refreshed.profit_surrendered_r !== selectedTrade.profit_surrendered_r
      || refreshed.mfe_source !== selectedTrade.mfe_source
      || refreshed.mae_source !== selectedTrade.mae_source) {
      setSelectedTrade(refreshed);
    }
  }, [analytics.data?.trade_metrics, selectedTrade]);

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
    if (selectedGroup.kind === "symbol") return trade.symbol === selectedGroup.key;
    if (selectedGroup.kind === "setup") return trade.setup === selectedGroup.key;
    if (selectedGroup.kind === "session") return trade.session === selectedGroup.key;
    if (selectedGroup.kind === "hour") return new Date(trade.closed_at).toISOString().slice(11,13) + ":00" === selectedGroup.key;
    if (selectedGroup.kind === "month") return trade.closed_at.slice(0,7) === selectedGroup.key;
    if (selectedGroup.kind === "week") { const closed = new Date(trade.closed_at); closed.setUTCHours(0,0,0,0); closed.setUTCDate(closed.getUTCDate()-((closed.getUTCDay()+6)%7)); return closed.toISOString().slice(0,10) === selectedGroup.key; }
    if (selectedGroup.kind === "date") return trade.closed_at.slice(0,10) === selectedGroup.key;
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
  const metricDescriptions: Record<string, string> = {
    "Closed trades": "Number of closed Journal trades included by the current account, date, instrument and setup filters.",
    "Win rate": "Winning closed trades divided by all qualifying closed trades. Breakeven trades remain in the denominator.",
    "Net P&L": "Sum of recorded net profit and loss in the selected account currency.",
    "Expectancy / trade": "Average realized net profit or loss per qualifying closed trade.",
    "Profit factor": "Gross profit divided by the absolute value of gross loss. Unavailable when the denominator is zero.",
    "Max drawdown": "Largest peak-to-trough decline in cumulative realized Journal P&L, not broker equity drawdown.",
    "Average winner": "Average net P&L among winning closed trades.",
    "Average loser": "Average net P&L among losing closed trades.",
    "Average R": "Average realized R across trades with a valid recorded risk denominator.",
    "Avg. MFE": "Average maximum favorable excursion in R where the required risk and excursion records are available.",
    "Avg. MAE": "Average maximum adverse excursion in R where the required risk and excursion records are available.",
    "Rules followed": "Share of trades with a recorded rule decision that were marked as followed.",
    "Valid setup rate": "Share of trades with a recorded setup-validity decision that were marked valid.",
    "Avg. discipline": "Mean of recorded discipline scores. Missing scores are excluded.",
    "Emotional trades": "Count of trades explicitly recorded as emotional. Missing psychology records are not assumed to be non-emotional.",
    "Mistakes": "Count of mistakes explicitly recorded in the Journal.",
    "Gross profit": "Sum of positive gross trade outcomes before costs.",
    "Gross loss": "Sum of negative gross trade outcomes before costs.",
    "Breakeven": "Count of closed trades with a zero realized net outcome.",
  };
  return <div className="analytics-page">
    <header className="page-header analytics-page-header"><div><span className="analytics-eyebrow">TRADING INTELLIGENCE WORKSPACE</span><h1>Analytics Lab</h1><p>Understand your trading edge across profitability, execution, behaviour and risk. {userEmail ? `Signed in as ${userEmail}.` : ""}</p>{a?.generated_at ? <small className="muted">Analytics snapshot generated {new Date(a.generated_at).toLocaleString("en-GB",{timeZone:"UTC"})} UTC · All grouped session, weekday and hour metrics use UTC.</small> : null}</div><div style={{display:"flex",gap:10,alignItems:"center",flexWrap:"wrap"}}><Link className="btn" href="/market-data">Open TradingView Market Analysis</Link><span className="badge ok">Journal-connected · {accounts.data?.length ?? 0} accounts</span></div></header>
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
        <label className="analytics-filter-field"><span>Saved views · synced to your Journal account</span><select value="" onChange={e=>{const view=savedViews.find(v=>v.name===e.target.value && v.accountId===accountId);if(view){setFromDate(view.fromDate);setToDate(view.toDate);setDatePreset(view.datePreset);setSymbolFilter(view.symbolFilter);setSetupFilter(view.setupFilter);}}}><option value="">Load a saved view…</option>{savedViews.filter(view=>view.accountId===accountId).map(view=><option key={view.name} value={view.name}>{view.name}</option>)}</select></label>
        <label className="analytics-filter-field"><span>Save current filters</span><input value={viewName} onChange={e=>setViewName(e.target.value)} placeholder="Name this view"/></label>
        <div className="analytics-filter-field" style={{justifyContent:"end"}}><span>&nbsp;</span><button className="btn" type="button" disabled={!viewName.trim() || savedViewsQuery.isLoading} onClick={async()=>{const name=viewName.trim();setSavedViewError(null);try{await journalApi.saveAnalyticsSavedView({name,accountId:accountId || null,filters:{fromDate,toDate,datePreset,symbolFilter,setupFilter}});await savedViewsQuery.refetch();setViewName("");}catch{setSavedViewError("Could not save this view to your Journal account. Please retry.");}}}>Save view</button></div>
      </div>
      {savedViewError ? <p className="error analytics-inline-status" role="alert">{savedViewError}</p> : null}
      {savedViewsQuery.error ? <p className="error analytics-inline-status" role="alert">Saved views could not be loaded from the Journal API. <button type="button" className="btn" onClick={()=>void savedViewsQuery.refetch()}>Retry</button></p> : null}
      <div className="analytics-active-filters">{accountId ? <button type="button" onClick={()=>setAccountId("")}>Account: {selectedAccount?.name ?? "Selected"} ×</button> : null}{fromDate || toDate ? <button type="button" onClick={()=>{setFromDate("");setToDate("");setDatePreset("all");}}>Date: {fromDate || "Start"} – {toDate || "Today"} ×</button> : null}{symbolFilter ? <button type="button" onClick={()=>setSymbolFilter("")}>Instrument: {symbolFilter} ×</button> : null}{setupFilter ? <button type="button" onClick={()=>setSetupFilter("")}>Setup: {setups.data?.find(x=>x.id===setupFilter)?.name ?? "Selected"} ×</button> : null}<span>Reporting timezone: UTC · Currency: {displayCurrency}</span></div>
      {accounts.isLoading ? <p className="muted analytics-inline-status">Loading your Journal accounts…</p> : null}
      {accounts.error ? <div className="error analytics-inline-status" role="alert"><p>Could not load your Journal accounts. Check your connection and try again.</p><button className="btn" onClick={()=>void accounts.refetch()}>Retry accounts</button></div> : null}
      {mixedCurrencies ? <div className="error analytics-inline-status" role="status"><strong>Select a single account to continue.</strong><p>These accounts use different currencies ({accountCurrencies.join(", ")}). EdgeNest will not combine unlike currencies into misleading P&amp;L totals. Choose an account above to see its analytics.</p></div> : null}
      {analytics.isLoading && !mixedCurrencies ? <p className="muted analytics-inline-status">Calculating your Journal analytics…</p> : null}
      {analytics.error ? <div className="error analytics-inline-status" role="alert"><p>{analytics.error instanceof ApiError && analytics.error.code === "mixed_currency_analytics" ? "Your selected Journal records contain different P&amp;L currencies. Select a single account/currency or correct trade currency fields before calculating monetary analytics." : "Could not load analytics from your Journal. Check your connection and retry."}</p><button className="btn" onClick={()=>void analytics.refetch()}>Retry</button></div> : null}
    </section>
    {a && a.trade_count > 0 && !mixedCurrencies ? <><nav aria-label="Analytics workspaces" className="analytics-tabs">{(["overview","performance","edge","execution","behaviour","risk","calendar"] as const).map(tab=><button key={tab} type="button" className={activeTab===tab?"analytics-tab active":"analytics-tab"} aria-current={activeTab===tab?"page":undefined} onClick={()=>{setActiveTab(tab);if(window.location.hash!==`#${tab}`)window.location.hash=tab;}}>{tab==="edge"?"Edge Explorer":tab==="behaviour"?"Behaviour":tab==="calendar"?"Calendar":tab.charAt(0).toUpperCase()+tab.slice(1)}</button>)}</nav>
      {activeTab==="calendar" ? <CalendarAnalytics curve={a.equity_curve} trades={a.trade_metrics ?? []} currency={displayCurrency} onSelectDate={date=>{setFromDate(date);setToDate(date);setDatePreset("custom");setSelectedGroup({kind:"date",key:date});}} onSelectPeriod={(kind,key)=>setSelectedGroup({kind,key})} onSelectTrade={trade=>setSelectedTrade(trade)} /> : null}
      {activeTab==="behaviour" && intelligence.data ? <div style={{marginTop:14}}><BehaviourDashboard data={intelligence.data} trades={a.trade_metrics ?? []} onSelectGroup={(dimension,key)=>{if(dimension==="Session")setSelectedGroup({kind:"session",key});else if(dimension==="Setup")setSelectedGroup({kind:"setup",key});else if(dimension==="Month (UTC)")setSelectedGroup({kind:"month",key});else setSelectedGroup({kind:"weekday",key});}} /></div> : activeTab==="behaviour" && intelligence.isLoading ? <section className="panel" style={{marginTop:14}}>Calculating process intelligence…</section> : activeTab==="behaviour" && intelligence.error ? <section className="panel error" style={{marginTop:14}}>Process intelligence could not be loaded. <button className="btn" onClick={()=>void intelligence.refetch()}>Retry</button></section> : activeTab==="behaviour" ? <section className="panel" style={{marginTop:14}}><h2 className="panel-title">BEHAVIOUR</h2><p>Behavioral analytics require recorded rule adherence, setup validity, emotion, mistake and discipline fields. No missing data is inferred.</p><Link href="/journal" className="btn">Review Journal records</Link></section> : null}
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
          {(a.trade_metrics ?? []).length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed (UTC)</th><th>Trade</th><th className="num">Net P&amp;L</th><th className="num">Realized R</th><th className="num">MFE</th><th className="num">MAE</th><th className="num">Capture</th><th className="num">Surrendered</th><th className="num">Held</th></tr></thead><tbody>{(a.trade_metrics ?? []).map(t=><tr key={t.trade_id}><td>{new Date(t.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})}</td><td><button type="button" className="btn" onClick={()=>setSelectedTrade(t)}>{t.symbol} · {t.direction.toUpperCase()}</button></td><td className={Number(t.net_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl,displayCurrency)}</td><td className="num">{t.realized_r == null ? "—" : Number(t.realized_r).toFixed(2)+"R"}</td><td className="num">{t.mfe_r == null ? "—" : Number(t.mfe_r).toFixed(2)+"R"}</td><td className="num">{t.mae_r == null ? "—" : Number(t.mae_r).toFixed(2)+"R"}</td><td className="num">{t.mfe_capture_percent == null ? "—" : Number(t.mfe_capture_percent).toFixed(1)+"%"}</td><td className="num">{t.profit_surrendered_r == null ? "—" : Number(t.profit_surrendered_r).toFixed(2)+"R"}</td><td className="num">{(t.holding_seconds/3600).toFixed(1)}h</td></tr>)}</tbody></table></div> : <p className="muted">No trade-level execution facts are available for this filter.</p>}
        </section>
        <section className="panel execution-quality-panel"><h2 className="panel-title">EXECUTION DATA QUALITY</h2><p className="muted">Coverage of recorded execution fields across the currently selected trades.</p><div className="execution-coverage-grid">
          {[["MFE capture sample",a.execution_coverage_count ?? 0],["MFE recorded",a.mfe_coverage_count ?? 0],["MAE recorded",a.mae_coverage_count ?? 0],["Stop loss recorded",(a.trade_metrics ?? []).filter(t=>t.stop_loss_recorded).length],["Take profit recorded",(a.trade_metrics ?? []).filter(t=>t.take_profit_recorded).length]].map(([label,count])=>{const total=a.trade_count;const covered=Number(count)||0;const coverage=total>0?Math.min(100,covered/total*100):0;return <div className="execution-coverage-item" key={String(label)}><div className="execution-coverage-heading"><span>{label}</span><strong>{covered} <small>/ {total}</small></strong></div><div className="execution-coverage-track" role="progressbar" aria-label={String(label)} aria-valuemin={0} aria-valuemax={total} aria-valuenow={covered}><span style={{width:coverage+"%"}}/></div><small className="muted">{total?coverage.toFixed(0)+"% coverage":"No qualifying trades"}</small></div>;})}
        </div><p className="muted execution-quality-note">Stop-loss and take-profit presence is a coverage indicator, not proof of correct placement or execution. Candle-derived excursions are not tick-level execution records.</p></section>
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
            <div className="panel analytics-kpi-card"><span className="metric-label">Recovery factor</span><strong className="metric-value">{a.recovery_factor == null ? "—" : Number(a.recovery_factor).toFixed(2)}</strong><span className="metric-sub">Net profit ÷ max realized drawdown</span></div>
          </div>
          <h3 style={{marginTop:14}}>Realized equity curve</h3>{a.equity_curve.length?<AnalyticsEquityChart curve={a.equity_curve} currency={displayCurrency}/>:<p className="muted">No closed-trade equity history.</p>}
          <h3 style={{marginTop:22}}>Underwater drawdown</h3><p className="muted">Peak-to-trough realized journal drawdown, shown separately from cumulative P&amp;L.</p><AnalyticsDrawdownChart curve={a.equity_curve} currency={displayCurrency}/>
          <h3 style={{marginTop:22}}>Recorded risk distribution</h3><p className="muted">Quartiles of positive recorded risk amounts, in {displayCurrency}. This describes historical position risk records, not account-equity-normalized risk.</p><div className="grid grid-kpi risk-distribution-grid">{[["Minimum",a.risk_distribution?.minimum],["25th percentile",a.risk_distribution?.q1],["Median",a.risk_distribution?.median],["75th percentile",a.risk_distribution?.q3],["Maximum",a.risk_distribution?.maximum]].map(([label,value])=><div className="panel" key={label}><span className="metric-label">{label}</span><strong className="metric-value">{money(value,displayCurrency)}</strong></div>)}</div>
          <h3 style={{marginTop:22}}>Trades with highest recorded risk</h3>
          {(a.trade_metrics ?? []).filter(t=>t.risk_amount != null).sort((x,y)=>Number(y.risk_amount)-Number(x.risk_amount)).slice(0,10).length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed (UTC)</th><th>Instrument</th><th className="num">Recorded risk</th><th className="num">Realized R</th><th className="num">Net P&amp;L</th></tr></thead><tbody>{(a.trade_metrics ?? []).filter(t=>t.risk_amount != null).sort((x,y)=>Number(y.risk_amount)-Number(x.risk_amount)).slice(0,10).map(t=><tr key={t.trade_id}><td>{new Date(t.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})}</td><td><button type="button" className="btn" onClick={()=>setSelectedTrade(t)}>{t.symbol}</button></td><td className="num">{money(t.risk_amount,displayCurrency)}</td><td className="num">{t.realized_r == null?"—":Number(t.realized_r).toFixed(2)+"R"}</td><td className={Number(t.net_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl,displayCurrency)}</td></tr>)}</tbody></table></div> : <p className="muted">No positive risk amounts have been recorded for these trades.</p>}
          <p className="muted">This view reflects realized Journal outcomes only. It is not broker equity, open exposure, floating P&amp;L, or the MT5 risk engine’s official drawdown. Missing risk fields are not estimated.</p>
        </section>
        <section className="panel"><h2 className="panel-title">RISK REVIEW NOTES</h2><ul><li>Review risk coverage before drawing conclusions from average R.</li><li>Keep risk per trade consistent with your written trading plan.</li><li>Review the largest recorded risk and longest loss streak.</li><li>Separate drawdown caused by strategy expectancy from process violations.</li><li>Do not treat this analytics report as a broker-side risk limit or execution kill switch.</li></ul><Link href="/journal" className="btn">Review risk records in Journal</Link></section>
      </div> : null}
      {activeTab==="overview" || activeTab==="performance" ? <section className="analytics-summary"><div className="analytics-section-heading"><div><span className="analytics-eyebrow">PERFORMANCE SNAPSHOT</span><h2>Performance at a glance</h2><p className="muted">Six core measures for the selected account and period.</p></div></div><div className="grid grid-kpi">{metrics.slice(0,6).map(([label,value])=><section className="panel analytics-kpi-card" key={label} title={metricDescriptions[label]} aria-label={label + ". " + metricDescriptions[label]}><div className="metric"><span className="metric-label">{label}</span><span className="metric-value">{value}</span></div></section>)}</div></section> : null}
      {activeTab==="overview" ? <><AnalyticsInsights symbols={a.by_symbol} sessions={a.by_session} setups={a.by_setup ?? []} currency={displayCurrency} onSelectGroup={(dimension,key)=>setSelectedGroup({kind:dimension,key})} /><section className="panel" style={{marginTop:14}}><div style={{display:"flex",justifyContent:"space-between",gap:12,alignItems:"center",flexWrap:"wrap"}}><h2 className="panel-title">RECENT CLOSED TRADES</h2><Link href="/journal" className="btn">Open Journal</Link></div>{recentTrades.isLoading ? <p className="muted">Loading recent trades…</p> : recentTrades.error ? <div className="error">Could not load recent trades. <button className="btn" onClick={()=>void recentTrades.refetch()}>Retry</button></div> : <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed</th><th>Symbol</th><th>Side</th><th>Source</th><th className="num">Net P&amp;L</th><th className="num">R</th></tr></thead><tbody>{(recentTrades.data?.data ?? []).filter(t=>t.status==="closed"&&t.closed_at).slice(0,10).map(t=><tr key={t.id}><td>{t.closed_at ? new Date(t.closed_at).toLocaleString("en-GB",{timeZone:"UTC"}) : "—"}</td><td><button type="button" className="btn" disabled={!a.trade_metrics?.some(x=>x.trade_id===t.id)} onClick={()=>{const detail=a.trade_metrics?.find(x=>x.trade_id===t.id);if(detail)setSelectedTrade(detail);}}>{t.symbol}</button></td><td>{t.direction.toUpperCase()}</td><td>{t.source_system ?? t.source}</td><td className={Number(t.net_pnl ?? t.gross_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl ?? t.gross_pnl, t.pnl_currency)}</td><td className="num">{t.r_multiple == null ? "—" : Number(t.r_multiple).toFixed(2)+"R"}</td></tr>)}</tbody></table>{(recentTrades.data?.data ?? []).filter(t=>t.status==="closed"&&t.closed_at).length===0 ? <p className="muted">No closed trades in the selected period.</p> : null}</div>}</section></> : null}
      
      {activeTab==="edge" ? <div className="grid grid-dash" style={{marginTop:14}}><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by instrument" groups={a.by_symbol} currency={displayCurrency} onSelect={key=>{setSymbolFilter(key);setSelectedGroup({kind:"symbol",key});}} /></section><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by setup" groups={a.by_setup ?? []} currency={displayCurrency} onSelect={key=>{const setup=setups.data?.find(x=>x.name===key);if(setup)setSetupFilter(setup.id);setSelectedGroup({kind:"setup",key});}} /></section><section className="panel analytics-breakdown-wide"><AnalyticsBreakdownChart title="Net P&L and win rate by session" groups={a.by_session} currency={displayCurrency} onSelect={key=>setSelectedGroup({kind:"session",key})} /></section><section className="panel"><AnalyticsBreakdownChart title="Net P&L by weekday (UTC)" groups={weekdaysOrdered} currency={displayCurrency} onSelect={key=>setSelectedGroup({kind:"weekday",key})} /></section><section className="panel analytics-hourly-panel"><div className="analytics-chart-heading"><h2>Hourly performance heatmap</h2><p className="muted">Net realized P&amp;L by UTC hour. Empty cells indicate no recorded trades.</p></div><HourlyPerformanceHeatmap groups={a.by_hour_utc ?? []} currency={displayCurrency} onSelect={hour=>setSelectedGroup({kind:"hour",key:hour})} /></section></div> : null}
      {selectedGroup && (activeTab==="edge" || activeTab==="behaviour" || activeTab==="calendar" || activeTab==="overview" || activeTab==="performance") ? <section className="panel" style={{marginTop:14}}><div style={{display:"flex",justifyContent:"space-between",gap:12,alignItems:"center",flexWrap:"wrap"}}><div><h2 className="panel-title">TRADE INVESTIGATION · {selectedGroup.kind.toUpperCase()} · {selectedGroup.key}</h2><p className="muted">{investigatedTrades.length} matching trade records in the selected filters.</p></div><button className="btn" type="button" onClick={()=>setSelectedGroup(null)}>Clear selection</button></div>{investigatedTrades.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed (UTC)</th><th>Instrument</th><th>Side</th><th className="num">Net P&amp;L</th><th className="num">R</th><th className="num">Risk</th><th className="num">MFE / MAE</th></tr></thead><tbody>{investigatedTrades.map(t=><tr key={t.trade_id}><td>{new Date(t.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})}</td><td><button type="button" className="btn" onClick={()=>setSelectedTrade(t)}>{t.symbol}</button></td><td>{t.direction.toUpperCase()}</td><td className={Number(t.net_pnl)>=0?"num pos":"num neg"}>{money(t.net_pnl,displayCurrency)}</td><td>{t.realized_r==null?"—":Number(t.realized_r).toFixed(2)+"R"}</td><td className="num">{money(t.risk_amount,displayCurrency)}</td><td className="num">{t.mfe_r==null?"—":Number(t.mfe_r).toFixed(2)+"R"} / {t.mae_r==null?"—":Number(t.mae_r).toFixed(2)+"R"}</td></tr>)}</tbody></table></div>:<p className="muted">No trade-level records match this chart selection.</p>}</section>:null}
      <div className="grid grid-dash" style={{marginTop:14,display:activeTab==="edge"?"":"none"}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SYMBOL</h2>
          {a.by_symbol.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Symbol</th><th className="num">Trades</th><th className="num">Net P&amp;L</th><th className="num">Win rate</th><th className="num">Expectancy</th></tr></thead><tbody>{a.by_symbol.map(x=><tr key={x.key}><td><button type="button" className="btn" onClick={()=>setSelectedGroup({kind:"symbol",key:x.key})}>{x.key}</button></td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl, displayCurrency)}</td><td className="num">{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy, displayCurrency)}</td></tr>)}</tbody></table></div>:<p>No symbol data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SESSION</h2>
          {a.by_session.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Session</th><th className="num">Trades</th><th className="num">Net P&amp;L</th><th className="num">Win rate</th><th className="num">Expectancy</th></tr></thead><tbody>{a.by_session.map(x=><tr key={x.key}><td><button type="button" className="btn" onClick={()=>setSelectedGroup({kind:"session",key:x.key})}>{x.key}</button></td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl,displayCurrency)}</td><td className="num">{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy,displayCurrency)}</td></tr>)}</tbody></table></div>:<p>No session data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SETUP</h2>
          {a.by_setup?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Setup</th><th className="num">Trades</th><th className="num">Net P&amp;L</th><th className="num">Win rate</th><th className="num">Expectancy</th></tr></thead><tbody>{a.by_setup.map(x=><tr key={x.key}><td><button type="button" className="btn" onClick={()=>setSelectedGroup({kind:"setup",key:x.key})}>{x.key}</button></td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl,displayCurrency)}</td><td className="num">{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy,displayCurrency)}</td></tr>)}</tbody></table></div>:<p>No setup grouping yet.</p>}
        </section>
      </div>

      <div className="grid grid-dash" style={{marginTop:14,display:activeTab==="edge"?"":"none"}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY WEEKDAY</h2>{a.by_weekday?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Weekday</th><th className="num">Trades</th><th className="num">Net P&amp;L</th><th className="num">Win rate</th></tr></thead><tbody>{a.by_weekday.map(x=><tr key={x.key}><td><button type="button" className="btn" onClick={()=>setSelectedGroup({kind:"weekday",key:x.key})}>{x.key}</button></td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"pos":"neg"}>{money(x.net_pnl,displayCurrency)}</td><td className="num">{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No weekday data.</p>}</section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY HOUR (UTC)</h2>{a.by_hour_utc?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Hour</th><th className="num">Trades</th><th className="num">Net P&amp;L</th><th className="num">Win rate</th></tr></thead><tbody>{a.by_hour_utc.map(x=><tr key={x.key}><td><button type="button" className="btn" onClick={()=>setSelectedGroup({kind:"hour",key:x.key})}>{x.key}</button></td><td>{x.trades}</td><td className={Number(x.net_pnl)>=0?"pos":"neg"}>{money(x.net_pnl,displayCurrency)}</td><td className="num">{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No hour data.</p>}</section>
      </div>
      {activeTab==="performance" ? <><section className="panel analytics-secondary-metrics"><span className="analytics-eyebrow">DEEPER METRICS</span><h2 className="analytics-filter-title">Trade quality &amp; execution stats</h2><div className="analytics-secondary-grid">{metrics.slice(6).map(([label,value])=><div className="analytics-secondary-metric" key={label} title={metricDescriptions[label]}><span className="metric-label">{label}</span><strong className="metric-value">{value}</strong></div>)}</div></section><div className="grid grid-dash" style={{marginTop:14}}><section className="panel">{a.by_day?.length ? <AnalyticsBreakdownChart title="Daily realized P&L" groups={a.by_day} currency={displayCurrency} sortBy="source" onSelect={key=>{if(/^\\d{4}-\\d{2}-\\d{2}$/.test(key)){setFromDate(key);setToDate(key);setDatePreset("custom");setSelectedGroup({kind:"date",key});}}} /> : <p className="muted">No daily P&amp;L points for this range.</p>}</section><section className="panel">{a.by_month?.length ? <AnalyticsBreakdownChart title="Monthly realized P&L" groups={a.by_month} currency={displayCurrency} sortBy="source" onSelect={key=>{const match=key.match(/^(\\d{4})-(\\d{2})$/);if(match){const year=Number(match[1]);const month=Number(match[2]);const end=new Date(Date.UTC(year,month,0)).toISOString().slice(0,10);setFromDate(key+"-01");setToDate(end);setDatePreset("custom");setSelectedGroup({kind:"month",key});}}} /> : <p className="muted">No monthly P&amp;L points for this range.</p>}</section></div></> : null}
      <section className="panel" style={{marginTop:14,display:activeTab==="overview"||activeTab==="performance"?"":"none"}}><h2 className="panel-title">CUMULATIVE JOURNAL P&amp;L · CLOSED TRADES</h2><p className="muted">Realized Journal P&amp;L accumulated from closed trades. This is not broker-reported account equity.</p>
        {a.equity_curve.length ? <><AnalyticsEquityChart curve={a.equity_curve} currency={displayCurrency} /><details><summary>Inspect underlying trade series</summary><div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed</th><th>Trade</th><th className="num">Net P&amp;L</th><th className="num">Cumulative P&amp;L</th><th className="num">Drawdown</th></tr></thead><tbody>{a.equity_curve.map((x,i)=><tr key={x.trade_id}><td>{new Date(x.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})}</td><td><button type="button" className="btn" disabled={!a.trade_metrics?.some(t=>t.trade_id===x.trade_id)} onClick={()=>{const trade=a.trade_metrics?.find(t=>t.trade_id===x.trade_id);if(trade)setSelectedTrade(trade);}}>{i+1} · Inspect</button></td><td className={Number(x.net_pnl)>=0?"num pos":"num neg"}>{money(x.net_pnl,displayCurrency)}</td><td className={Number(x.equity)>=0?"num pos":"num neg"}>{money(x.equity,displayCurrency)}</td><td className={Number(x.drawdown)<0?"num neg":"num"}>{money(x.drawdown,displayCurrency)}</td></tr>)}</tbody></table></div></details></>:<p>No closed trades with realized P&amp;L yet. Add trades or import historical records in the Journal.</p>}
      </section>
      <section className="panel" style={{marginTop:14,display:activeTab==="overview"||activeTab==="risk"?"":"none"}}><p className="muted">{a.notice ?? "Analytics are descriptive historical calculations, not forecasts or trading advice."}</p></section>
    </>:null}
    {a && a.trade_count === 0 && !mixedCurrencies ? <section className="panel" style={{marginTop:14}}><h2>No closed trades in this range</h2><p className="muted">Analytics Lab only calculates performance from closed Journal trades with realized P&amp;L. Analytics will appear once eligible closed trades are available.</p><div style={{display:"flex",gap:8,flexWrap:"wrap"}}><Link href="/journal" className="btn primary">Add or import Journal trades</Link><button type="button" className="btn" onClick={()=>{setFromDate("");setToDate("");setDatePreset("all");}}>View all time</button></div></section> : null}
    {selectedTrade ? <div className="trade-investigation-backdrop" role="presentation" onMouseDown={event=>{if(event.target===event.currentTarget)setSelectedTrade(null);}}>
      <section role="dialog" aria-modal="true" aria-labelledby="analytics-trade-detail-title" aria-describedby="analytics-trade-detail-meta" className="trade-investigation" onMouseDown={event=>event.stopPropagation()}>
        <header className="trade-investigation-header"><div><span className="analytics-eyebrow">TRADE INVESTIGATION</span><h2 id="analytics-trade-detail-title">{selectedTrade.symbol} <span>·</span> {selectedTrade.direction.toUpperCase()}</h2><p id="analytics-trade-detail-meta" className="muted">Opened {new Date(selectedTrade.opened_at).toLocaleString("en-GB",{timeZone:"UTC"})} UTC <span>·</span> Closed {new Date(selectedTrade.closed_at).toLocaleString("en-GB",{timeZone:"UTC"})} UTC</p></div><button className="btn" type="button" autoFocus onClick={()=>setSelectedTrade(null)}>Close <span aria-hidden="true">×</span></button></header>
        <section className="trade-investigation-section"><div className="trade-investigation-section-heading"><h3>Trade outcome</h3><p>Realized result and capital at risk</p></div><div className="trade-investigation-metrics trade-investigation-primary">
          <article className="trade-metric-card"><span>Net P&amp;L</span><strong className={Number(selectedTrade.net_pnl)>=0?"pos":"neg"}>{money(selectedTrade.net_pnl,displayCurrency)}</strong></article>
          <article className="trade-metric-card"><span>Risk amount</span><strong>{money(selectedTrade.risk_amount,displayCurrency)}</strong></article>
          <article className="trade-metric-card"><span>Realized R</span><strong>{selectedTrade.realized_r==null?"—":Number(selectedTrade.realized_r).toFixed(2)+"R"}</strong></article>
          <article className="trade-metric-card"><span>Holding time</span><strong>{(selectedTrade.holding_seconds/3600).toFixed(2)}h</strong></article>
        </div></section>
        <section className="trade-investigation-section"><div className="trade-investigation-section-heading"><h3>Excursion &amp; trade management</h3><p>Recorded movement during the trade; unavailable values are not estimated</p></div><div className="trade-investigation-metrics trade-investigation-secondary">
          <article className="trade-metric-card"><span>Maximum favorable excursion (MFE)</span><strong>{selectedTrade.mfe_r==null?"Not recorded":Number(selectedTrade.mfe_r).toFixed(2)+"R"}</strong></article>
          <article className="trade-metric-card"><span>Maximum adverse excursion (MAE)</span><strong>{selectedTrade.mae_r==null?"Not recorded":Number(selectedTrade.mae_r).toFixed(2)+"R"}</strong></article>
          <article className="trade-metric-card"><span>MFE capture</span><strong>{selectedTrade.mfe_capture_percent==null?"Not available":Number(selectedTrade.mfe_capture_percent).toFixed(1)+"%"}</strong></article>
          <article className="trade-metric-card"><span>Profit surrendered</span><strong>{selectedTrade.profit_surrendered_r==null?"Not available":Number(selectedTrade.profit_surrendered_r).toFixed(2)+"R"}</strong></article>
        </div><p className="trade-data-note">Excursion source: MFE {selectedTrade.mfe_source??"not recorded"} ({selectedTrade.mfe_precision??"precision unknown"}) · MAE {selectedTrade.mae_source??"not recorded"} ({selectedTrade.mae_precision??"precision unknown"}). These labels do not guarantee tick-level resolution.</p></section>
        <section className="trade-investigation-section"><div className="trade-investigation-section-heading"><h3>Execution details</h3><p>Recorded trade setup and order levels</p></div><dl className="trade-detail-grid">
          <div><dt>Instrument</dt><dd>{selectedTrade.symbol}</dd></div><div><dt>Direction</dt><dd>{selectedTrade.direction.toUpperCase()}</dd></div><div><dt>Session</dt><dd>{selectedTrade.session??"Not recorded"}</dd></div><div><dt>Setup</dt><dd>{selectedTrade.setup??"Unassigned"}</dd></div><div><dt>Entry price</dt><dd className="trade-detail-price">{formatInstrumentPrice(selectedTrade.entry_price,selectedTrade.symbol)}</dd></div><div><dt>Exit price</dt><dd className="trade-detail-price">{formatInstrumentPrice(selectedTrade.exit_price,selectedTrade.symbol)}</dd></div><div><dt>Stop loss</dt><dd>{selectedTrade.stop_loss_recorded?"Recorded":"Not recorded"}</dd></div><div><dt>Take profit</dt><dd>{selectedTrade.take_profit_recorded?"Recorded":"Not recorded"}</dd></div>
        </dl></section>
        <section className="trade-investigation-section trade-investigation-replay">
          <div className="trade-investigation-section-heading"><h3>Historical trade replay</h3><p>Historical candles are used to calculate MFE and MAE; no market values are invented</p></div>
          {selectedTradeRecord.isLoading ? <p className="muted">Loading full trade record…</p> : null}
          {selectedTradeRecord.isError ? <div className="panel error" role="alert"><p>Could not load the full trade record needed for replay.</p><button className="btn" type="button" onClick={() => void selectedTradeRecord.refetch()}>Retry</button></div> : null}
          {selectedTradeRecord.data ? <JournalTradePlayback key={selectedTrade.trade_id} trade={selectedTradeRecord.data} initiallyExpanded /> : null}
        </section>
        <section className="trade-investigation-section">
          <div className="trade-investigation-section-heading"><h3>Screenshots &amp; trade evidence</h3><p>Private images attached to this Journal trade</p></div>
          {selectedTradeAttachments.isLoading ? <p className="muted">Loading attached screenshots…</p> : null}
          {selectedTradeAttachments.isError ? <div className="panel error" role="alert"><p>Could not load this trade’s attachments.</p><button className="btn" type="button" onClick={() => void selectedTradeAttachments.refetch()}>Retry</button></div> : null}
          {selectedTradeAttachments.data?.length ? <div className="trade-evidence-grid">
            {selectedTradeAttachments.data.map(item => {
              const url = selectedTradeAttachmentUrls.data?.[item.id] ?? "";
              const stage = item.attachment_type === "before" ? "Before trade" : item.attachment_type === "after" ? "After trade" : "Trade evidence";
              const isImage = (item.content_type ?? "").startsWith("image/") || /\.(png|jpe?g|gif|webp|bmp|svg)$/i.test(item.original_filename ?? item.storage_key);
              return <article className="trade-evidence-card" key={item.id}>
                {url && isImage ? <a href={url} target="_blank" rel="noreferrer" aria-label={`Open ${item.original_filename || stage} in full size`}><img src={url} alt={item.original_filename || stage} loading="lazy" /></a> : <div className="trade-evidence-placeholder">{selectedTradeAttachmentUrls.isLoading ? "Preparing secure preview…" : "Preview unavailable"}</div>}
                <div className="trade-evidence-meta"><strong>{stage}</strong><span>{item.original_filename || "Screenshot"}</span>{url ? <a href={url} target="_blank" rel="noreferrer">Open full size</a> : null}</div>
              </article>;
            })}
          </div> : selectedTradeAttachments.isSuccess ? <p className="muted">No screenshots are attached to this trade. You can add them from Journal → Screenshots / Attachments.</p> : null}
        </section>
        <section className="trade-investigation-section"><div className="trade-investigation-section-heading"><h3>Psychology &amp; discipline</h3><p>Self-reported context captured in the Journal</p></div><div className="trade-psychology-grid">
          <div><span>Before</span><strong>{selectedTrade.psychology.emotion_before??"Not recorded"}</strong></div><div><span>During</span><strong>{selectedTrade.psychology.emotion_during??"Not recorded"}</strong></div><div><span>After</span><strong>{selectedTrade.psychology.emotion_after??"Not recorded"}</strong></div>
        </div><div className="trade-psychology-scores">{[["Confidence",selectedTrade.psychology.confidence],["Fear",selectedTrade.psychology.fear],["FOMO",selectedTrade.psychology.fomo],["Frustration",selectedTrade.psychology.frustration],["Revenge",selectedTrade.psychology.revenge_intensity],["Boredom",selectedTrade.psychology.boredom]].map(([label,value])=><div key={String(label)}><span>{label}</span><strong>{value==null?"—":value+"/10"}</strong></div>)}</div></section>
        <footer className="trade-investigation-footer"><span>Trade reference</span><code>{selectedTrade.trade_id}</code><p>Missing values are shown as not recorded. No risk, execution or psychology values are inferred.</p></footer>
      </section>
    </div>: null}
  </div>;
}
