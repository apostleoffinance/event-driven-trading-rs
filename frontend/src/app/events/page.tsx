"use client";

import { useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { EventStreamList } from "@/components/events/EventStreamList";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { formatDateTime } from "@/lib/format";
import type { TradingEvent } from "@/types/domain";

export default function EventsPage() {
  const { events, connected } = useSharedEventStream();
  const [selected, setSelected] = useState<TradingEvent | null>(null);

  return (
    <>
      <PageHeader
        title="Event Stream"
        description="Immutable TradingEvent facts. Live mock ticks append while EVENT STREAM is connected."
        actions={
          <span className={`badge ${connected ? "ok" : "warn"}`}>
            {connected ? "CONNECTED" : "DISCONNECTED"}
          </span>
        }
      />
      <div className="panel">
        <div
          onClick={(e) => {
            const target = (e.target as HTMLElement).closest("[data-event-id]");
            if (!target) return;
            const id = target.getAttribute("data-event-id");
            const ev = events.find((x) => x.id === id) ?? null;
            setSelected(ev);
          }}
        >
          {events.map((e) => (
            <div key={e.id} data-event-id={e.id} style={{ cursor: "pointer" }}>
              <EventStreamList events={[e]} limit={1} />
            </div>
          ))}
        </div>
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
            <h2 style={{ marginTop: 0 }}>{selected.type}</h2>
            <div className="metric-sub">{formatDateTime(selected.timestamp)}</div>
            <div className="metric-sub">
              Correlation {selected.correlationId ?? "—"}
            </div>
            <div className="metric-sub">Strategy {selected.strategyId ?? "—"}</div>
            <div className="metric-sub">Instrument {selected.instrumentId ?? "—"}</div>
            <div className="metric-sub">Account {selected.accountId ?? "—"}</div>
            <p>{selected.detail}</p>
            <button type="button" className="btn" onClick={() => setSelected(null)}>
              Close
            </button>
          </aside>
        </>
      ) : null}
    </>
  );
}
