"use client";

import { useState } from "react";
import dynamic from "next/dynamic";
import { useQuery } from "@tanstack/react-query";
import { frontendConfig } from "@/config/frontend";
import { marketDataApi } from "@/lib/api/market-data";
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
  const [timeframe, setTimeframe] = useState("1m");
  const [provider, setProvider] = useState("mt5-bridge");
  const [submitted, setSubmitted] = useState({ instrument: "EURUSD", timeframe: "1m", provider: "mt5-bridge" });

  const providers = useQuery({ queryKey: ["market-data", "providers"], queryFn: () => marketDataApi.providers(), enabled: frontendConfig.dataMode === "api" });
  const instruments = useQuery({ queryKey: ["market-data", "instruments"], queryFn: () => marketDataApi.instruments(), enabled: frontendConfig.dataMode === "api" });
  const candles = useQuery({ queryKey: ["market-data", "candles", submitted], queryFn: () => marketDataApi.candles({ ...submitted, limit: 100 }), enabled: frontendConfig.dataMode === "api" && Boolean(submitted.instrument) });

  if (frontendConfig.dataMode !== "api") {
    return <div><PageHeader title="Market Data" description="Explore market prices and historical data." /><section className="panel"><h2>Market data is temporarily unavailable</h2><p>Please try again later. We’ll show verified market data when the service is connected.</p></section></div>;
  }

  const error = providers.error ?? instruments.error ?? candles.error;
  const candleData = candles.data;
  return <div>
    <PageHeader title="Market Data" description="Read-only provider registry, instrument metadata, and candle retrieval from Rust API." />
    <MarketAnalysisChartBoundary><MarketAnalysisChart /></MarketAnalysisChartBoundary>
    {error ? <section className="panel error" role="alert"><h2>Market Data API error</h2><p>{error instanceof Error ? error.message : "Unable to load market data."}</p><p>Verify service URL, provider configuration, CORS, and service readiness.</p><button className="btn" onClick={() => { void providers.refetch(); void instruments.refetch(); void candles.refetch(); }}>Retry</button></section> : null}
    <section className="panel">
      <h2 className="panel-title">CANDLE QUERY</h2>
      <form onSubmit={e => { e.preventDefault(); setSubmitted({ instrument, timeframe, provider }); }} style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "end" }}>
        <label className="journal-field"><span>Instrument</span><select value={instrument} onChange={e => setInstrument(e.target.value)}>{(instruments.data ?? []).map(i => <option key={i.id} value={i.id}>{i.symbol ?? i.id}</option>)}</select></label>
        <label className="journal-field"><span>Timeframe</span><select value={timeframe} onChange={e => setTimeframe(e.target.value)}>{["M1","M5","M15","M30","H1","H4","D1","W1"].map(t => <option key={t}>{t}</option>)}</select></label>
        <label className="journal-field"><span>Provider</span><select value={provider} onChange={e => setProvider(e.target.value)}>{(providers.data ?? []).map(p => <option key={p.id} value={p.id}>{p.name ?? p.id}</option>)}</select></label>
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
    <section className="panel" style={{ marginTop: 12 }}><h2 className="panel-title">PROVIDER REGISTRY</h2>{providers.data?.length ? <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Provider</th><th>State</th></tr></thead><tbody>{providers.data.map(p => <tr key={p.id}><td>{p.name ?? p.id}</td><td>{p.status ? JSON.stringify(p.status) : "—"}</td></tr>)}</tbody></table></div> : !providers.isLoading ? <p>No providers returned.</p> : null}</section>
  </div>;
}
