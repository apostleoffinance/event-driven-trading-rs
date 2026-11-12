"use client";

import type { HourlyAnalyticsGroup } from "./HourlyPerformanceHeatmap";

type InsightGroup = HourlyAnalyticsGroup;
type Dimension = "symbol" | "session" | "setup";

function money(value: number, currency: string) {
  return value.toLocaleString(undefined, { style: "currency", currency, maximumFractionDigits: 2 });
}

function sampleLabel(trades: number) {
  if (trades <= 5) return { label: "Limited sample", tone: "limited" };
  if (trades < 20) return { label: "Developing sample", tone: "developing" };
  return { label: "Larger sample", tone: "larger" };
}

function ranked(groups: InsightGroup[], order: "best" | "worst") {
  return groups
    .filter(group => Number.isFinite(Number(group.net_pnl)) && Number(group.trades) >= 3)
    .slice()
    .sort((a, b) => order === "best"
      ? Number(b.net_pnl) - Number(a.net_pnl)
      : Number(a.net_pnl) - Number(b.net_pnl))[0] ?? null;
}

function InsightRow({ label, group, currency, rank, scale, onSelect }: {
  label: string;
  group: InsightGroup;
  currency: string;
  rank: string;
  scale: number;
  onSelect: () => void;
}) {
  const pnl = Number(group.net_pnl);
  const winRate = Math.max(0, Math.min(100, Number(group.win_rate_percent) || 0));
  const sample = sampleLabel(Number(group.trades));
  const tone = pnl >= 0 ? "positive" : "negative";
  const barWidth = Math.max(1.5, Math.abs(pnl) / Math.max(1, scale) * 50);
  return <article className="analytics-insight-item">
    <div className="analytics-insight-item-head">
      <span className="analytics-insight-dimension">{label}</span>
      <span className="analytics-insight-rank">{rank}</span>
      <strong className={tone}>{money(pnl, currency)}</strong>
    </div>
    <button type="button" className="analytics-insight-select" onClick={onSelect} aria-label={`Investigate ${group.key} ${label.toLowerCase()} group`}>
      {group.key}<span aria-hidden="true"> ↗</span>
    </button>
    <div className="analytics-insight-meta">
      <span>{group.trades} trades</span><span>{winRate.toFixed(1)}% win rate</span>
      <span className={`analytics-sample-cue ${sample.tone}`}>{sample.label}</span>
    </div>
    <div className="analytics-insight-winrate" role="progressbar" aria-label={`${group.key} win rate`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={winRate}>
      <span style={{ width: `${winRate}%` }} />
    </div>
    <div className="analytics-insight-pnl-track" aria-label={`Net P&L ${money(pnl, currency)}`}>
      <span className="analytics-insight-zero" />
      <span className={`analytics-insight-pnl-bar ${tone}`} style={{
        width: `${barWidth}%`,
        [pnl >= 0 ? "left" : "right"]: "50%"
      }} />
    </div>
  </article>;
}

export function AnalyticsInsights({ symbols, sessions, setups, currency, onSelectGroup }: {
  symbols: InsightGroup[];
  sessions: InsightGroup[];
  setups: InsightGroup[];
  currency: string;
  onSelectGroup: (dimension: Dimension, key: string) => void;
}) {
  const candidates = [
    { label: "Instrument", dimension: "symbol" as const, group: ranked(symbols, "best"), rank: "Best instrument by net P&L" },
    { label: "Session", dimension: "session" as const, group: ranked(sessions, "best"), rank: "Best session by net P&L" },
    { label: "Setup", dimension: "setup" as const, group: ranked(setups, "best"), rank: "Best setup by net P&L" }
  ].filter(item => item.group);
  const risks = [
    { label: "Instrument", dimension: "symbol" as const, group: ranked(symbols, "worst"), rank: "Weakest instrument by net P&L" },
    { label: "Session", dimension: "session" as const, group: ranked(sessions, "worst"), rank: "Weakest session by net P&L" },
    { label: "Setup", dimension: "setup" as const, group: ranked(setups, "worst"), rank: "Weakest setup by net P&L" }
  ].filter(item => item.group && Number(item.group.net_pnl) < 0);

  const scale = Math.max(1, ...[...symbols, ...sessions, ...setups].filter(group => Number(group.trades) >= 3).map(group => Math.abs(Number(group.net_pnl) || 0)));

  return <section className="panel analytics-insights" aria-labelledby="analytics-insights-title">
    <div className="analytics-insights-heading">
      <div><span className="analytics-eyebrow">RESEARCH PROMPTS</span><h2 id="analytics-insights-title">What stands out in this sample</h2>
        <p className="muted">Descriptive comparisons, ranked by net P&amp;L. Groups with fewer than three trades are excluded; sample labels describe count, not statistical confidence.</p>
      </div>
      <span className="badge info">Sample-aware</span>
    </div>
    <div className="analytics-insights-grid">
      <section className="analytics-insight-column" aria-label="Top performing groups">
        <h3>Strongest observed groups</h3>
        {candidates.length ? candidates.map(item => <InsightRow key={item.label} label={item.label} group={item.group!} currency={currency} rank={item.rank} scale={scale} onSelect={() => onSelectGroup(item.dimension, item.group!.key)} />) : <p className="muted">Not enough qualifying trades to identify a strongest group.</p>}
      </section>
      <section className="analytics-insight-column" aria-label="Groups worth investigating">
        <h3>Groups worth investigating</h3>
        {risks.length ? risks.map(item => <InsightRow key={item.label} label={item.label} group={item.group!} currency={currency} rank={item.rank} scale={scale} onSelect={() => onSelectGroup(item.dimension, item.group!.key)} />) : <p className="muted">No negative group with at least three trades in this sample.</p>}
      </section>
    </div>
    <p className="analytics-insight-legend"><span><i className="positive" /> Positive net P&amp;L</span><span><i className="negative" /> Negative net P&amp;L</span><span>Win-rate bars are scaled from 0–100%</span></p>
  </section>;
}
