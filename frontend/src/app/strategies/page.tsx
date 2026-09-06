"use client";

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { PageHeader } from "@/components/layout/PageHeader";
import { StatusBadge } from "@/components/data-display/StatusBadge";
import { formatPct } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import type { Strategy } from "@/types/domain";

const TABS = [
  "overview",
  "signals",
  "intents",
  "trades",
  "performance",
  "risk",
  "configuration",
  "events",
] as const;

type Tab = (typeof TABS)[number];

function strategyTone(status: Strategy["status"]): "ok" | "warn" | "bad" | "info" {
  if (status === "RUNNING") return "ok";
  if (status === "ERROR") return "bad";
  if (status === "PAUSED" || status === "STOPPED") return "warn";
  return "info";
}

function TabBody({ strategy, tab }: { strategy: Strategy; tab: Tab }) {
  switch (tab) {
    case "overview":
      return (
        <>
          <p className="metric-sub muted" style={{ fontFamily: "var(--font-sans)" }}>
            {strategy.description}
          </p>
          <div className="metric-sub">
            Version {strategy.version} · {strategy.environment} · Instrument{" "}
            {strategy.instrumentId}
          </div>
          <div className="metric-sub">
            Emits TradeIntent only — never submits orders or holds secrets.
          </div>
        </>
      );
    case "signals":
      return (
        <div className="metric-sub num">Signals generated: {strategy.signals}</div>
      );
    case "intents":
      return (
        <ul className="metric-sub">
          {strategy.recentIntentIds.map((id) => (
            <li key={id} className="num">
              {id}
            </li>
          ))}
        </ul>
      );
    case "trades":
      return (
        <div className="metric-sub num">
          Fills attributed after risk/OMS: {strategy.trades}
        </div>
      );
    case "performance":
      return (
        <div className="grid" style={{ gridTemplateColumns: "repeat(3,1fr)" }}>
          <div className="metric-sub num">Return {formatPct(strategy.returnPct)}</div>
          <div className="metric-sub num">Sharpe {strategy.sharpe.toFixed(2)}</div>
          <div className="metric-sub num">
            Max DD {strategy.maxDrawdownPct.toFixed(2)}%
          </div>
          <div className="metric-sub num">
            Win {strategy.winRatePct.toFixed(1)}%
          </div>
          <div className="metric-sub num">PF {strategy.profitFactor.toFixed(2)}</div>
        </div>
      );
    case "risk":
      return (
        <div className="metric-sub">
          Strategy risk is evaluated in Rust risk-engine. UI cannot bypass limits.
        </div>
      );
    case "configuration":
      return (
        <table className="data-table">
          <thead>
            <tr>
              <th>Key</th>
              <th>Value</th>
            </tr>
          </thead>
          <tbody>
            {strategy.configuration.map((c) => (
              <tr key={c.key}>
                <td>{c.key}</td>
                <td className="num">{c.value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      );
    case "events":
      return (
        <div className="metric-sub">
          See Events page filtered by strategy id {strategy.id}.
        </div>
      );
    default:
      return null;
  }
}

export default function StrategiesPage() {
  const { data = [], isLoading, isError } = useQuery({
    queryKey: ["strategies"],
    queryFn: () => getRepositories().strategies.listStrategies(),
  });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("overview");

  if (isLoading) return <div className="loading">Loading strategies…</div>;
  if (isError) return <div className="error">Failed to load strategies.</div>;

  const selected =
    data.find((s) => s.id === selectedId) ?? data[0] ?? null;

  return (
    <>
      <PageHeader
        title="Strategies"
        description="Python strategies emit TradeIntent only — they never submit orders. Execution stays in Rust."
      />
      <div
        className="grid"
        style={{ gridTemplateColumns: "minmax(280px,340px) 1fr", gap: 12 }}
      >
        <div className="grid" style={{ gap: 8 }}>
          {data.map((s) => (
            <button
              key={s.id}
              type="button"
              className={`panel${selected?.id === s.id ? " primary" : ""}`}
              style={{
                textAlign: "left",
                borderColor:
                  selected?.id === s.id
                    ? "color-mix(in srgb, var(--accent) 40%, transparent)"
                    : undefined,
                background:
                  selected?.id === s.id ? "var(--accent-soft)" : undefined,
              }}
              onClick={() => {
                setSelectedId(s.id);
                setTab("overview");
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  gap: 8,
                }}
              >
                <strong style={{ fontWeight: 500 }}>{s.name}</strong>
                <StatusBadge tone={strategyTone(s.status)}>{s.status}</StatusBadge>
              </div>
              <div className="metric-sub">
                {s.version} · {s.environment}
              </div>
              <div className="metric-sub num">
                {formatPct(s.returnPct)} · Sharpe {s.sharpe.toFixed(2)}
              </div>
            </button>
          ))}
        </div>
        {selected ? (
          <div className="panel">
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                gap: 8,
                marginBottom: 8,
              }}
            >
              <h2 style={{ margin: 0, fontSize: 16, fontWeight: 500 }}>
                {selected.name}
              </h2>
              <StatusBadge tone={strategyTone(selected.status)}>
                {selected.status}
              </StatusBadge>
            </div>
            <div className="tabs" role="tablist">
              {TABS.map((t) => (
                <button
                  key={t}
                  type="button"
                  role="tab"
                  aria-selected={tab === t}
                  className={`tab${tab === t ? " active" : ""}`}
                  onClick={() => setTab(t)}
                >
                  {t}
                </button>
              ))}
            </div>
            <TabBody strategy={selected} tab={tab} />
          </div>
        ) : (
          <div className="empty">No strategies.</div>
        )}
      </div>
    </>
  );
}
