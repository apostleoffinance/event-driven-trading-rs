"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { frontendConfig } from "@/config/frontend";
import { journalApi } from "@/lib/api/journal";
import { ApiError } from "@/lib/api/client";
import { getSessionUser, hasSession } from "@/lib/auth/session";
import { AnalyticsEquityChart } from "@/components/charts/AnalyticsEquityChart";
import { AnalyticsBreakdownChart } from "@/components/charts/AnalyticsBreakdownChart";

function money(value: string | null | undefined, currency = "USD") {
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
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");
  const [datePreset, setDatePreset] = useState("all");
  const [userEmail, setUserEmail] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"overview" | "performance" | "edge" | "execution" | "behaviour" | "risk">("overview");
  useEffect(() => {
    setAuthenticated(hasSession());
    setUserEmail(getSessionUser()?.email ?? null);
  }, []);

  const accounts = useQuery({
    queryKey: ["journal", "accounts"],
    queryFn: journalApi.listAccounts,
    enabled: authenticated && frontendConfig.dataMode === "api",
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
  const analytics = useQuery({
    queryKey: ["journal", "analytics", accountId, fromDate, toDate],
    queryFn: () => journalApi.analyticsSummary({ journal_account_id: accountId || undefined, ...dateFilter }),
    enabled: authenticated && frontendConfig.dataMode === "api" && accounts.isSuccess && !mixedCurrencies,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });

  const recentTrades = useQuery({
    queryKey: ["journal", "recent-closed-trades", accountId, fromDate, toDate],
    queryFn: () => journalApi.listTrades({ journal_account_id: accountId || undefined, limit: 10, offset: 0, closed_from: dateFilter.from, closed_to: dateFilter.to }),
    enabled: authenticated && frontendConfig.dataMode === "api" && accounts.isSuccess && !mixedCurrencies,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });

  const intelligence = useQuery({
    queryKey: ["journal", "analytics-intelligence", accountId, fromDate, toDate],
    queryFn: () => journalApi.analyticsIntelligence({ journal_account_id: accountId || undefined, ...dateFilter }),
    enabled: authenticated && frontendConfig.dataMode === "api" && !mixedCurrencies,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });

  if (frontendConfig.dataMode !== "api") return <div>
    <header className="page-header"><div><h1>Analytics Lab</h1><p>Trader performance intelligence from EdgeNest Journal.</p></div><span className="badge warn">Analytics unavailable</span></header>
    <section className="panel"><h2>Connect your trading journal</h2><p>Analytics will be available once your trading journal is connected and you have recorded closed trades.</p><Link className="btn" href="/journal">Open Journal</Link></section>
  </div>;
  if (!authenticated) return <div>
    <header className="page-header"><div><h1>Analytics Lab</h1><p>Performance analytics based on your Journal records.</p></div><span className="badge warn">Sign-in required</span></header>
    <section className="panel"><h2>Authentication required</h2><p>Sign in to see your private Journal analytics.</p><Link className="btn" href="/login">Sign in</Link></section>
  </div>;

  const a = analytics.data;
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
  return <div>
    <header className="page-header"><div><h1>Analytics Lab</h1><p>Performance intelligence from closed trades. {userEmail ? `Signed in as ${userEmail}.` : ""}</p></div><span className="badge ok">Journal-connected · {accounts.data?.length ?? 0} accounts</span></header>
    <section className="panel">
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",gap:12,flexWrap:"wrap"}}>
        <div><h2 className="panel-title">ANALYTICS FILTER</h2><p className="muted">Account selection changes all metrics and charts.</p></div>
        <label className="journal-field" style={{minWidth:240}}><span>Journal account</span>
          <select value={accountId} onChange={e=>setAccountId(e.target.value)}><option value="">All accounts</option>{accounts.data?.filter(x=>!x.archived).map(x=><option key={x.id} value={x.id}>{x.name} · {x.currency}</option>)}</select>
          <span>Date range</span><select value={datePreset} onChange={e=>applyDatePreset(e.target.value)}><option value="all">All time</option><option value="today">Today</option><option value="yesterday">Yesterday</option><option value="7d">Last 7 days</option><option value="30d">Last 30 days</option><option value="90d">Last 90 days</option><option value="month">This month</option><option value="lastmonth">Last month</option><option value="ytd">Year to date</option><option value="custom">Custom range</option></select>
          <span>From UTC</span><input type="date" value={fromDate} onChange={e=>{setFromDate(e.target.value);setDatePreset("custom");}} />
          <span>To UTC</span><input type="date" min={fromDate || undefined} value={toDate} onChange={e=>{setToDate(e.target.value);setDatePreset("custom");}} />
          <button className="btn" type="button" onClick={()=>{setAccountId("");setFromDate("");setToDate("");setDatePreset("all");}}>Reset</button>
        </label>
      </div>
      {accounts.isLoading ? <p className="muted">Loading your Journal accounts…</p> : null}
      {accounts.error ? <div className="error" role="alert"><p>Could not load your Journal accounts. Check your connection and try again.</p><button className="btn" onClick={()=>void accounts.refetch()}>Retry accounts</button></div> : null}
      {mixedCurrencies ? <div className="error" role="status"><strong>Select a single account to continue.</strong><p>These accounts use different currencies ({accountCurrencies.join(", ")}). EdgeNest will not combine unlike currencies into misleading P&amp;L totals. Choose an account above to see its analytics.</p></div> : null}
      {analytics.isLoading && !mixedCurrencies ? <p>Calculating your Journal analytics…</p> : null}
      {analytics.error ? <div className="error" role="alert"><p>{analytics.error instanceof ApiError && analytics.error.code === "mixed_currency_analytics" ? "Your selected Journal records contain different P&L currencies. Select a single account/currency or correct trade currency fields before calculating monetary analytics." : "Could not load analytics from your Journal. Check your connection and retry."}</p><button className="btn" onClick={()=>void analytics.refetch()}>Retry</button></div> : null}
    </section>
    {a && a.trade_count > 0 && !mixedCurrencies ? <><nav aria-label="Analytics workspaces" className="analytics-tabs" style={{display:"flex",gap:8,overflowX:"auto",padding:"12px 0",marginTop:10}}>
        {(["overview","performance","edge","execution","behaviour","risk"] as const).map(tab=><button key={tab} type="button" className={activeTab===tab?"btn primary":"btn"} aria-current={activeTab===tab?"page":undefined} onClick={()=>setActiveTab(tab)} style={{whiteSpace:"nowrap",textTransform:"capitalize"}}>{tab==="edge"?"Edge Explorer":tab==="behaviour"?"Behaviour":tab}</button>)}
      </nav>
      {activeTab==="behaviour" && intelligence.data ? <section className="panel" style={{marginTop:14}}>
        <h2 className="panel-title">PROCESS & STATISTICAL INTELLIGENCE</h2>
        <p className="muted">95% Wilson win-rate interval: {intelligence.data.win_rate_confidence_95 ? intelligence.data.win_rate_confidence_95.lower_percent.toFixed(1)+"%–"+intelligence.data.win_rate_confidence_95.upper_percent.toFixed(1)+"%" : "Not available"} · Sample: {intelligence.data.sample_status.replaceAll("_"," ")} · n={intelligence.data.trade_count}</p>
        <div className="grid grid-dash"><div><h3>Process vs outcome</h3><p>Good process / win: {intelligence.data.process_outcomes.good_process_win}</p><p>Good process / loss: {intelligence.data.process_outcomes.good_process_loss}</p><p>Poor process / win: {intelligence.data.process_outcomes.poor_process_win}</p><p>Poor process / loss: {intelligence.data.process_outcomes.poor_process_loss}</p></div>
        <div><h3>Behavioral markers</h3><p>Rule adherence: {intelligence.data.rule_adherence_percent == null ? "—" : intelligence.data.rule_adherence_percent.toFixed(1)+"%"}</p><p>Valid setup rate: {intelligence.data.valid_setup_percent == null ? "—" : intelligence.data.valid_setup_percent.toFixed(1)+"%"}</p><p>Emotional trades: {intelligence.data.emotional_trade_count}</p><p>Mistakes: {intelligence.data.mistake_count}</p><p>Average discipline: {intelligence.data.average_discipline_score == null ? "—" : intelligence.data.average_discipline_score.toFixed(1)+"/100"}</p></div></div>
        <h3 style={{marginTop:14}}>Imported Journal psychology</h3><p className="muted">Psychology records: {intelligence.data.psychology_record_count ?? "Not available"} · emotion recorded before/during/after: {intelligence.data.emotion_before_record_count ?? 0}/{intelligence.data.emotion_during_record_count ?? 0}/{intelligence.data.emotion_after_record_count ?? 0}</p><div className="grid grid-dash"><div><p>Confidence: {intelligence.data.average_confidence == null ? "Not recorded" : intelligence.data.average_confidence.toFixed(1)+"/10"}</p><p>Fear: {intelligence.data.average_fear == null ? "Not recorded" : intelligence.data.average_fear.toFixed(1)+"/10"}</p><p>FOMO: {intelligence.data.average_fomo == null ? "Not recorded" : intelligence.data.average_fomo.toFixed(1)+"/10"}</p></div><div><p>Frustration: {intelligence.data.average_frustration == null ? "Not recorded" : intelligence.data.average_frustration.toFixed(1)+"/10"}</p><p>Revenge intensity: {intelligence.data.average_revenge_intensity == null ? "Not recorded" : intelligence.data.average_revenge_intensity.toFixed(1)+"/10"}</p><p>Boredom: {intelligence.data.average_boredom == null ? "Not recorded" : intelligence.data.average_boredom.toFixed(1)+"/10"}</p></div></div><h3>Review insights</h3>{intelligence.data.insights.map((item,i)=><section key={item.type+"-"+i} className="panel" style={{marginTop:8}}><strong>{item.title}</strong><p>{item.message}</p></section>)}<p className="muted">{intelligence.data.notice}</p></section> : activeTab==="behaviour" && intelligence.isLoading ? <section className="panel" style={{marginTop:14}}>Calculating process intelligence…</section> : activeTab==="behaviour" && intelligence.error ? <section className="panel error" style={{marginTop:14}}>Process intelligence could not be loaded. <button className="btn" onClick={()=>void intelligence.refetch()}>Retry</button></section> : activeTab==="behaviour" ? <section className="panel" style={{marginTop:14}}><h2 className="panel-title">BEHAVIOUR</h2><p>Behavioral analytics require recorded rule adherence, setup validity, emotion, mistake and discipline fields. No missing data is inferred.</p><Link href="/journal" className="btn">Review Journal records</Link></section> : null}
      {activeTab==="execution" ? <section className="panel" style={{marginTop:14}}><h2 className="panel-title">EXECUTION ANALYSIS</h2><div className="grid grid-kpi"><div className="panel"><span className="metric-label">Average MFE (R)</span><strong className="metric-value">{a.average_mfe_r == null ? "Not available" : Number(a.average_mfe_r).toFixed(2)+"R"}</strong></div><div className="panel"><span className="metric-label">Average MAE (R)</span><strong className="metric-value">{a.average_mae_r == null ? "Not available" : Number(a.average_mae_r).toFixed(2)+"R"}</strong></div><div className="panel"><span className="metric-label">Trades with MFE / MAE</span><strong className="metric-value">{a.average_mfe_r == null || a.average_mae_r == null ? "Unknown" : "Journal-derived"}</strong></div></div><p className="muted" style={{marginTop:12}}>Only stored Journal MFE/MAE fields are used. Broker slippage, commission breakdown, duration, entry/exit efficiency and price-history-based execution metrics are not inferred if missing.</p></section> : null}
      {activeTab==="risk" ? <section className="panel" style={{marginTop:14}}><h2 className="panel-title">JOURNAL RISK REVIEW</h2><div className="grid grid-kpi"><div className="panel"><span className="metric-label">Maximum realized drawdown</span><strong className="metric-value">{money(a.max_drawdown, displayCurrency)}</strong></div><div className="panel"><span className="metric-label">Average R (valid risk records)</span><strong className="metric-value">{a.average_r == null ? "Not available" : Number(a.average_r).toFixed(2)+"R"}</strong></div><div className="panel"><span className="metric-label">Risk coverage</span><strong className="metric-value">{a.average_r == null ? "Incomplete" : "Journal risk records present"}</strong></div></div><p className="muted" style={{marginTop:12}}>This is risk calculated from closed Journal trade records. Broker equity, open positions, current exposure, and official account drawdown are not available from this analytics response.</p></section> : null}
      {activeTab==="overview" ? <section className="panel" style={{marginTop:14}}><div style={{display:"flex",justifyContent:"space-between",gap:12,alignItems:"center",flexWrap:"wrap"}}><h2 className="panel-title">RECENT CLOSED TRADES</h2><Link href="/journal" className="btn">Open Journal</Link></div>{recentTrades.isLoading ? <p className="muted">Loading recent trades…</p> : recentTrades.error ? <div className="error">Could not load recent trades. <button className="btn" onClick={()=>void recentTrades.refetch()}>Retry</button></div> : <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed</th><th>Symbol</th><th>Side</th><th>Source</th><th>Net P&amp;L</th><th>R</th></tr></thead><tbody>{(recentTrades.data?.data ?? []).filter(t=>t.status==="closed"&&t.closed_at).slice(0,10).map(t=><tr key={t.id}><td>{t.closed_at ? new Date(t.closed_at).toLocaleString() : "—"}</td><td>{t.symbol}</td><td>{t.direction.toUpperCase()}</td><td>{t.source_system ?? t.source}</td><td className="num">{money(t.net_pnl ?? t.gross_pnl, t.pnl_currency)}</td><td>{t.r_multiple == null ? "—" : Number(t.r_multiple).toFixed(2)+"R"}</td></tr>)}</tbody></table>{(recentTrades.data?.data ?? []).filter(t=>t.status==="closed"&&t.closed_at).length===0 ? <p className="muted">No closed trades in the selected period.</p> : null}</div>}</section> : null}
      <div className="grid grid-kpi" style={{marginTop:14,display:activeTab==="overview"||activeTab==="performance"?"":"none"}}>{metrics.map(([label,value])=><section className="panel" key={label}><div className="metric"><span className="metric-label">{label}</span><span className="metric-value">{value}</span></div></section>)}</div>
      {activeTab==="edge" ? <div className="grid grid-dash" style={{marginTop:14}}><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by instrument" groups={a.by_symbol} currency={displayCurrency} /></section><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by setup" groups={a.by_setup ?? []} currency={displayCurrency} /></section><section className="panel"><AnalyticsBreakdownChart title="Net P&L and win rate by session" groups={a.by_session} currency={displayCurrency} /></section></div> : null}
      <div className="grid grid-dash" style={{marginTop:14,display:activeTab==="edge"?"":"none"}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SYMBOL</h2>
          {a.by_symbol.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Symbol</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_symbol.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className="num">{money(x.net_pnl, displayCurrency)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy, displayCurrency)}</td></tr>)}</tbody></table></div>:<p>No symbol data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SESSION</h2>
          {a.by_session.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Session</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_session.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className="num">{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy)}</td></tr>)}</tbody></table></div>:<p>No session data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SETUP</h2>
          {a.by_setup?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Setup</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_setup.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className="num">{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy)}</td></tr>)}</tbody></table></div>:<p>No setup grouping yet.</p>}
        </section>
      </div>

      <div className="grid grid-dash" style={{marginTop:14,display:activeTab==="edge"?"":"none"}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY WEEKDAY</h2>{a.by_weekday?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Weekday</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th></tr></thead><tbody>{a.by_weekday.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No weekday data.</p>}</section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY HOUR (UTC)</h2>{a.by_hour_utc?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Hour</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th></tr></thead><tbody>{a.by_hour_utc.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No hour data.</p>}</section>
      </div>
      {activeTab==="performance" ? <div className="grid grid-dash" style={{marginTop:14}}><section className="panel">{a.by_day?.length ? <AnalyticsBreakdownChart title="Daily realized P&L" groups={a.by_day} currency={displayCurrency} /> : <p className="muted">No daily P&amp;L points for this range.</p>}</section><section className="panel">{a.by_month?.length ? <AnalyticsBreakdownChart title="Monthly realized P&L" groups={a.by_month} currency={displayCurrency} /> : <p className="muted">No monthly P&amp;L points for this range.</p>}</section></div> : null}
      <section className="panel" style={{marginTop:14,display:activeTab==="overview"||activeTab==="performance"?"":"none"}}><h2 className="panel-title">CUMULATIVE JOURNAL P&amp;L · CLOSED TRADES</h2><p className="muted">Realized Journal P&amp;L accumulated from closed trades. This is not broker-reported account equity.</p>
        {a.equity_curve.length ? <><AnalyticsEquityChart curve={a.equity_curve} currency={displayCurrency} /><details><summary>Inspect underlying trade series</summary><div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed</th><th>Trade</th><th>Net P&amp;L</th><th>Cumulative P&amp;L</th><th>Drawdown</th></tr></thead><tbody>{a.equity_curve.map((x,i)=><tr key={x.trade_id}><td>{new Date(x.closed_at).toLocaleString()}</td><td>{i+1}</td><td className="num">{money(x.net_pnl)}</td><td className="num">{money(x.equity)}</td><td className="num">{money(x.drawdown)}</td></tr>)}</tbody></table></div></details></>:<p>No closed trades with realized P&amp;L yet. Add trades or import historical records in the Journal.</p>}
      </section>
      <section className="panel" style={{marginTop:14,display:activeTab==="overview"||activeTab==="risk"?"":"none"}}><p className="muted">{a.notice ?? "Analytics are descriptive historical calculations, not forecasts or trading advice."}</p></section>
    </>:null}
    {a && a.trade_count === 0 && !mixedCurrencies ? <section className="panel" style={{marginTop:14}}><h2>No closed trades in this range</h2><p className="muted">Analytics Lab only calculates performance from closed Journal trades with realized P&amp;L. No placeholder or sample results are shown.</p><div style={{display:"flex",gap:8,flexWrap:"wrap"}}><Link href="/journal" className="btn primary">Add or import Journal trades</Link><button type="button" className="btn" onClick={()=>{setFromDate("");setToDate("");setDatePreset("all");}}>View all time</button></div></section> : null}
  </div>;
}
