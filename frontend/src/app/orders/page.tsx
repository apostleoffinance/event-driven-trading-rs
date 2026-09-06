"use client";

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "@/components/data-display/DataTable";
import { orderTone, StatusBadge } from "@/components/data-display/StatusBadge";
import { PageHeader } from "@/components/layout/PageHeader";
import { OrderLifecycleChain } from "@/components/orders/OrderLifecycleChain";
import { getRepositories } from "@/lib/repositories";
import type { Order } from "@/types/domain";

export default function OrdersPage() {
  const [selected, setSelected] = useState<Order | null>(null);
  const { data = [], isLoading, isError, error } = useQuery({
    queryKey: ["orders"],
    queryFn: () => getRepositories().orders.listOrders(),
  });

  const columns = useMemo<ColumnDef<Order>[]>(
    () => [
      {
        accessorKey: "id",
        header: "Order ID",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      {
        accessorKey: "clientOrderId",
        header: "Client ID",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      { accessorKey: "accountId", header: "Account" },
      { accessorKey: "venueId", header: "Venue" },
      { accessorKey: "strategyId", header: "Strategy" },
      { accessorKey: "instrumentId", header: "Instrument" },
      { accessorKey: "side", header: "Side" },
      {
        accessorKey: "quantity",
        header: "Qty",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      {
        accessorKey: "filledQuantity",
        header: "Filled",
        cell: ({ getValue }) => <span className="num">{String(getValue())}</span>,
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => (
          <StatusBadge tone={orderTone(row.original.status)}>
            {row.original.status}
          </StatusBadge>
        ),
      },
      {
        accessorKey: "updatedAt",
        header: "Updated",
        cell: ({ getValue }) => (
          <span className="num">
            {new Date(String(getValue())).toLocaleString()}
          </span>
        ),
      },
    ],
    [],
  );

  if (isLoading) return <div className="loading">Loading orders…</div>;
  if (isError)
    return <div className="error">{(error as Error).message ?? "Failed to load"}</div>;

  return (
    <>
      <PageHeader
        title="Orders"
        description="OMS surface. Lifecycle is always TradeIntent → Risk → OMS → Venue → Fill — never Strategy → Order. Unknown means ambiguous venue outcome — reconcile, do not assume Failed."
      />
      <div className="panel" style={{ marginBottom: 12 }}>
        <div className="panel-title">EXECUTION CHAIN (SELECTED OR DEFAULT)</div>
        <OrderLifecycleChain status={selected?.status ?? "Submitted"} />
      </div>
      <div className="panel">
        <DataTable
          data={data}
          columns={columns}
          onRowClick={setSelected}
          emptyMessage="No orders."
        />
      </div>
      {selected ? (
        <>
          <button
            type="button"
            className="drawer-backdrop"
            aria-label="Close order detail"
            onClick={() => setSelected(null)}
          />
          <aside className="drawer" role="dialog" aria-label="Order detail">
            <h2 style={{ marginTop: 0, fontSize: 16, fontWeight: 500 }}>
              Order {selected.id}
            </h2>
            <div className="metric-sub">Client {selected.clientOrderId}</div>
            <div className="metric-sub">
              Intent {selected.tradeIntentId ?? "—"} (not an order)
            </div>
            <div className="metric-sub">
              Account {selected.accountId} · Venue {selected.venueId}
            </div>
            <div className="metric-sub" style={{ marginTop: 8 }}>
              Status{" "}
              <StatusBadge tone={orderTone(selected.status)}>
                {selected.status}
              </StatusBadge>
            </div>
            {selected.status === "Unknown" ? (
              <p className="metric-sub" style={{ marginTop: 8 }}>
                Venue response ambiguous. Reconciliation must resolve — never
                auto-correct or guess Failed.
              </p>
            ) : null}
            <div className="panel-title" style={{ marginTop: 16 }}>
              LIFECYCLE
            </div>
            <OrderLifecycleChain status={selected.status} />
            <div className="panel-title" style={{ marginTop: 16 }}>
              TRACE
            </div>
            <ol className="metric-sub">
              <li>TradeIntent {selected.tradeIntentId}</li>
              <li>Risk decision (required — no bypass)</li>
              <li>OMS {selected.id}</li>
              <li>Venue {selected.venueId}</li>
              <li>Fill qty {selected.filledQuantity}</li>
            </ol>
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
