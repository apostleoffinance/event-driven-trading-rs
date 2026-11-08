"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { frontendConfig } from "@/config/frontend";
import { journalApi } from "@/lib/api/journal";
import { hasSession } from "@/lib/auth/session";

function money(value: string | null | undefined, currency: string) {
  if (value == null || !Number.isFinite(Number(value))) return "—";
  try { return new Intl.NumberFormat(undefined, { style: "currency", currency, maximumFractionDigits: 2 }).format(Number(value)); }
  catch { return Number(value).toFixed(2) + " " + currency; }
}

export function JournalDashboardSummary() {
  const [authenticated, setAuthenticated] = useState(false);
  useEffect(() => setAuthenticated(hasSession()), []);
  const accounts = useQuery({
    queryKey: ["journal", "accounts"],
    queryFn: journalApi.listAccounts,
    enabled: authenticated && frontendConfig.dataMode === "api",
    staleTime: 30_000,
    refetchOnWindowFocus: true,
    retry: 1,
  });
  const activeAccounts = useMemo(() => (accounts.data ?? []).filter(account => !account.archived), [accounts.data]);
  const summaries = useQueries({
    queries: activeAccounts.map(account => ({
      queryKey: ["journal", "analytics", account.id, "dashboard-summary"],
      queryFn: () => journalApi.analyticsSummary({ journal_account_id: account.id }),
      enabled: authenticated && frontendConfig.dataMode === "api",
      staleTime: 15_000,
      refetchOnWindowFocus: true,
      retry: 1,
    })),
  });

  if (frontendConfig.dataMode !== "api" || !authenticated) return null;
  return <section className="panel" style={{ marginBottom: 14 }}>
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
      <div><h2 className="panel-title">JOURNAL ACCOUNTS</h2><p className="muted">Imported and manually recorded trading history. These are Journal records—not broker balance/equity.</p></div>
      <div style={{ display: "flex", gap: 8 }}><Link href="/journal" className="btn">Open Journal</Link><Link href="/analytics" className="btn primary">Analytics Lab</Link></div>
    </div>
    {accounts.isLoading ? <p className="muted">Loading Journal accounts…</p> : null}
    {accounts.error ? <div className="error" role="alert">Journal account summary could not be loaded. <button className="btn" onClick={() => void accounts.refetch()}>Retry</button></div> : null}
    {accounts.isSuccess && activeAccounts.length === 0 ? <p className="muted">No Journal accounts yet. Create one in Journal or import your historical trades.</p> : null}
    {activeAccounts.length ? <div className="grid grid-dash" style={{ marginTop: 12 }}>
      {activeAccounts.map((account, index) => {
        const result = summaries[index];
        const stats = result?.data;
        return <article className="panel" key={account.id}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "start" }}>
            <div><h3>{account.name}</h3><p className="muted">{account.broker || "Journal"} · {account.account_type} · {account.currency}</p></div>
            <span className="badge">{result?.isError ? "Unavailable" : result?.isLoading ? "Loading" : "Loaded"}</span>
          </div>
          {result?.isError ? <p className="muted">Analytics unavailable. Open Analytics Lab to retry.</p> : result?.isLoading || !stats ? <p className="muted">Loading closed-trade performance…</p> : stats.trade_count === 0 ? <p className="muted">No closed Journal trades recorded for this account yet. <Link href="/journal">Add or import trades</Link>.</p> : <div className="grid grid-kpi" style={{ marginTop: 8 }}>
            <div><span className="metric-label">Closed trades</span><strong className="metric-value">{stats.trade_count}</strong></div>
            <div><span className="metric-label">Journal net P&amp;L</span><strong className="metric-value">{money(stats.net_profit, account.currency)}</strong></div>
            <div><span className="metric-label">Win rate</span><strong className="metric-value">{stats.win_rate_percent == null ? "—" : Number(stats.win_rate_percent).toFixed(1)+"%"}</strong></div>
          </div>}
        </article>;
      })}
    </div> : null}
  </section>;
}
