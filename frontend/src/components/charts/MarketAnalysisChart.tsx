"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { createChart, type IChartApi, type ISeriesApi, type Time, type IPriceLine } from "lightweight-charts";
import { useQuery } from "@tanstack/react-query";
import { frontendConfig } from "@/config/frontend";
import { marketDataApi } from "@/lib/api/market-data";

const FRAMES = ["M1", "M5", "M15", "M30", "H1", "H4", "D1", "W1"] as const;
type Frame = (typeof FRAMES)[number];

function movingAverage(values: { time: Time; close: number }[], period: number) {
  return values.map((item, index) => {
    const window = values.slice(Math.max(0, index - period + 1), index + 1);
    return { time: item.time, value: window.reduce((sum, candle) => sum + candle.close, 0) / window.length };
  }).filter((_, index) => index >= period - 1);
}

export function MarketAnalysisChart() {
  const [symbol, setSymbol] = useState("EURUSD");
  const [frame, setFrame] = useState<Frame>("H1");
  const [provider, setProvider] = useState("mt5-bridge");
  const [showVolume, setShowVolume] = useState(true);
  const [showSma, setShowSma] = useState(false);
  const [sl, setSl] = useState("");
  const [tp, setTp] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candleRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const volumeRef = useRef<ISeriesApi<"Histogram"> | null>(null);
  const smaRef = useRef<ISeriesApi<"Line"> | null>(null);
  const slLineRef = useRef<IPriceLine | null>(null);
  const tpLineRef = useRef<IPriceLine | null>(null);

  const providers = useQuery({ queryKey: ["market-analysis", "providers"], queryFn: () => marketDataApi.providers(), enabled: frontendConfig.dataMode === "api" });
  const instruments = useQuery({ queryKey: ["market-analysis", "instruments"], queryFn: () => marketDataApi.instruments(), enabled: frontendConfig.dataMode === "api" });
  const selectedProvider = providers.data?.some(p => p.id === provider) ? provider : providers.data?.[0]?.id ?? provider;
  const selectedSymbol = instruments.data?.some(i => i.id === symbol) ? symbol : instruments.data?.[0]?.id ?? symbol;
  const candles = useQuery({ queryKey: ["market-analysis", "candles", selectedSymbol, frame, selectedProvider], queryFn: () => marketDataApi.candles({ instrument: selectedSymbol, timeframe: frame, provider: selectedProvider, limit: 500 }), enabled: frontendConfig.dataMode === "api" && Boolean(selectedSymbol && selectedProvider) });

  const chartData = useMemo(() => (candles.data?.candles ?? []).filter(c => Number.isFinite(c.timestamp_ms) && [c.open,c.high,c.low,c.close].every(Number.isFinite)).sort((a,b)=>a.timestamp_ms-b.timestamp_ms).map(c => ({ time: Math.floor(c.timestamp_ms / 1000) as Time, open:c.open, high:c.high, low:c.low, close:c.close, volume:c.volume ?? 0 })), [candles.data]);
  const latest = chartData.at(-1);

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    const chart = createChart(node, {
      width: node.clientWidth, height: 480,
      layout: { background: { color: "transparent" }, textColor: "var(--foreground, #d1d4dc)" },
      grid: { vertLines: { color: "rgba(130,130,130,.12)" }, horzLines: { color: "rgba(130,130,130,.12)" } },
      rightPriceScale: { borderColor: "rgba(130,130,130,.25)" },
      timeScale: { borderColor: "rgba(130,130,130,.25)", timeVisible: true, secondsVisible: false },
      crosshair: { mode: 1 },
    });
    const candlesSeries = chart.addCandlestickSeries( { upColor: "#26a69a", downColor: "#ef5350", borderUpColor: "#26a69a", borderDownColor: "#ef5350", wickUpColor: "#26a69a", wickDownColor: "#ef5350" });
    const volumeSeries = chart.addHistogramSeries( { priceFormat: { type: "volume" }, priceScaleId: "volume" });
    chart.priceScale("volume").applyOptions({ scaleMargins: { top: 0.82, bottom: 0 } });
    const smaSeries = chart.addLineSeries( { color: "#f59e0b", lineWidth: 2, priceLineVisible: false, lastValueVisible: false });
    chartRef.current = chart; candleRef.current = candlesSeries; volumeRef.current = volumeSeries; smaRef.current = smaSeries;
    const resize = new ResizeObserver(entries => { const width = entries[0]?.contentRect.width; if (width) chart.applyOptions({ width }); });
    resize.observe(node);
    return () => { resize.disconnect(); chart.remove(); chartRef.current = null; candleRef.current = null; volumeRef.current = null; smaRef.current = null; };
  }, []);

  useEffect(() => {
    candleRef.current?.setData(chartData.map(c => ({ time:c.time, open:c.open, high:c.high, low:c.low, close:c.close })));
    volumeRef.current?.setData(chartData.map(c => ({ time:c.time, value:c.volume, color:c.close >= c.open ? "rgba(38,166,154,.35)" : "rgba(239,83,80,.35)" })));
    smaRef.current?.setData(showSma ? movingAverage(chartData, 20) : []);
    chartRef.current?.timeScale().fitContent();
  }, [chartData, showSma, showVolume]);

  useEffect(() => {
    volumeRef.current?.applyOptions({ visible: showVolume });
  }, [showVolume]);

  useEffect(() => {
    const series = candleRef.current;
    if (!series) return;
    if (slLineRef.current) series.removePriceLine(slLineRef.current);
    if (tpLineRef.current) series.removePriceLine(tpLineRef.current);
    slLineRef.current = null; tpLineRef.current = null;
    const stop = Number(sl), target = Number(tp);
    if (sl.trim() && Number.isFinite(stop) && stop > 0) slLineRef.current = series.createPriceLine({ price: stop, color: "#ef5350", lineWidth: 2, lineStyle: 2, axisLabelVisible: true, title: "SL" });
    if (tp.trim() && Number.isFinite(target) && target > 0) tpLineRef.current = series.createPriceLine({ price: target, color: "#26a69a", lineWidth: 2, lineStyle: 2, axisLabelVisible: true, title: "TP" });
  }, [sl, tp, chartData]);

  if (frontendConfig.dataMode !== "api") return <section className="panel"><h2 className="panel-title">MARKET ANALYSIS CHART</h2><p>Connect the Market Data API to render real OHLC candles. No synthetic candles are shown.</p></section>;

  return <section className="panel market-analysis">
    <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",gap:12,flexWrap:"wrap"}}>
      <div><h2 className="panel-title" style={{marginBottom:4}}>MARKET ANALYSIS</h2><p style={{margin:0,fontSize:12,opacity:.7}}>Powered by TradingView Lightweight Charts · Read-only analysis</p></div>
      <div style={{display:"flex",gap:8,flexWrap:"wrap"}}><label className="journal-field"><span>Instrument</span><select value={selectedSymbol} onChange={e=>setSymbol(e.target.value)}>{(instruments.data??[]).map(i=><option key={i.id} value={i.id}>{i.symbol??i.id}</option>)}</select></label><label className="journal-field"><span>Provider</span><select value={selectedProvider} onChange={e=>setProvider(e.target.value)}>{(providers.data??[]).map(p=><option key={p.id} value={p.id}>{p.name??p.id}</option>)}</select></label></div>
    </div>
    <div style={{display:"flex",gap:5,flexWrap:"wrap",margin:"14px 0"}}>{FRAMES.map(f=><button key={f} className={frame===f?"btn primary":"btn"} onClick={()=>setFrame(f)}>{f}</button>)}<span style={{flex:1}}/><label style={{display:"flex",alignItems:"center",gap:5,fontSize:12}}><input type="checkbox" checked={showVolume} onChange={e=>setShowVolume(e.target.checked)}/> Volume</label><label style={{display:"flex",alignItems:"center",gap:5,fontSize:12}}><input type="checkbox" checked={showSma} onChange={e=>setShowSma(e.target.checked)}/> SMA 20</label></div>
    {latest&&<div style={{display:"flex",gap:14,flexWrap:"wrap",fontSize:12,marginBottom:8}}><strong>{selectedSymbol}</strong><span>O {latest.open}</span><span>H {latest.high}</span><span>L {latest.low}</span><span style={{color:latest.close>=latest.open?"#26a69a":"#ef5350"}}>C {latest.close}</span><span>{candles.data?.classification??"Source classification unavailable"}</span></div>}
    {candles.isLoading||providers.isLoading||instruments.isLoading?<p>Loading market candles…</p>:null}
    {candles.error?<div role="alert" className="error"><p>{candles.error instanceof Error?candles.error.message:"Failed to fetch candles."}</p><button className="btn" onClick={()=>void candles.refetch()}>Retry</button></div>:null}
    {!candles.isLoading&&!candles.error&&chartData.length===0?<p>No candle history available from the selected provider/instrument/timeframe. Connect a data provider or MT5 bridge; chart will not fabricate prices.</p>:null}
    <div ref={containerRef} style={{width:"100%",minHeight:480}} aria-label="Interactive candlestick chart"/>
    <div style={{display:"flex",gap:10,alignItems:"end",flexWrap:"wrap",marginTop:12}}><label className="journal-field"><span>Stop Loss price</span><input inputMode="decimal" value={sl} onChange={e=>setSl(e.target.value)} placeholder="Draw SL line"/></label><label className="journal-field"><span>Take Profit price</span><input inputMode="decimal" value={tp} onChange={e=>setTp(e.target.value)} placeholder="Draw TP line"/></label><button className="btn" onClick={()=>{setSl("");setTp("");}}>Clear levels</button><span style={{fontSize:12,opacity:.65}}>Horizontal SL/TP lines are visual analysis only; they do not place broker orders.</span></div>
    <p style={{fontSize:11,opacity:.65,marginTop:12}}>Data source: {candles.data?.last_updated_at??"provider timestamp unavailable"} · {candles.data?.count??chartData.length} candles · Historical price data may be delayed or classified by the provider.</p>
  </section>;
}
