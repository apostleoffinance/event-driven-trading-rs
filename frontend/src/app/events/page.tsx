"use client";

import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "@/components/data-display/DataTable";
import { PageHeader } from "@/components/layout/PageHeader";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { formatDateTime } from "@/lib/format";
import type { TradingEvent } from "@/types/domain";

function eventLabel(type: string) {
  return type.replace(/([a-z])([A-Z])/g, "$1 $2");
}

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
      { accessorKey: "type", header: "Activity", cell: ({ row }) => eventLabel(row.original.type) },
      {
        accessorKey: "instrumentId",
        header: "Instrument",
        cell: ({ row }) => row.original.instrumentId ?? "—",
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
        title="Recent activity"
        description="Review updates to your orders, positions and trading accounts."
        actions={
          <span className={`badge ${connected ? "ok" : "warn"}`}>
            {connected ? "Live updates" : "Updates paused"}
          </span>
        }
      />
      <div className="panel">
        <DataTable
          data={events}
          columns={columns}
          onRowClick={setSelected}
          emptyMessage="No recent trading activity."
        />
      </div>
      {selected ? (
        <>
          <button
            type="button"
            className="drawer-backdrop"
            aria-label="Close activity details"
            onClick={() => setSelected(null)}
          />
          <aside className="drawer" role="dialog" aria-label="Activity details">
            <h2 style={{ marginTop: 0, fontSize: 16, fontWeight: 500 }}>
              {eventLabel(selected.type)}
            </h2>
            <div className="metric-sub">{formatDateTime(selected.timestamp)}</div>
            <div className="metric-sub">Instrument {selected.instrumentId ?? "—"}</div>
            <div className="metric-sub">Status {selected.status ?? "Recorded"}</div>
            <p style={{ fontSize: 13 }}>{selected.detail}</p>
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
