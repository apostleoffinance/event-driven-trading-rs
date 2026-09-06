"use client";

import { useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { orderTone, StatusBadge } from "@/components/data-display/StatusBadge";
import { getRepositories } from "@/lib/repositories";
import type { Order } from "@/types/domain";

const LIFECYCLE = [
  "TradeIntent",
  "Risk Decision",
  "Order Created",
  "Submitted",
  "Accepted",
  "Filled",
] as const;

export default function OrdersPage() {
  const [rows, setRows] = useState<Order[]>([]);
  const [selected, setSelected] = useState<Order | null>(null);

  useEffect(() => {
    getRepositories().orders.listOrders().then(setRows);
  }, []);

  return (
    <>
      <PageHeader
        title="Orders"
        description="OMS surface. Lifecycle is always TradeIntent → Risk → OMS → Venue → Fill — never Strategy → Order."
      />
      <div className="panel" style={{ marginBottom: 12 }}>
        <div className="panel-title">EXECUTION CHAIN</div>
        <div className="pipeline">
          {LIFECYCLE.map((step, i) => (
            <div key={step} style={{ display: "contents" }}>
              {i > 0 ? <div className="pipeline-arrow">→</div> : null}
              <div className="pipeline-node">
                <div className="metric-label">{step}</div>
              </div>
            </div>
          ))}
        </div>
      </div>
      <div className="data-table-wrap panel">
        <table className="data-table">
          <thead>
            <tr>
              <th>Order ID</th>
              <th>Client ID</th>
              <th>Account</th>
              <th>Strategy</th>
              <th>Instrument</th>
              <th>Side</th>
              <th>Type</th>
              <th>Qty</th>
              <th>Filled</th>
              <th>Status</th>
              <th>Updated</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((o) => (
              <tr
                key={o.id}
                style={{ cursor: "pointer" }}
                onClick={() => setSelected(o)}
              >
                <td className="num">{o.id}</td>
                <td className="num">{o.clientOrderId}</td>
                <td>{o.accountId}</td>
                <td>{o.strategyId}</td>
                <td>{o.instrumentId}</td>
                <td>{o.side}</td>
                <td>{o.orderType}</td>
                <td className="num">{o.quantity}</td>
                <td className="num">{o.filledQuantity}</td>
                <td>
                  <StatusBadge tone={orderTone(o.status)}>{o.status}</StatusBadge>
                </td>
                <td className="num">{new Date(o.updatedAt).toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
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
            <h2 style={{ marginTop: 0 }}>Order {selected.id}</h2>
            <div className="metric-sub">Client {selected.clientOrderId}</div>
            <div className="metric-sub">
              Intent {selected.tradeIntentId ?? "—"} (not an order)
            </div>
            <div className="metric-sub">Venue {selected.venueId}</div>
            <div className="metric-sub">
              Status{" "}
              <StatusBadge tone={orderTone(selected.status)}>
                {selected.status}
              </StatusBadge>
            </div>
            <div className="panel-title" style={{ marginTop: 16 }}>
              TIMELINE
            </div>
            <ol className="metric-sub">
              <li>TradeIntent {selected.tradeIntentId}</li>
              <li>Risk decision (required)</li>
              <li>OMS {selected.id}</li>
              <li>Venue {selected.venueId}</li>
              <li>Fill qty {selected.filledQuantity}</li>
            </ol>
            <button type="button" className="btn" onClick={() => setSelected(null)}>
              Close
            </button>
          </aside>
        </>
      ) : null}
    </>
  );
}
