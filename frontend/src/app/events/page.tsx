"use client";

import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "@/components/data-display/DataTable";
import { PageHeader } from "@/components/layout/PageHeader";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { formatDateTime } from "@/lib/format";
import type { TradingEvent } from "@/types/domain";

export default function EventsPage() {
  const { events, connected } = useSharedEventStream();
  const [selected, setSelected] = useState<TradingEvent | null>(null);

  const columns = useMemo<ColumnDef<TradingEvent>[]>(
    () => [
      {
        accessorKey: "timestamp",
        header: "Time",
        cell: ({ getValue }) => (
          <span className="num">{formatDateTime(String(getValue()))}</span>
        ),
      },
      { accessorKey: "type", header: "Type" },
      {
        accessorKey: "entity",
        header: "Entity",
        cell: ({ row }) => (
          <span className="num">{row.original.entity ?? "—"}</span>
        ),
      },
      {
        accessorKey: "accountId",
        header: "Account",
        cell: ({ row }) => row.original.accountId ?? "—",
      },
      {
        accessorKey: "strategyId",
        header: "Strategy",
        cell: ({ row }) => row.original.strategyId ?? "—",
      },
      {
        accessorKey: "instrumentId",
        header: "Instrument",
        cell: ({ row }) => row.original.instrumentId ?? "—",
      },
      {
        accessorKey: "correlationId",
        header: "Correlation",
        cell: ({ row }) => (
          <span className="num">{row.original.correlationId ?? "—"}</span>
        ),
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => row.original.status ?? "—",
      },
    ],
    [],
  );

  return (
    <>
      <PageHeader
        title="Event Stream"
        description="Immutable TradingEvent facts with correlation IDs. Live mock ticks append while EVENT STREAM is connected."
        actions={
          <span className={`badge ${connected ? "ok" : "warn"}`}>
            {connected ? "CONNECTED" : "DISCONNECTED"}
          </span>
        }
      />
      <div className="panel">
        <DataTable
          data={events}
          columns={columns}
          onRowClick={setSelected}
          emptyMessage="No events yet."
        />
      </div>
      {selected ? (
        <>
          <button
            type="button"
            className="drawer-backdrop"
            aria-label="Close event"
            onClick={() => setSelected(null)}
          />
          <aside className="drawer" role="dialog" aria-label="Event detail">
            <h2 style={{ marginTop: 0, fontSize: 16, fontWeight: 500 }}>
              {selected.type}
            </h2>
            <div className="metric-sub">{formatDateTime(selected.timestamp)}</div>
            <div className="metric-sub num">
              Correlation {selected.correlationId ?? "—"}
            </div>
            <div className="metric-sub">Strategy {selected.strategyId ?? "—"}</div>
            <div className="metric-sub">
              Instrument {selected.instrumentId ?? "—"}
            </div>
            <div className="metric-sub">Account {selected.accountId ?? "—"}</div>
            <p style={{ fontSize: 13 }}>{selected.detail}</p>
            {selected.payload ? (
              <>
                <div className="panel-title">PAYLOAD (REDACTED)</div>
                <pre
                  className="metric-sub"
                  style={{ whiteSpace: "pre-wrap", margin: 0 }}
                >
                  {JSON.stringify(selected.payload, null, 2)}
                </pre>
              </>
            ) : null}
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
