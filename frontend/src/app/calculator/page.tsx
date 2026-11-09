"use client";

import Link from "next/link";
import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { journalApi } from "@/lib/api/journal";
import { hasSession } from "@/lib/auth/session";

const symbols = [
  { symbol: "EURUSD", kind: "FX · USD quote", contract: "100000", step: "0.01", pip: "0.0001" },
  { symbol: "GBPUSD", kind: "FX · USD quote", contract: "100000", step: "0.01", pip: "0.0001" },
  { symbol: "AUDUSD", kind: "FX · USD quote", contract: "100000", step: "0.01", pip: "0.0001" },
  { symbol: "NZDUSD", kind: "FX · USD quote", contract: "100000", step: "0.01", pip: "0.0001" },
  { symbol: "USDJPY", kind: "FX · JPY quote", contract: "100000", step: "0.01", pip: "0.01" },
  { symbol: "USDCHF", kind: "FX · CHF quote", contract: "100000", step: "0.01", pip: "0.0001" },
  { symbol: "USDCAD", kind: "FX · CAD quote", contract: "100000", step: "0.01", pip: "0.0001" },
  { symbol: "XAUUSD", kind: "Gold · broker contract", contract: "100", step: "0.01", pip: "0.01" },
  { symbol: "BTCUSD", kind: "Crypto · broker contract", contract: "1", step: "0.001", pip: "1" },
  { symbol: "US100", kind: "Index CFD · broker contract", contract: "1", step: "1", pip: "1" },
  { symbol: "CUSTOM", kind: "Custom contract", contract: "1", step: "0.01", pip: "0.01" },
];

function money(value: number, currency: string) {
  return value.toLocaleString("en-US", { style: "currency", currency, maximumFractionDigits: 2 });
}

export default function CalculatorPage() {
  const [symbol, setSymbol] = useState("EURUSD");
  const [direction, setDirection] = useState<"long" | "short">("long");
  const [currency, setCurrency] = useState("USD");
  const [accountId, setAccountId] = useState("");
  const [balance, setBalance] = useState("10000");
  const [riskPct, setRiskPct] = useState("1");
  const [entry, setEntry] = useState("1.0850");
  const [stop, setStop] = useState("1.0800");
  const [contractValue, setContractValue] = useState("100000");
  const [step, setStep] = useState("0.01");
  const [pipSize, setPipSize] = useState("0.0001");
  const [customContract, setCustomContract] = useState(false);
  const [auth, setAuth] = useState(false);

  useMemo(() => { if (typeof window !== "undefined") setAuth(hasSession()); return null; }, []);
  const accounts = useQuery({ queryKey: ["journal", "accounts"], queryFn: journalApi.listAccounts, enabled: auth });
  const activeSymbol = symbols.find(x => x.symbol === symbol) ?? symbols[0];

  function selectSymbol(value: string) {
    setSymbol(value);
    const preset = symbols.find(x => x.symbol === value);
    if (preset && value !== "CUSTOM") {
      setContractValue(preset.contract); setStep(preset.step); setPipSize(preset.pip); setCustomContract(false);
    } else setCustomContract(true);
    if (value === "USDJPY") { setEntry("155.000"); setStop("154.500"); }
    else if (value === "XAUUSD") { setEntry("2700.00"); setStop("2695.00"); }
    else if (value === "BTCUSD") { setEntry("80000"); setStop("79000"); }
    else if (value === "US100") { setEntry("20000"); setStop("19950"); }
    else if (value !== "CUSTOM") { setEntry("1.0850"); setStop("1.0800"); }
  }

  const result = useMemo(() => {
    const b=Number(balance), r=Number(riskPct), e=Number(entry), s=Number(stop), v=Number(contractValue), qStep=Number(step), pip=Number(pipSize);
    if (![b,r,e,s,v,qStep,pip].every(Number.isFinite)||b<=0||r<=0||r>100||e<=0||s<=0||e===s||v<=0||qStep<=0||pip<=0) return null;
    if (direction==="long"&&s>=e || direction==="short"&&s<=e) return null;
    const budget=b*r/100, distance=Math.abs(e-s), raw=budget/(distance*v);
    const quantity=Math.floor((raw+Number.EPSILON)/qStep)*qStep;
    const risk=quantity*distance*v;
    return {budget,distance,pips:distance/pip,raw,quantity,risk};
  },[balance,riskPct,entry,stop,contractValue,step,pipSize,direction]);

  const journalHref = result && result.quantity > 0
    ? `/journal?newTrade=1&symbol=${encodeURIComponent(symbol)}&direction=${direction}&entry=${encodeURIComponent(entry)}&stop=${encodeURIComponent(stop)}&quantity=${encodeURIComponent(String(Number(result.quantity.toFixed(8))))}&risk=${encodeURIComponent(String(Number(result.risk.toFixed(2))))}&account=${encodeURIComponent(accountId)}&currency=${encodeURIComponent(currency)}`
    : "/journal";

  return <div>
    <header className="page-header"><div><h1>Position Size Calculator</h1><p>Plan position size from instrument, account equity, risk tolerance and stop-loss distance.</p></div><span className="badge warn">Planning only · no order execution</span></header>
    <div className="grid grid-dash calculator-layout">
      <section className="panel">
        <h2 className="panel-title">TRADE PARAMETERS</h2>
        <div className="journal-form-grid">
          <label className="journal-field"><span>Instrument / symbol</span><select value={symbol} onChange={e=>selectSymbol(e.target.value)}>{symbols.map(x=><option key={x.symbol} value={x.symbol}>{x.symbol} · {x.kind}</option>)}</select></label>
          <label className="journal-field"><span>Direction</span><select value={direction} onChange={e=>setDirection(e.target.value as "long"|"short")}><option value="long">Buy / Long</option><option value="short">Sell / Short</option></select></label>
          <label className="journal-field"><span>Account currency</span><select value={currency} onChange={e=>setCurrency(e.target.value)}>{["USD","EUR","GBP","NGN","CHF","JPY","USDT"].map(x=><option key={x}>{x}</option>)}</select></label>
          <label className="journal-field"><span>Account balance</span><input type="number" min="0" step="any" value={balance} onChange={e=>setBalance(e.target.value)}/></label>
          <label className="journal-field"><span>Risk per trade (%)</span><input type="number" min="0" max="100" step="any" value={riskPct} onChange={e=>setRiskPct(e.target.value)}/></label>
          <label className="journal-field"><span>Entry price</span><input type="number" min="0" step="any" value={entry} onChange={e=>setEntry(e.target.value)}/></label>
          <label className="journal-field"><span>Stop-loss price</span><input type="number" min="0" step="any" value={stop} onChange={e=>setStop(e.target.value)}/></label>
          <label className="journal-field"><span>Contract value per price unit (account currency)</span><input type="number" min="0" step="any" value={contractValue} onChange={e=>setContractValue(e.target.value)} onFocus={()=>setCustomContract(true)}/></label>
          <label className="journal-field"><span>Quantity / lot step</span><input type="number" min="0" step="any" value={step} onChange={e=>setStep(e.target.value)}/></label>
          <label className="journal-field"><span>Pip / point size</span><input type="number" min="0" step="any" value={pipSize} onChange={e=>setPipSize(e.target.value)}/></label>
        </div>
        <p className="muted" style={{marginTop:12,fontSize:12}}>Instrument presets are editable assumptions, not broker specifications. Contract value must be expressed in your account currency for one full quantity unit per 1.0 price move. For non-USD quote pairs (e.g. USDJPY), CFDs, metals and crypto, confirm contract/tick value and conversion with your broker. If conversion is not supplied, results are estimates.</p>
      </section>
      <section className="panel calculator-result">
        <h2 className="panel-title">POSITION SIZE & RISK</h2>
        {result ? result.quantity>0 ? <>
          <div className="metric"><span className="metric-label">Risk budget</span><span className="metric-value">{money(result.budget,currency)}</span></div>
          <div className="journal-result-grid">
            <div><span className="metric-label">Stop distance</span><div className="metric-value">{result.distance.toLocaleString(undefined,{maximumFractionDigits:8})}</div></div>
            <div><span className="metric-label">Pips / points</span><div className="metric-value">{result.pips.toLocaleString(undefined,{maximumFractionDigits:2})}</div></div>
            <div><span className="metric-label">Unrounded quantity</span><div className="metric-value">{result.raw.toLocaleString(undefined,{maximumFractionDigits:8})}</div></div>
            <div><span className="metric-label">Step-adjusted size</span><div className="metric-value pos">{result.quantity.toLocaleString(undefined,{maximumFractionDigits:8})}</div></div>
            <div><span className="metric-label">Estimated risk</span><div className="metric-value">{money(result.risk,currency)}</div></div>
            <div><span className="metric-label">Risk used</span><div className="metric-value">{(result.risk/Number(balance)*100).toFixed(3)}%</div></div>
          </div>
          <div style={{display:"grid",gap:8,marginTop:18}}>
            <label className="journal-field"><span>Journal account (optional)</span><select value={accountId} onChange={e=>setAccountId(e.target.value)}><option value="">Choose account later</option>{accounts.data?.filter(a=>!a.archived).map(a=><option key={a.id} value={a.id}>{a.name} · {a.currency}</option>)}</select></label>
            <Link className="btn primary" href={journalHref}>Add planned trade to Journal</Link>
          </div>
        </> : <p className="badge warn">Risk budget is smaller than the minimum quantity step. Reduce step size or increase risk budget.</p> : <p className="error">Check inputs: positive values are required, and stop loss must be below entry for Long or above entry for Short.</p>}
        <p className="muted" style={{marginTop:16,fontSize:12}}>Calculation: risk budget ÷ (absolute entry-to-stop distance × contract value), rounded down to the configured quantity step. No broker order is placed.</p>
      </section>
    </div>
  </div>;
}
