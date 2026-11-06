"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { frontendConfig } from "@/config/frontend";
import { journalApi } from "@/lib/api/journal";
import { getSessionUser, hasSession, logout } from "@/lib/auth/session";

export default function JournalPage() {
  const [authenticated, setAuthenticated] = useState(false);
  const [userEmail, setUserEmail] = useState<string | null>(null);
  useEffect(() => {
    setAuthenticated(hasSession());
    setUserEmail(getSessionUser()?.email ?? null);
  }, []);

  const accounts = useQuery({ queryKey: ["journal", "accounts"], queryFn: () => journalApi.listAccounts(), enabled: authenticated && frontendConfig.dataMode === "api" });
  const trades = useQuery({ queryKey: ["journal", "trades"], queryFn: () => journalApi.listTrades({ limit: 100, offset: 0 }), enabled: authenticated && frontendConfig.dataMode === "api" });

  async function signOut() {
    try { await logout(); } finally {
      setAuthenticated(false);
      setUserEmail(null);
    }
  }

  if (frontendConfig.dataMode !== "api") {
    return <div>
      <header className="page-header"><div><h1>Trading Journal</h1><p>Persisted journal accounts, setups, and trade history.</p></div><span className="badge warn">Mock mode</span></header>
      <section className="panel"><h2>API mode is disabled</h2><p>Set <code>NEXT_PUBLIC_DATA_MODE=api</code> and configure <code>NEXT_PUBLIC_API_URL</code> to use the authenticated Rust Journal API. Mock data is not shown here.</p></section>
    </div>;
  }

  if (!authenticated) {
    return <div>
      <header className="page-header"><div><h1>Trading Journal</h1><p>Authenticated records from the Rust Journal API.</p></div><span className="badge warn">Sign-in required</span></header>
      <section className="panel"><h2>Authentication required</h2><p>Sign in to EdgeNest to load journal data. Journal records are scoped by the API to your authenticated owner.</p><Link className="btn" href="/login">Sign in</Link></section>
    </div>;
  }

  const error = accounts.error ?? trades.error;
  return <div>
    <header className="page-header"><div><h1>Trading Journal</h1><p>Live journal data from Rust API. {userEmail ? `Signed in as ${userEmail}.` : ""}</p></div><div style={{ display: "flex", gap: 8, alignItems: "center" }}><span className="badge ok">API mode</span><button className="btn" onClick={signOut}>Sign out</button></div></header>
    {error ? <section className="panel error" role="alert"><h2>Journal API error</h2><p>{error instanceof Error ? error.message : "Unable to load journal data."}</p><p>Check backend availability, API URL, authentication, and CORS.</p><button className="btn" onClick={() => { void accounts.refetch(); void trades.refetch(); }}>Retry</button></section> : null}
    <div className="grid grid-kpi">
      <section className="panel"><div className="metric"><span className="metric-label">Journal accounts</span><span className="metric-value">{accounts.isLoading ? "…" : accounts.data?.length ?? "—"}</span></div></section>
      <section className="panel"><div className="metric"><span className="metric-label">Loaded trades</span><span className="metric-value">{trades.isLoading ? "…" : trades.data?.data.length ?? "—"}</span><span className="metric-sub">First 100 records</span></div></section>
      <section className="panel"><div className="metric"><span className="metric-label">API connectivity</span><span className="metric-value">{error ? "Error" : accounts.isLoading || trades.isLoading ? "Checking…" : "Connected"}</span><span className="metric-sub">Identity and Journal API</span></div></section>
    </div>
    <section className="panel" style={{ marginTop: 14 }}><h2 className="panel-title">JOURNAL ACCOUNTS</h2>
      {accounts.isLoading ? <p>Loading journal accounts…</p> : accounts.data?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Name</th><th>Broker</th><th>Type</th><th>Currency</th><th>Initial balance</th></tr></thead><tbody>{accounts.data.map(a => <tr key={a.id}><td>{a.name}<div className="metric-sub">{a.id}</div></td><td>{a.broker ?? "—"}</td><td>{a.account_type}</td><td>{a.currency}</td><td className="num">{a.initial_balance}</td></tr>)}</tbody></table></div> : !error ? <p>No journal accounts yet. Create an account via Journal API before adding trades.</p> : null}
    </section>
    <section className="panel" style={{ marginTop: 14 }}><h2 className="panel-title">TRADE HISTORY</h2>
      {trades.isLoading ? <p>Loading journal trades…</p> : trades.data?.data.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Symbol</th><th>Direction</th><th>Status</th><th>Opened</th><th>Quantity</th><th>Entry</th><th>Net P&amp;L</th><th>Currency</th></tr></thead><tbody>{trades.data.data.map(t => <tr key={t.id}><td>{t.symbol}<div className="metric-sub">{t.id}</div></td><td>{t.direction}</td><td>{t.status}</td><td>{new Date(t.opened_at).toLocaleString()}</td><td className="num">{t.quantity}</td><td className="num">{t.entry_price}</td><td className="num">{t.net_pnl ?? "—"}</td><td>{t.pnl_currency}</td></tr>)}</tbody></table></div> : !error ? <p>No trades found. API returned an empty journal, not fabricated records.</p> : null}
      {trades.data?.pagination ? <p className="metric-sub">Offset {trades.data.pagination.offset}, limit {trades.data.pagination.limit}.</p> : null}
    </section>
  </div>;
}
