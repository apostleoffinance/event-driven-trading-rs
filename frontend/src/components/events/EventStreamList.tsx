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
    return <div className="empty">No events yet.</div>;
  }
  return (
    <div role="log" aria-live="polite">
      {rows.map((e) => (
        <div key={e.id} className="event-row">
          <time dateTime={e.timestamp}>{formatTime(e.timestamp)}</time>
          <div>
            <strong>{e.type}</strong>
            <div className="metric-sub">
              {[e.instrumentId, e.strategyId, e.accountId, e.entity]
                .filter(Boolean)
                .join(" · ")}
            </div>
            <div className="metric-sub muted">{e.detail}</div>
          </div>
        </div>
      ))}
    </div>
  );
}
