"use client";

import { useEffect, useMemo, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { Position } from "@/types/domain";

export default function PositionsPage() {
  const [rows, setRows] = useState<Position[]>([]);
  const [side, setSide] = useState<"ALL" | "Long" | "Short">("ALL");

  useEffect(() => {
    getRepositories().positions.listPositions().then(setRows);
  }, []);

  const filtered = useMemo(
    () => (side === "ALL" ? rows : rows.filter((r) => r.side === side)),
    [rows, side],
  );

  return (
    <>
      <PageHeader
        title="Positions"
        description="Open positions after venue fills. Risk sizing already applied upstream."
        actions={
          <select
            className="btn"
            value={side}
            onChange={(e) => setSide(e.target.value as typeof side)}
            aria-label="Filter by side"
          >
            <option value="ALL">All sides</option>
            <option value="Long">Long</option>
            <option value="Short">Short</option>
          </select>
        }
      />
      <div className="data-table-wrap panel">
        <table className="data-table">
          <thead>
            <tr>
              <th>Instrument</th>
              <th>Account</th>
              <th>Strategy</th>
              <th>Side</th>
              <th>Qty</th>
              <th>Entry</th>
              <th>Mark</th>
              <th>uP&L</th>
              <th>rP&L</th>
              <th>Exposure</th>
              <th>Risk %</th>
              <th>Opened</th>
            </tr>
          </thead>
          <tbody>
            {filtered.length === 0 ? (
              <tr>
                <td colSpan={12}>
                  <div className="empty">No open positions.</div>
                </td>
              </tr>
            ) : (
              filtered.map((p) => (
                <tr key={p.id}>
                  <td>{p.instrumentId}</td>
                  <td>{p.accountId}</td>
                  <td>{p.strategyId}</td>
                  <td>{p.side}</td>
                  <td className="num">{p.quantity}</td>
                  <td className="num">{p.entryPrice}</td>
                  <td className="num">{p.markPrice}</td>
                  <td className={`num ${Number(p.unrealizedPnl) >= 0 ? "pos" : "neg"}`}>
                    {formatSignedMoney(p.unrealizedPnl)}
                  </td>
                  <td className="num">{formatSignedMoney(p.realizedPnl)}</td>
                  <td className="num">{p.exposure}</td>
                  <td className="num">{p.riskPct.toFixed(2)}</td>
                  <td className="num">{new Date(p.openedAt).toLocaleString()}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </>
  );
}
