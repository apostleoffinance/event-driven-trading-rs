"use client";

import { useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { frontendConfig } from "@/config/frontend";
import { apiRequest } from "@/lib/api/client";
import { useQuery } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "@/components/data-display/DataTable";
import {
  healthTone,
  riskToneClass,
  StatusBadge,
} from "@/components/data-display/StatusBadge";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatMoney, formatPct, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { Account } from "@/types/domain";

export default function AccountsPage() {
  const [selected, setSelected] = useState<Account | null>(null);
  const { data = [], isLoading, isError, error } = useQuery({
    queryKey: ["accounts"],
    queryFn: () => getRepositories().accounts.listAccounts(),
  });

  const columns = useMemo<ColumnDef<Account>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Account",
        cell: ({ row }) => (
          <span>
            {row.original.name}
            <div className="metric-sub">{row.original.id}</div>
          </span>
        ),
      },
      { accessorKey: "type", header: "Type" },
      {
        accessorKey: "venueName",
        header: "Venue",
        cell: ({ row }) => (
          <span>
            {row.original.venueName}
            <div className="metric-sub">{row.original.venueId}</div>
          </span>
        ),
      },
      {
        accessorKey: "environment",
        header: "Env",
        cell: ({ getValue }) => (
          <span
            className={`env-pill${String(getValue()) === "LIVE" ? " live" : ""}`}
          >
            {String(getValue())}
          </span>
        ),
      },
      {
        accessorKey: "balance",
        header: "Balance",
        cell: ({ getValue }) => (
          <span className="num">{formatMoney(String(getValue()))}</span>
        ),
      },
      {
        accessorKey: "equity",
        header: "Equity",
        cell: ({ getValue }) => (
          <span className="num">{formatMoney(String(getValue()))}</span>
        ),
      },
      {
        accessorKey: "dailyPnl",
        header: "Daily P&L",
        cell: ({ row }) => (
          <span
            className={`num ${Number(row.original.dailyPnl) >= 0 ? "pos" : "neg"}`}
          >
            {formatSignedMoney(row.original.dailyPnl)}
          </span>
        ),
      },
      {
        accessorKey: "drawdownPct",
        header: "DD",
        cell: ({ getValue }) => (
          <span className="num">{formatPct(Number(getValue()))}</span>
        ),
      },
      {
        accessorKey: "riskStatus",
        header: "Risk",
        cell: ({ row }) => (
          <StatusBadge tone={riskToneClass(row.original.riskStatus)}>
            {row.original.riskStatus}
          </StatusBadge>
        ),
      },
      {
        accessorKey: "connectionStatus",
        header: "Conn",
        cell: ({ row }) => (
          <StatusBadge tone={healthTone(row.original.connectionStatus)}>
            {row.original.connectionStatus}
          </StatusBadge>
        ),
      },
    ],
    [],
  );

  if (isLoading) return <div className="loading">Loading accounts…</div>;
  if (isError)
    return (
      <div className="error">{(error as Error).message ?? "Failed to load"}</div>
    );

  return (
    <>
      <PageHeader
        title="Accounts"
        description="Capital endpoints from account-engine. Account ≠ Venue — each row shows venue separately."
      />
      <div className="panel">
        <DataTable
          data={data}
          columns={columns}
          onRowClick={setSelected}
          emptyMessage={frontendConfig.dataMode === "api" ? "No trading accounts. Create a Paper Account to begin, or have an existing account explicitly linked to your user." : "No accounts."}
        />
      </div>
      {createOpen ? (
        <>
          <button type="button" className="drawer-backdrop" aria-label="Close account creation" onClick={() => setCreateOpen(false)} />
          <aside className="drawer" role="dialog" aria-label="Create paper account">
            <h2 style={{ marginTop: 0, fontSize: 16, fontWeight: 500 }}>Create Paper Account</h2>
            <p className="metric-sub">This creates a PAPER-only simulated account. It does not connect a broker or enable live execution.</p>
            <form onSubmit={async (event) => {
              event.preventDefault();
              setCreating(true);
              setCreateError(null);
              try {
                await apiRequest("/api/v1/trading/accounts", {
                  method: "POST",
                  body: JSON.stringify({ name: accountName, initial_capital: initialCapital }),
                });
                await queryClient.invalidateQueries({ queryKey: ["accounts"] });
                await queryClient.invalidateQueries({ queryKey: ["risk", "overview"] });
                await queryClient.invalidateQueries({ queryKey: ["portfolio"] });
                setCreateOpen(false);
              } catch (e) {
                setCreateError(e instanceof Error ? e.message : "Account creation failed.");
              } finally {
                setCreating(false);
              }
            }}>
              <label className="metric-label" htmlFor="paper-account-name">Account name</label>
              <input id="paper-account-name" className="input" style={{ width: "100%", padding: 8, border: "1px solid var(--border)", background: "var(--panel)", borderRadius: 6, marginBottom: 8 }} value={accountName} maxLength={80} onChange={(e) => setAccountName(e.target.value)} required />
              <label className="metric-label" htmlFor="paper-account-capital" style={{ marginTop: 12 }}>Starting capital (account currency)</label>
              <input id="paper-account-capital" className="input num" style={{ width: "100%", padding: 8, border: "1px solid var(--border)", background: "var(--panel)", borderRadius: 6, marginBottom: 8 }} type="number" min="0.01" step="0.01" value={initialCapital} onChange={(e) => setInitialCapital(e.target.value)} required />
              {createError ? <div className="error" role="alert">{createError}</div> : null}
              <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
                <button className="btn primary" type="submit" disabled={creating}>{creating ? "Creating…" : "Create account"}</button>
                <button className="btn" type="button" disabled={creating} onClick={() => setCreateOpen(false)}>Cancel</button>
              </div>
            </form>
          </aside>
        </>
      ) : null}
      {selected ? (
        <>
          <button
            type="button"
            className="drawer-backdrop"
            aria-label="Close account"
            onClick={() => setSelected(null)}
          />
          <aside className="drawer" role="dialog" aria-label="Account detail">
            <h2 style={{ marginTop: 0, fontSize: 16, fontWeight: 500 }}>
              {selected.name}
            </h2>
            <div className="metric-sub">Account {selected.id}</div>
            <div className="metric-sub">
              Venue {selected.venueName} ({selected.venueId})
            </div>
            <div className="metric-sub num">
              Equity {formatMoney(selected.equity)}
            </div>
            <div className="metric-sub num">
              Available {formatMoney(selected.availableCapital)}
            </div>
            <div className="metric-sub">
              Open positions {selected.openPositionCount}
            </div>
            <button
              type="button"
              className="btn"
              style={{ marginTop: 12 }}
              onClick={() => setSelected(null)}
            >
              Close
            </button>
          </aside>
        </>
      ) : null}
    </>
  );
}
