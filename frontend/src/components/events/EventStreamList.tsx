import { formatTime } from "@/lib/format";
import type { TradingEvent } from "@/types/domain";

export function EventStreamList({
  events,
  limit = 12,
}: {
  events: TradingEvent[];
  limit?: number;
}) {
  const rows = events.slice(0, limit);
  if (rows.length === 0) {
    return <div className="empty">No recent trading activity.</div>;
  }
  const eventLabel = (type: string) => type.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/^(Order|Risk|Market Data|Strategy Signal|Trade Intent|Position|Account|Reconciliation)/, "$1");
  return (
    <div role="log" aria-live="polite">
      {rows.map((e) => (
        <div key={e.id} className="event-row">
          <time dateTime={e.timestamp}>{formatTime(e.timestamp)}</time>
          <div>
            <strong>{eventLabel(e.type)}</strong>
            <div className="metric-sub">
              {e.instrumentId ?? "Trading activity"}
            </div>
            <div className="metric-sub muted">{e.detail}</div>
          </div>
        </div>
      ))}
    </div>
  );
}
