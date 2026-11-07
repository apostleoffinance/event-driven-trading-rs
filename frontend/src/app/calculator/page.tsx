"use client";

import { useMemo, useState } from "react";

function money(value: number, currency: string) {
  return value.toLocaleString("en-US", { style: "currency", currency, maximumFractionDigits: 2 });
}

export default function CalculatorPage() {
  const [balance, setBalance] = useState("10000");
  const [currency, setCurrency] = useState("USD");
  const [instrumentPreset, setInstrumentPreset] = useState("fx_usd_quote");
  const [riskPct, setRiskPct] = useState("1");
  const [entry, setEntry] = useState("1.0850");
  const [stop, setStop] = useState("1.0800");
  const [valuePerPriceUnit, setValuePerPriceUnit] = useState("100000");
  const [step, setStep] = useState("0.01");

  const result = useMemo(() => {
    const b = Number(balance), r = Number(riskPct), e = Number(entry), s = Number(stop);
    const v = Number(valuePerPriceUnit), qStep = Number(step);
    if (![b, r, e, s, v, qStep].every(Number.isFinite) || b <= 0 || r <= 0 || r > 100 || e <= 0 || s <= 0 || e === s || v <= 0 || qStep <= 0) return null;
    const budget = b * r / 100;
    const distance = Math.abs(e - s);
    const raw = budget / (distance * v);
    const quantity = Math.floor((raw + Number.EPSILON) / qStep) * qStep;
    return { budget, distance, raw, quantity, risk: quantity * distance * v };
  }, [balance, riskPct, entry, stop, valuePerPriceUnit, step]);

  function applyPreset(value: string) { setInstrumentPreset(value); if (value === "fx_usd_quote") { setValuePerPriceUnit("100000"); setStep("0.01"); } else if (value === "crypto_spot") { setValuePerPriceUnit("1"); setStep("0.001"); } else if (value === "index_cfd") { setValuePerPriceUnit("1"); setStep("1"); } }

  const fields = [
    ["Account balance (" + currency + ")", balance, setBalance],
    ["Risk (%)", riskPct, setRiskPct],
    ["Entry price", entry, setEntry],
    ["Stop loss price", stop, setStop],
    ["Value per price unit per lot", valuePerPriceUnit, setValuePerPriceUnit],
    ["Quantity step", step, setStep],
  ] as const;

  return (
    <div>
      <header className="page-header">
        <div><h1>Position Size Calculator</h1><p>Estimate quantity from account risk, stop distance, and instrument contract value.</p></div>
        <span className="badge warn">Planning tool · not execution</span>
      </header>
      <div className="grid grid-dash">
        <section className="panel">
          <h2 className="panel-title">INPUTS</h2>
          <div className="journal-form-grid">
            {fields.map(([label, value, setter]) => (
              <label className="journal-field" key={label}>
                <span>{label}</span>
                <input type="number" min="0" step="any" value={value} onChange={(e) => setter(e.target.value)} />
              </label>
            ))}
          </div>
          <p className="muted" style={{ marginTop: 12, fontSize: 12 }}>Presets are calculation starting points only. FX preset assumes a standard 100,000-unit lot and USD quote currency; crypto spot uses 1 base unit; CFD values are broker-specific. For non-USD quote currencies, account currency conversion is not automated. Verify tick/contract value and lot step with your broker before trading.</p>
        </section>
        <section className="panel">
          <h2 className="panel-title">ESTIMATED RESULT</h2>
          {result ? <>
            <div className="metric"><span className="metric-label">Risk budget</span><span className="metric-value">{money(result.budget, currency)}</span></div>
            <div className="journal-result-grid">
              <div><span className="metric-label">Stop distance</span><div className="metric-value">{result.distance.toPrecision(6)}</div></div>
              <div><span className="metric-label">Raw quantity</span><div className="metric-value">{result.raw.toFixed(6)}</div></div>
              <div><span className="metric-label">Step-rounded size</span><div className="metric-value pos">{result.quantity.toFixed(4)}</div></div>
              <div><span className="metric-label">Estimated risk</span><div className="metric-value">{money(result.risk, currency)}</div></div>
            </div>
          </> : <p className="muted">Enter valid positive values. Entry and stop must differ; risk must be 0–100%.</p>}
          <div className="badge warn" style={{ marginTop: 16 }}>This calculator is not connected to an order ticket or risk engine.</div>
        </section>
      </div>
    </div>
  );
}
