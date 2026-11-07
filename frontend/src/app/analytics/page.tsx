"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { frontendConfig } from "@/config/frontend";
import { journalApi } from "@/lib/api/journal";
import { getSessionUser, hasSession } from "@/lib/auth/session";

function money(value: string | null | undefined) {
  if (value == null) return "—";
  const n = Number(value);
  return Number.isFinite(n) ? n.toLocaleString("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }) : "—";
}
function percent(value: string | null | undefined) {
  if (value == null) return "—";
  const n = Number(value);
  return Number.isFinite(n) ? n.toFixed(1) + "%" : "—";
}

export default function AnalyticsLabPage() {
  const [authenticated, setAuthenticated] = useState(false);
  const [accountId, setAccountId] = useState("");
  const [userEmail, setUserEmail] = useState<string | null>(null);
  useEffect(() => {
    setAuthenticated(hasSession());
    setUserEmail(getSessionUser()?.email ?? null);
  }, []);

  const accounts = useQuery({
    queryKey: ["journal", "accounts"],
    queryFn: journalApi.listAccounts,
    enabled: authenticated && frontendConfig.dataMode === "api",
  });
  const analytics = useQuery({
    queryKey: ["journal", "analytics", accountId],
    queryFn: () => journalApi.analyticsSummary({ journal_account_id: accountId || undefined }),
    enabled: authenticated && frontendConfig.dataMode === "api",
  });

  const intelligence = useQuery({
    queryKey: ["journal", "analytics-intelligence", accountId],
    queryFn: () => journalApi.analyticsIntelligence({ journal_account_id: accountId || undefined }),
    enabled: authenticated && frontendConfig.dataMode === "api",
  });

  if (frontendConfig.dataMode !== "api") return <div>
    <header className="page-header"><div><h1>Analytics Lab</h1><p>Trader performance intelligence from EdgeNest Journal.</p></div><span className="badge warn">API mode required</span></header>
    <section className="panel"><h2>Connect your Journal</h2><p>Analytics are computed from your actual closed journal trades. Enable API mode and sign in; EdgeNest does not fabricate performance history.</p><Link className="btn" href="/journal">Open Journal</Link></section>
  </div>;
  if (!authenticated) return <div>
    <header className="page-header"><div><h1>Analytics Lab</h1><p>Performance analytics based on your Journal records.</p></div><span className="badge warn">Sign-in required</span></header>
    <section className="panel"><h2>Authentication required</h2><p>Sign in to see your private Journal analytics.</p><Link className="btn" href="/login">Sign in</Link></section>
  </div>;

  const a = analytics.data;
  const metrics: [string, string][] = [
    ["Closed trades", String(a?.trade_count ?? "—")],
    ["Win rate", percent(a?.win_rate_percent)],
    ["Net P&L", money(a?.net_profit)],
    ["Expectancy / trade", money(a?.expectancy)],
    ["Profit factor", a?.profit_factor == null ? (a?.trade_count ? "∞" : "—") : Number(a.profit_factor).toFixed(2)],
    ["Max drawdown", money(a?.max_drawdown)],
    ["Average winner", money(a?.average_win)],
    ["Average loser", money(a?.average_loss)],
    ["Average R", a?.average_r == null ? "—" : `${Number(a.average_r).toFixed(2)}R`],
    ["Avg. MFE", a?.average_mfe_r == null ? "—" : `${Number(a.average_mfe_r).toFixed(2)}R`],
    ["Avg. MAE", a?.average_mae_r == null ? "—" : `${Number(a.average_mae_r).toFixed(2)}R`],
    ["Rules followed", percent(a?.rules_followed_percent)],
    ["Valid setup rate", percent(a?.setup_valid_percent)],
    ["Avg. discipline", a?.average_discipline_score == null ? "—" : Number(a.average_discipline_score).toFixed(0) + "/100"],
    ["Emotional trades", String(a?.emotional_trade_count ?? "—")],
    ["Mistakes", String(a?.mistake_count ?? "—")],
    ["Gross profit", money(a?.gross_profit)],
    ["Gross loss", money(a?.gross_loss)],
    ["Breakeven", String(a?.breakeven ?? "—")],
  ];
  return <div>
    <header className="page-header"><div><h1>Analytics Lab</h1><p>Performance intelligence from closed trades. {userEmail ? `Signed in as ${userEmail}.` : ""}</p></div><span className="badge ok">Journal-connected</span></header>
    <section className="panel">
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",gap:12,flexWrap:"wrap"}}>
        <div><h2 className="panel-title">ANALYTICS FILTER</h2><p className="muted">Account selection changes all metrics and charts.</p></div>
        <label className="journal-field" style={{minWidth:240}}><span>Journal account</span>
          <select value={accountId} onChange={e=>setAccountId(e.target.value)}><option value="">All accounts</option>{accounts.data?.map(x=><option key={x.id} value={x.id}>{x.name} · {x.currency}</option>)}</select>
        </label>
      </div>
      {analytics.isLoading ? <p>Calculating your Journal analytics…</p> : null}
      {analytics.error ? <div className="error" role="alert"><p>{analytics.error instanceof Error ? analytics.error.message : "Could not load analytics."}</p><button className="btn" onClick={()=>void analytics.refetch()}>Retry</button></div> : null}
    </section>
    {a ? <>
      {intelligence.data ? <section className="panel" style={{marginTop:14}}>
        <h2 className="panel-title">PROCESS & STATISTICAL INTELLIGENCE</h2>
        <p className="muted">95% Wilson win-rate interval: {intelligence.data.win_rate_confidence_95 ? intelligence.data.win_rate_confidence_95.lower_percent.toFixed(1)+"%–"+intelligence.data.win_rate_confidence_95.upper_percent.toFixed(1)+"%" : "Not available"} · Sample: {intelligence.data.sample_status.replaceAll("_"," ")} · n={intelligence.data.trade_count}</p>
        <div className="grid grid-dash"><div><h3>Process vs outcome</h3><p>Good process / win: {intelligence.data.process_outcomes.good_process_win}</p><p>Good process / loss: {intelligence.data.process_outcomes.good_process_loss}</p><p>Poor process / win: {intelligence.data.process_outcomes.poor_process_win}</p><p>Poor process / loss: {intelligence.data.process_outcomes.poor_process_loss}</p></div>
        <div><h3>Behavioral markers</h3><p>Rule adherence: {intelligence.data.rule_adherence_percent == null ? "—" : intelligence.data.rule_adherence_percent.toFixed(1)+"%"}</p><p>Valid setup rate: {intelligence.data.valid_setup_percent == null ? "—" : intelligence.data.valid_setup_percent.toFixed(1)+"%"}</p><p>Emotional trades: {intelligence.data.emotional_trade_count}</p><p>Mistakes: {intelligence.data.mistake_count}</p><p>Average discipline: {intelligence.data.average_discipline_score == null ? "—" : intelligence.data.average_discipline_score.toFixed(1)+"/100"}</p></div></div>
        <h3>Review insights</h3>{intelligence.data.insights.map((item,i)=><section key={item.type+"-"+i} className="panel" style={{marginTop:8}}><strong>{item.title}</strong><p>{item.message}</p></section>)}<p className="muted">{intelligence.data.notice}</p></section> : intelligence.isLoading ? <section className="panel" style={{marginTop:14}}>Calculating process intelligence…</section> : intelligence.error ? <section className="panel error" style={{marginTop:14}}>Process intelligence could not be loaded. <button className="btn" onClick={()=>void intelligence.refetch()}>Retry</button></section> : null}
      <div className="grid grid-kpi" style={{marginTop:14}}>{metrics.map(([label,value])=><section className="panel" key={label}><div className="metric"><span className="metric-label">{label}</span><span className="metric-value">{value}</span></div></section>)}</div>
      <div className="grid grid-dash" style={{marginTop:14}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SYMBOL</h2>
          {a.by_symbol.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Symbol</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_symbol.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className="num">{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy)}</td></tr>)}</tbody></table></div>:<p>No symbol data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SESSION</h2>
          {a.by_session.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Session</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_session.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className="num">{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy)}</td></tr>)}</tbody></table></div>:<p>No session data yet.</p>}
        </section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY SETUP</h2>
          {a.by_setup?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Setup</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th><th>Expectancy</th></tr></thead><tbody>{a.by_setup.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td className="num">{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td><td className="num">{money(x.expectancy)}</td></tr>)}</tbody></table></div>:<p>No setup grouping yet.</p>}
        </section>
      </div>

      <div className="grid grid-dash" style={{marginTop:14}}>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY WEEKDAY</h2>{a.by_weekday?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Weekday</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th></tr></thead><tbody>{a.by_weekday.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No weekday data.</p>}</section>
        <section className="panel"><h2 className="panel-title">PERFORMANCE BY HOUR (UTC)</h2>{a.by_hour_utc?.length?<div className="data-table-wrap"><table className="data-table"><thead><tr><th>Hour</th><th>Trades</th><th>Net P&amp;L</th><th>Win rate</th></tr></thead><tbody>{a.by_hour_utc.map(x=><tr key={x.key}><td>{x.key}</td><td>{x.trades}</td><td>{money(x.net_pnl)}</td><td>{percent(x.win_rate_percent)}</td></tr>)}</tbody></table></div>:<p>No hour data.</p>}</section>
      </div>
      <section className="panel" style={{marginTop:14}}><h2 className="panel-title">EQUITY CURVE · CLOSED TRADES</h2>
        {a.equity_curve.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Closed</th><th>Trade</th><th>Net P&amp;L</th><th>Cumulative equity</th><th>Drawdown</th></tr></thead><tbody>{a.equity_curve.map((x,i)=><tr key={x.trade_id}><td>{new Date(x.closed_at).toLocaleString()}</td><td>{i+1}</td><td className="num">{money(x.net_pnl)}</td><td className="num">{money(x.equity)}</td><td className="num">{money(x.drawdown)}</td></tr>)}</tbody></table></div>:<p>No closed trades with realized P&amp;L yet. Add trades or import your TraderOS history in the Journal.</p>}
      </section>
      <section className="panel" style={{marginTop:14}}><p className="muted">{a.notice ?? "Analytics are descriptive historical calculations, not forecasts or trading advice."}</p></section>
    </>:null}
  </div>;
}
