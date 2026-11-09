"use client";

import { useState } from "react";
import dynamic from "next/dynamic";
import { useQuery } from "@tanstack/react-query";
import { isProviderSelectable, marketDataApi, providerStatusLabel } from "@/lib/api/market-data";
import { PageHeader } from "@/components/layout/PageHeader";
import { MarketAnalysisChartBoundary } from "@/components/charts/MarketAnalysisChartBoundary";

// Lightweight Charts depends on browser canvas APIs. Keep it out of the server-rendered
// route bundle and load it only after the Market Data page mounts in the browser.
const MarketAnalysisChart = dynamic(
  () => import("@/components/charts/MarketAnalysisChart").then((module) => module.MarketAnalysisChart),
  {
    ssr: false,
    loading: () => <section className="panel"><h2 className="panel-title">MARKET ANALYSIS</h2><p>Initializing interactive chart…</p></section>,
  },
);

export default function MarketDataPage() {
  const [instrument, setInstrument] = useState("EURUSD");
  const [timeframe, setTimeframe] = useState("M1");
  const [providerOverride, setProviderOverride] = useState<string | null>(null);
  const [submitted, setSubmitted] = useState({ instrument: "EURUSD", timeframe: "M1", provider: "biquote-live" });

  const providers = useQuery({ queryKey: ["market-data", "providers"], queryFn: () => marketDataApi.providers() });
  const instruments = useQuery({ queryKey: ["market-data", "instruments"], queryFn: () => marketDataApi.instruments(), enabled: true });
  const provider = providerOverride ?? String(instruments.data?.find(i => i.id === instrument)?.default_provider ?? "biquote-live");
  const candles = useQuery({ queryKey: ["market-data", "candles", submitted], queryFn: () => marketDataApi.candles({ ...submitted, limit: 100 }), enabled: Boolean(submitted.instrument) });

  const error = providers.error ?? instruments.error ?? candles.error;
  const candleData = candles.data;
  return <div>
    <PageHeader title="Market Data" description="Browse market data providers, instruments, and historical price candles." />
    <MarketAnalysisChartBoundary><MarketAnalysisChart /></MarketAnalysisChartBoundary>
    {error ? <section className="panel error" role="alert"><h2>Market data could not be loaded</h2><p>{error instanceof Error ? error.message : "Unable to load market data."}</p><p>Check your market data connection or try again shortly.</p><button className="btn" onClick={() => { void providers.refetch(); void instruments.refetch(); void candles.refetch(); }}>Retry</button></section> : null}
    <section className="panel">
      <h2 className="panel-title">CANDLE QUERY</h2>
      <form onSubmit={e => { e.preventDefault(); setSubmitted({ instrument, timeframe, provider }); }} style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "end" }}>
        <label className="journal-field"><span>Instrument</span><select value={instrument} onChange={e => { setInstrument(e.target.value); setProviderOverride(null); }}>{(instruments.data ?? []).map(i => <option key={i.id} value={i.id}>{i.symbol ?? i.id}</option>)}</select></label>
        <label className="journal-field"><span>Timeframe</span><select value={timeframe} onChange={e => setTimeframe(e.target.value)}>{["M1","M5","M15","M30","H1","H4","D1","W1"].map(t => <option key={t}>{t}</option>)}</select></label>
        <label className="journal-field"><span>Provider</span><select value={provider} onChange={e => setProviderOverride(e.target.value)}>{(providers.data ?? []).filter(isProviderSelectable).map(p => <option key={p.id} value={p.id}>{p.name ?? p.id}</option>)}</select></label>
        <button className="btn primary" type="submit" disabled={providers.isLoading || instruments.isLoading}>Fetch candles</button>
      </form>
      {providers.isLoading || instruments.isLoading ? <p>Loading provider/instrument registry…</p> : null}
    </section>
    <div className="grid grid-kpi" style={{ marginTop: 12 }}>
      <section className="panel"><div className="metric"><span className="metric-label">Candles returned</span><span className="metric-value">{candles.isLoading ? "…" : candleData?.count ?? "—"}</span></div></section>
      <section className="panel"><div className="metric"><span className="metric-label">Data classification</span><span className="metric-value">{candleData?.classification ?? "—"}</span><span className="metric-sub">Inspect classification before using data</span></div></section>
      <section className="panel"><div className="metric"><span className="metric-label">Last updated (UTC)</span><span className="metric-value" style={{ fontSize: 14 }}>{candleData?.last_updated_at ?? "—"}</span><span className="metric-sub">{candleData?.freshness ? JSON.stringify(candleData.freshness) : ""}</span></div></section>
    </div>
    <section className="panel" style={{ marginTop: 12 }}><h2 className="panel-title">LATEST CANDLES</h2>
      {candles.isLoading ? <p>Fetching candles…</p> : candleData?.candles?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Time</th><th>Open</th><th>High</th><th>Low</th><th>Close</th><th>Volume</th></tr></thead><tbody>{candleData.candles.slice(-30).reverse().map((c, i) => <tr key={String(c.timestamp_ms ?? i)}><td>{new Date(c.timestamp_ms).toLocaleString()}</td><td className="num">{c.open}</td><td className="num">{c.high}</td><td className="num">{c.low}</td><td className="num">{c.close}</td><td className="num">{c.volume ?? "—"}</td></tr>)}</tbody></table></div> : !error ? <p>No candles returned for the selected instrument/provider. Provider status and classification determine whether the source is available.</p> : null}
    </section>
    <section className="panel" style={{ marginTop: 12 }}><h2 className="panel-title">PROVIDER REGISTRY</h2>{providers.data?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Provider</th><th>State</th></tr></thead><tbody>{providers.data.map(p => <tr key={p.id}><td>{p.name ?? p.id}</td><td>{providerStatusLabel(p)}</td></tr>)}</tbody></table></div> : !providers.isLoading ? <p>No providers returned.</p> : null}</section>
  </div>;
}
