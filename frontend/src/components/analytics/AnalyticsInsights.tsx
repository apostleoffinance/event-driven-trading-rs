"use client";

import type { HourlyAnalyticsGroup } from "./HourlyPerformanceHeatmap";

type InsightGroup = HourlyAnalyticsGroup;

function money(value: number, currency: string) {
  return value.toLocaleString(undefined, { style: "currency", currency, maximumFractionDigits: 2 });
}

function strongest(groups: InsightGroup[]) {
  return groups.filter(group => Number.isFinite(Number(group.net_pnl)) && group.trades >= 3)
    .sort((a, b) => Number(b.net_pnl) - Number(a.net_pnl))[0] ?? null;
}

function weakest(groups: InsightGroup[]) {
  return groups.filter(group => Number.isFinite(Number(group.net_pnl)) && group.trades >= 3)
    .sort((a, b) => Number(a.net_pnl) - Number(b.net_pnl))[0] ?? null;
}

export function AnalyticsInsights({ symbols, sessions, setups, currency, onSelectGroup }: {
  symbols: InsightGroup[];
  sessions: InsightGroup[];
  setups: InsightGroup[];
  currency: string;
  onSelectGroup: (dimension: "symbol" | "session" | "setup", key: string) => void;
}) {
  const candidates = [
    { label: "Instrument", group: strongest(symbols) },
    { label: "Session", group: strongest(sessions) },
    { label: "Setup", group: strongest(setups) }
  ].filter(item => item.group);
  const risks = [
    { label: "Instrument", group: weakest(symbols) },
    { label: "Session", group: weakest(sessions) },
    { label: "Setup", group: weakest(setups) }
  ].filter(item => item.group && Number(item.group.net_pnl) < 0);

  return <section className="panel analytics-insights" aria-labelledby="analytics-insights-title">
    <div className="analytics-insights-heading">
      <div><span className="analytics-eyebrow">RESEARCH PROMPTS</span><h2 id="analytics-insights-title">What stands out in this sample</h2>
        <p className="muted">Descriptive comparisons only. Groups with fewer than three trades are excluded from these observations.</p>
      </div>
      <span className="badge info">Sample-aware</span>
    </div>
    <div className="analytics-insights-grid">
      <div className="analytics-insight-column">
        <h3>Strongest observed groups</h3>
        {candidates.length ? candidates.map(item => <div className="analytics-insight-row" key={item.label}>
          <span>{item.label}</span><button type="button" className="analytics-insight-select" onClick={() => onSelectGroup(item.label.toLowerCase() as "symbol" | "session" | "setup", item.group!.key)}>{item.group!.key}</button><b className={Number(item.group!.net_pnl) >= 0 ? "pos" : "neg"}>{money(Number(item.group!.net_pnl), currency)}</b>
          <small>{item.group!.trades} trades · {Number(item.group!.win_rate_percent).toFixed(1)}% win rate</small>
        </div>) : <p className="muted">Not enough qualifying trades to identify a strongest group.</p>}
      </div>
      <div className="analytics-insight-column">
        <h3>Groups worth investigating</h3>
        {risks.length ? risks.map(item => <div className="analytics-insight-row" key={item.label}>
          <span>{item.label}</span><button type="button" className="analytics-insight-select" onClick={() => onSelectGroup(item.label.toLowerCase() as "symbol" | "session" | "setup", item.group!.key)}>{item.group!.key}</button><b className="neg">{money(Number(item.group!.net_pnl), currency)}</b>
          <small>{item.group!.trades} trades · {Number(item.group!.win_rate_percent).toFixed(1)}% win rate</small>
        </div>) : <p className="muted">No negative group with at least three trades in this sample.</p>}
      </div>
    </div>
  </section>;
}
