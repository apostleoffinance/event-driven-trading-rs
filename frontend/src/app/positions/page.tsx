"use client";

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "@/components/data-display/DataTable";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { Position } from "@/types/domain";

export default function PositionsPage() {
  const [side, setSide] = useState<"ALL" | "Long" | "Short">("ALL");
  const [selected, setSelected] = useState<Position | null>(null);

  const { data = [], isLoading, isError, error } = useQuery({
    queryKey: ["positions"],
    queryFn: () => getRepositories().positions.listPositions(),
  });

  const filtered = useMemo(
    () => (side === "ALL" ? data : data.filter((r) => r.side === side)),
    [data, side],
  );

  const columns = useMemo<ColumnDef<Position>[]>(
    () => [
      { accessorKey: "instrumentId", header: "Instrument" },
      { accessorKey: "accountId", header: "Account" },
      { accessorKey: "strategyId", header: "Strategy" },
      { accessorKey: "side", header: "Side" },
      {
        accessorKey: "quantity",
        header: "Qty",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      {
        accessorKey: "entryPrice",
        header: "Entry",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      {
        accessorKey: "markPrice",
        header: "Mark",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      {
        accessorKey: "unrealizedPnl",
        header: "uP&L",
        cell: ({ row }) => (
          <span
            className={`num ${Number(row.original.unrealizedPnl) >= 0 ? "pos" : "neg"}`}
          >
            {formatSignedMoney(row.original.unrealizedPnl)}
          </span>
        ),
      },
      {
        accessorKey: "realizedPnl",
        header: "rP&L",
        cell: ({ row }) => (
          <span className="num">{formatSignedMoney(row.original.realizedPnl)}</span>
        ),
      },
      {
        accessorKey: "exposure",
        header: "Exposure",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      {
        accessorKey: "riskPct",
        header: "Risk %",
        cell: ({ getValue }) => (
          <span className="num">{Number(getValue()).toFixed(2)}</span>
        ),
      },
      {
        accessorKey: "openedAt",
        header: "Opened",
        cell: ({ getValue }) => (
          <span className="num">
            {new Date(String(getValue())).toLocaleString()}
          </span>
        ),
      },
    ],
    [],
  );

  if (isLoading) return <div className="loading">Loading positions…</div>;
  if (isError)
    return <div className="error">{(error as Error).message ?? "Failed to load"}</div>;

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
      <div className="panel">
        <DataTable
          data={filtered}
          columns={columns}
          onRowClick={setSelected}
          emptyMessage="No open positions."
        />
      </div>
      {selected ? (
        <>
          <button
            type="button"
            className="drawer-backdrop"
            aria-label="Close position"
            onClick={() => setSelected(null)}
          />
          <aside className="drawer" role="dialog" aria-label="Position detail">
            <h2 style={{ marginTop: 0, fontSize: 16, fontWeight: 500 }}>
              {selected.instrumentId} · {selected.side}
            </h2>
            <div className="metric-sub">Account {selected.accountId}</div>
            <div className="metric-sub">Strategy {selected.strategyId}</div>
            <div className="metric-sub num">Qty {selected.quantity}</div>
            <div className="metric-sub num">
              Entry {selected.entryPrice} · Mark {selected.markPrice}
            </div>
            <div className="metric-sub num">
              uP&L {formatSignedMoney(selected.unrealizedPnl)}
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
