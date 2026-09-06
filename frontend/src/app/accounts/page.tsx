"use client";

import { useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import {
  healthTone,
  riskToneClass,
  StatusBadge,
} from "@/components/data-display/StatusBadge";
import { formatMoney, formatPct, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { Account } from "@/types/domain";

export default function AccountsPage() {
  const [rows, setRows] = useState<Account[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getRepositories()
      .accounts.listAccounts()
      .then(setRows)
      .catch((e) => setError(e.message));
  }, []);

  return (
    <>
      <PageHeader
        title="Accounts"
        description="Capital endpoints from account-engine. Account ≠ Venue — each row shows venue separately."
      />
      {error ? <div className="error">{error}</div> : null}
      <div className="data-table-wrap panel">
        <table className="data-table">
          <thead>
            <tr>
              <th>Account</th>
              <th>Type</th>
              <th>Venue</th>
              <th>Env</th>
              <th>Balance</th>
              <th>Equity</th>
              <th>Available</th>
              <th>Daily P&L</th>
              <th>DD</th>
              <th>Risk</th>
              <th>Connection</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((a) => (
              <tr key={a.id}>
                <td>
                  <div>{a.name}</div>
                  <div className="metric-sub muted">{a.id}</div>
                </td>
                <td>{a.type}</td>
                <td>
                  {a.venueName}
                  <div className="metric-sub muted">{a.venueId}</div>
                </td>
                <td>
                  <StatusBadge tone="info">{a.environment}</StatusBadge>
                </td>
                <td className="num">{formatMoney(a.balance)}</td>
                <td className="num">{formatMoney(a.equity)}</td>
                <td className="num">{formatMoney(a.availableCapital)}</td>
                <td
                  className={`num ${Number(a.dailyPnl) >= 0 ? "pos" : "neg"}`}
                >
                  {formatSignedMoney(a.dailyPnl)}
                </td>
                <td className="num">{formatPct(-a.drawdownPct)}</td>
                <td>
                  <StatusBadge tone={riskToneClass(a.riskStatus)}>
                    {a.riskStatus}
                  </StatusBadge>
                </td>
                <td>
                  <StatusBadge tone={healthTone(a.connectionStatus)}>
                    {a.connectionStatus}
                  </StatusBadge>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
