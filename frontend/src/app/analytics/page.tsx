"use client";

import { useMemo, useState } from "react";

export default function AnalyticsLabPage() {
  const [pnlInput, setPnlInput] = useState("");
  const result = useMemo(() => {
    const raw = pnlInput.split(/[\s,;]+/).filter(Boolean);
    if (!raw.length) return null;
    const pnls = raw.map(Number);
    if (pnls.some((v) => !Number.isFinite(v))) return null;
    const wins = pnls.filter((v) => v > 0);
    const losses = pnls.filter((v) => v < 0);
    const net = pnls.reduce((a, b) => a + b, 0);
    const grossProfit = wins.reduce((a, b) => a + b, 0);
    const grossLoss = -losses.reduce((a, b) => a + b, 0);
    let equity = 0, peak = 0, maxDD = 0;
    for (const pnl of pnls) {
      equity += pnl;
      peak = Math.max(peak, equity);
      maxDD = Math.max(maxDD, peak - equity);
    }
    return {
      count: pnls.length,
      wins: wins.length,
      losses: losses.length,
      winRate: wins.length / pnls.length * 100,
      net,
      expectancy: net / pnls.length,
      profitFactor: grossLoss ? grossProfit / grossLoss : (grossProfit ? null : 0),
      maxDD,
    };
  }, [pnlInput]);

  const currency = (n: number) => n.toLocaleString("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 });
  return (
    <div>
      <header className="page-header">
        <div><h1>Analytics Lab</h1><p>Evaluate performance using closed-trade net P&amp;L, expectancy, profit factor, and equity drawdown.</p></div>
        <span className="badge warn">Manual sample mode</span>
      </header>
      <section className="panel">
        <h2 className="panel-title">PASTE CLOSED-TRADE NET P&amp;L (USD)</h2>
        <label className="journal-field"><span>Comma, space, or newline separated values</span>
          <textarea rows={4} value={pnlInput} onChange={(e) => setPnlInput(e.target.value)} placeholder="Example: 120, -45, 80, -30, 0" />
        </label>
        <p className="muted" style={{ fontSize: 12 }}>This sandbox calculates only the values entered here. It does not read your actual journal or account.</p>
      </section>
      <div className="grid grid-kpi" style={{ marginTop: 14 }}>
        {[
          ["Trades", result ? String(result.count) : "—"],
          ["Win rate", result ? result.winRate.toFixed(1) + "%" : "—"],
          ["Net P&L", result ? currency(result.net) : "—"],
          ["Expectancy", result ? currency(result.expectancy) : "—"],
          ["Profit factor", result ? (result.profitFactor === null ? "∞" : result.profitFactor.toFixed(2)) : "—"],
          ["Max drawdown", result ? currency(result.maxDD) : "—"],
          ["Winning trades", result ? String(result.wins) : "—"],
          ["Losing trades", result ? String(result.losses) : "—"],
        ].map(([label, value]) => <section className="panel" key={label}><div className="metric"><span className="metric-label">{label}</span><span className="metric-value">{value}</span></div></section>)}
      </div>
      <section className="panel" style={{ marginTop: 14 }}>
        <h2 className="panel-title">METRIC DEFINITIONS</h2>
        <ul className="journal-metric-list">
          <li><strong>Win rate</strong> = winning trades ÷ closed trades.</li>
          <li><strong>Expectancy</strong> = mean net P&amp;L per trade.</li>
          <li><strong>Profit factor</strong> = gross profit ÷ absolute gross loss.</li>
          <li><strong>Max drawdown</strong> = largest peak-to-trough decline in the cumulative P&amp;L series starting from zero.</li>
        </ul>
      </section>
    </div>
  );
}
