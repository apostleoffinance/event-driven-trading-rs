"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import {
  createChart,
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  type Time,
} from "lightweight-charts";
import { useQuery } from "@tanstack/react-query";
import { isProviderSelectable, marketDataApi, type Candle } from "@/lib/api/market-data";
import { formatPrice } from "@/lib/format";

const FRAMES = ["M1", "M5", "M15", "M30", "H1", "H4", "D1", "W1"] as const;
type Frame = (typeof FRAMES)[number];
type DrawMode = "none" | "horizontal" | "trendline";
type ChartCandle = { time: Time; open: number; high: number; low: number; close: number; volume: number | null };
type Drawing =
  | { id: number; kind: "horizontal"; price: number }
  | { id: number; kind: "trendline"; points: [{ time: Time; value: number }, { time: Time; value: number }] };

function toChartCandles(candles: Candle[]): ChartCandle[] {
  const byTime = new Map<number, ChartCandle>();
  for (const raw of candles) {
    const timestamp = Number(raw.timestamp_ms);
    const open = Number(raw.open), high = Number(raw.high), low = Number(raw.low), close = Number(raw.close);
    if (![timestamp, open, high, low, close].every(Number.isFinite) || timestamp <= 0) continue;
    if (high < Math.max(open, close, low) || low > Math.min(open, close, high)) continue;
    const seconds = Math.floor(timestamp / 1000);
    const volume = raw.volume == null ? null : Number(raw.volume);
    byTime.set(seconds, {
      time: seconds as Time, open, high, low, close,
      volume: volume !== null && Number.isFinite(volume) && volume >= 0 ? volume : null,
    });
  }
  return [...byTime.values()].sort((a, b) => Number(a.time) - Number(b.time));
}

function movingAverage(values: ChartCandle[], period: number) {
  if (values.length < period) return [];
  const result: { time: Time; value: number }[] = [];
  let sum = 0;
  for (let i = 0; i < values.length; i += 1) {
    sum += values[i].close;
    if (i >= period) sum -= values[i - period].close;
    if (i >= period - 1) result.push({ time: values[i].time, value: sum / period });
  }
  return result;
}

function precisionFor(symbol: string, values: ChartCandle[]) {
  const normalized = symbol.replace(/[^A-Z]/gi, "").toUpperCase();
  const currencies = ["EUR","GBP","AUD","NZD","USD","CAD","CHF","JPY","SEK","NOK","DKK","SGD","HKD","ZAR","MXN","PLN","TRY","CNH"];
  if (normalized.length === 6 && currencies.includes(normalized.slice(0, 3)) && currencies.includes(normalized.slice(3))) {
    return normalized.endsWith("JPY") ? 3 : 5;
  }
  const places = values.flatMap(c => [c.open,c.high,c.low,c.close]).map(v => {
    const text = v.toString().toLowerCase();
    if (text.includes("e-")) {
      const [coefficient, exponent] = text.split("e-");
      return (coefficient.split(".")[1]?.length ?? 0) + Number(exponent);
    }
    return text.split(".")[1]?.length ?? 0;
  });
  return Math.min(8, Math.max(2, ...places));
}

function cssColor(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  const value = window.getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value && !value.startsWith("var(") ? value : fallback;
}

function MarketAnalysisChartInstance({ onRetry }: { onRetry: () => void }) {
  const [symbol, setSymbol] = useState("EURUSD");
  const [frame, setFrame] = useState<Frame>("M1");
  const [providerOverride, setProviderOverride] = useState<string | null>(null);
  const [showVolume, setShowVolume] = useState(true);
  const [showSma, setShowSma] = useState(false);
  const [drawMode, setDrawMode] = useState<DrawMode>("none");
  const [drawings, setDrawings] = useState<Drawing[]>([]);
  const [pendingAnchor, setPendingAnchor] = useState<{ time: Time; value: number } | null>(null);
  const [sl, setSl] = useState("");
  const [tp, setTp] = useState("");
  const [chartReady, setChartReady] = useState(false);
  const [chartError, setChartError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candleRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const volumeRef = useRef<ISeriesApi<"Histogram"> | null>(null);
  const smaRef = useRef<ISeriesApi<"Line"> | null>(null);
  const slLineRef = useRef<IPriceLine | null>(null);
  const tpLineRef = useRef<IPriceLine | null>(null);
  const drawingSeriesRef = useRef<ISeriesApi<"Line">[]>([]);
  const drawingPriceLinesRef = useRef<IPriceLine[]>([]);
  const fittedViewKeyRef = useRef("");

  const providers = useQuery({ queryKey: ["market-data", "providers"], queryFn: () => marketDataApi.providers() });
  const instruments = useQuery({ queryKey: ["market-data", "instruments"], queryFn: () => marketDataApi.instruments() });
  const selectedSymbol = instruments.data?.some(item => item.id === symbol) ? symbol : instruments.data?.[0]?.id ?? "";
  const selectedInstrument = instruments.data?.find(item => item.id === selectedSymbol);
  const selectedProvider = providerOverride ?? String(selectedInstrument?.default_provider ?? "biquote-live");
  const candles = useQuery({
    queryKey: ["market-data", "analysis-candles", selectedSymbol, frame, selectedProvider],
    queryFn: () => marketDataApi.candles({ instrument: selectedSymbol, timeframe: frame, provider: selectedProvider, limit: 500 }),
    enabled: Boolean(selectedSymbol && selectedProvider),
    refetchInterval: 15_000,
    refetchIntervalInBackground: false,
    retry: 1,
  });

  const chartData = useMemo(() => toChartCandles(candles.data?.candles ?? []), [candles.data?.candles]);
  const latest = chartData.at(-1);
  const pricePrecision = precisionFor(selectedInstrument?.symbol ?? selectedInstrument?.name ?? selectedSymbol, chartData);
  const chartFailure = chartError;
  const loadingRegistry = providers.isLoading || instruments.isLoading;
  const feedFreshness = String(candles.data?.freshness ?? "unknown").toLowerCase();

  // Create the chart exactly once per mounted component. Data/precision changes update
  // existing series instead of destroying the chart and invalidating its references.
  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    const chartContainer: HTMLDivElement = node;
    let chart: IChartApi | null = null;
    let resizeObserver: ResizeObserver | null = null;
    let resizeHandler: (() => void) | null = null;
    try {
      const textColor = cssColor("--chart-muted", "#d0d2d6");
      const gridColor = cssColor("--chart-grid", "#2a2d34");
      const axisColor = cssColor("--chart-axis", "#3a3e48");
      chart = createChart(node, {
        width: Math.max(node.clientWidth, 320),
        height: 500,
        layout: { background: { color: cssColor("--bg-1", "#111216") }, textColor, fontFamily: '"IBM Plex Sans", -apple-system, "Segoe UI", sans-serif', fontSize: 12 },
        grid: { vertLines: { color: gridColor }, horzLines: { color: gridColor } },
        rightPriceScale: { borderColor: axisColor, scaleMargins: { top: 0.08, bottom: 0.2 } },
        timeScale: { borderColor: axisColor, timeVisible: true, secondsVisible: false, rightOffset: 5, barSpacing: 8 },
        crosshair: { mode: 1 },
        localization: { locale: "en-GB" },
      });
      chartRef.current = chart;
      candleRef.current = chart.addCandlestickSeries({
        upColor: "#26a69a", downColor: "#ef5350", borderUpColor: "#26a69a", borderDownColor: "#ef5350",
        wickUpColor: "#26a69a", wickDownColor: "#ef5350",
        priceFormat: { type: "price", precision: 5, minMove: 0.00001 },
      });
      volumeRef.current = chart.addHistogramSeries({
        priceFormat: { type: "volume" }, priceScaleId: "volume", lastValueVisible: false, priceLineVisible: false,
      });
      chart.priceScale("volume").applyOptions({ scaleMargins: { top: 0.82, bottom: 0 } });
      smaRef.current = chart.addLineSeries({
        color: "#e5ad48", lineWidth: 2, priceLineVisible: false, lastValueVisible: false, title: "SMA 20",
        priceFormat: { type: "price", precision: 5, minMove: 0.00001 },
      });
      resizeObserver = typeof ResizeObserver !== "undefined" ? new ResizeObserver(entries => {
        const width = entries[0]?.contentRect.width;
        if (width && width > 0) chart?.applyOptions({ width });
      }) : null;
      if (resizeObserver) resizeObserver.observe(chartContainer);
      else {
        resizeHandler = () => { if (chartContainer.clientWidth > 0) chart?.applyOptions({ width: chartContainer.clientWidth }); };
        window.addEventListener("resize", resizeHandler);
      }
      setChartError(null);
      setChartReady(true);
    } catch (error) {
      console.error("[MarketData] Lightweight Charts initialization failed", error);
      setChartError(error instanceof Error ? error.message : String(error));
      setChartReady(false);
    }
    return () => {
      resizeObserver?.disconnect();
      if (resizeHandler) window.removeEventListener("resize", resizeHandler);
      if (chart) {
        try { chart.remove(); } catch (error) { console.warn("[MarketData] Chart cleanup failed", error); }
      }
      if (chartRef.current === chart) chartRef.current = null;
      candleRef.current = null; volumeRef.current = null; smaRef.current = null;
      slLineRef.current = null; tpLineRef.current = null;
      drawingSeriesRef.current = []; drawingPriceLinesRef.current = [];
    };
  }, []);

  useEffect(() => {
    if (!chartReady || !candleRef.current || !volumeRef.current || !smaRef.current) return;
    try {
      const priceFormat = { type: "price" as const, precision: pricePrecision, minMove: 1 / (10 ** pricePrecision) };
      candleRef.current.applyOptions({ priceFormat });
      smaRef.current.applyOptions({ priceFormat });
      candleRef.current.setData(chartData.map(({ time, open, high, low, close }) => ({ time, open, high, low, close })));
      volumeRef.current.setData(chartData.filter(c => c.volume !== null).map(c => ({
        time: c.time, value: c.volume as number, color: c.close >= c.open ? "rgba(38,166,154,.38)" : "rgba(239,83,80,.38)",
      })));
      smaRef.current.setData(showSma ? movingAverage(chartData, 20) : []);
      volumeRef.current.applyOptions({ visible: showVolume });
      const viewKey = `${selectedSymbol}:${frame}:${selectedProvider}`;
      if (chartData.length && fittedViewKeyRef.current !== viewKey) {
        chartRef.current?.timeScale().fitContent();
        fittedViewKeyRef.current = viewKey;
      }
      if (candles.data?.last_updated_at) setLastUpdated(candles.data.last_updated_at);
      setChartError(null);
    } catch (error) {
      console.error("[MarketData] Failed to render candle series", error);
      setChartError(error instanceof Error ? error.message : String(error));
    }
  }, [chartData, chartReady, pricePrecision, showSma, showVolume, selectedSymbol, frame, selectedProvider, candles.data?.last_updated_at]);

  useEffect(() => {
    const series = candleRef.current;
    if (!series || !chartReady) return;
    try {
      if (slLineRef.current) series.removePriceLine(slLineRef.current);
      if (tpLineRef.current) series.removePriceLine(tpLineRef.current);
      slLineRef.current = null; tpLineRef.current = null;
      const stop = Number(sl), target = Number(tp);
      if (sl.trim() && Number.isFinite(stop) && stop > 0) slLineRef.current = series.createPriceLine({ price: stop, color: "#ef5350", lineWidth: 2, lineStyle: 2, axisLabelVisible: true, title: "SL" });
      if (tp.trim() && Number.isFinite(target) && target > 0) tpLineRef.current = series.createPriceLine({ price: target, color: "#26a69a", lineWidth: 2, lineStyle: 2, axisLabelVisible: true, title: "TP" });
    } catch (error) {
      console.error("[MarketData] Failed to update SL/TP overlays", error);
      setChartError(error instanceof Error ? error.message : String(error));
    }
  }, [sl, tp, chartReady]);

  useEffect(() => {
    const chart = chartRef.current, series = candleRef.current;
    if (!chart || !series || !chartReady) return;
    const onClick = (event: { point?: { x: number; y: number } }) => {
      if (drawMode === "none" || !event.point) return;
      const time = chart.timeScale().coordinateToTime(event.point.x);
      const price = series.coordinateToPrice(event.point.y);
      if (time === null || price === null || !Number.isFinite(price)) return;
      const point = { time, value: price };
      if (drawMode === "horizontal") {
        setDrawings(prev => [...prev, { id: Date.now(), kind: "horizontal", price }]);
        setDrawMode("none");
      } else if (!pendingAnchor) setPendingAnchor(point);
      else {
        setDrawings(prev => [...prev, { id: Date.now(), kind: "trendline", points: [pendingAnchor, point] }]);
        setPendingAnchor(null); setDrawMode("none");
      }
    };
    chart.subscribeClick(onClick);
    return () => chart.unsubscribeClick(onClick);
  }, [chartReady, drawMode, pendingAnchor]);

  useEffect(() => {
    const chart = chartRef.current, series = candleRef.current;
    if (!chart || !series || !chartReady) return;
    try {
      drawingPriceLinesRef.current.forEach(line => series.removePriceLine(line));
      drawingSeriesRef.current.forEach(line => chart.removeSeries(line));
      drawingPriceLinesRef.current = []; drawingSeriesRef.current = [];
      drawings.forEach(drawing => {
        if (drawing.kind === "horizontal") drawingPriceLinesRef.current.push(series.createPriceLine({
          price: drawing.price, color: "#e5ad48", lineWidth: 1, lineStyle: 2, axisLabelVisible: true, title: "Analysis",
        }));
        else {
          const line = chart.addLineSeries({ color: "#e5ad48", lineWidth: 2, priceLineVisible: false, lastValueVisible: false, title: "Trendline" });
          line.setData(drawing.points); drawingSeriesRef.current.push(line);
        }
      });
    } catch (error) {
      console.error("[MarketData] Failed to render chart drawings", error);
      setChartError(error instanceof Error ? error.message : String(error));
    }
  }, [drawings, chartReady]);



  const error = providers.error ?? instruments.error ?? candles.error;
  const fmt = (value: number | undefined) => value == null ? "—" : formatPrice(value);
  return (
    <section className="market-terminal">
      <header className="market-terminal-header">
        <div className="market-terminal-heading">
          <div><span className="market-eyebrow">MARKET DATA TERMINAL</span><h2>Market Analysis</h2></div>
          <span className={`market-feed-status ${feedFreshness === "fresh" ? "is-live" : feedFreshness === "stale" ? "is-stale" : "is-neutral"}`}><i />{feedFreshness === "fresh" ? "Fresh feed" : feedFreshness === "stale" ? "Stale feed" : feedFreshness === "unknown" ? "Feed status unknown" : feedFreshness}</span>
        </div>
        <div className="market-controls">
          <label><span>Instrument</span><select value={selectedSymbol} onChange={e => { setProviderOverride(null); setSymbol(e.target.value); }} disabled={instruments.isLoading || !instruments.data?.length}>
            {(instruments.data ?? []).map(item => <option key={item.id} value={item.id}>{item.symbol ?? item.name ?? item.id}</option>)}
          </select></label>
          <label><span>Data provider</span><select value={selectedProvider} onChange={e => setProviderOverride(e.target.value)} disabled={providers.isLoading || !providers.data?.length}>
            {(providers.data ?? []).filter(isProviderSelectable).map(item => <option key={item.id} value={item.id}>{item.name ?? item.id}</option>)}
          </select></label>
          <div className="market-price-hero">
            <span>Latest close</span><strong>{latest ? fmt(latest.close) : "—"}</strong>
            {latest ? <small className={latest.close >= latest.open ? "is-up" : "is-down"}>{latest.close >= latest.open ? "▲" : "▼"} {fmt(Math.abs(latest.close - latest.open))} candle range</small> : <small>Awaiting valid candles</small>}
          </div>
        </div>
        <div className="market-timeframes" role="group" aria-label="Chart timeframe">
          {FRAMES.map(timeframe => <button key={timeframe} type="button" className={frame === timeframe ? "active" : ""} aria-pressed={frame === timeframe} onClick={() => setFrame(timeframe)}>{timeframe}</button>)}
        </div>
      </header>

      <div className="market-chart-toolbar">
        <div className="market-tool-group">
          <button type="button" className={drawMode === "horizontal" ? "active" : ""} aria-pressed={drawMode === "horizontal"} onClick={() => { setDrawMode(drawMode === "horizontal" ? "none" : "horizontal"); setPendingAnchor(null); }}>Horizontal line</button>
          <button type="button" className={drawMode === "trendline" ? "active" : ""} aria-pressed={drawMode === "trendline"} onClick={() => { setDrawMode(drawMode === "trendline" ? "none" : "trendline"); setPendingAnchor(null); }}>Trendline</button>
          <button type="button" onClick={() => setDrawings(prev => prev.slice(0, -1))} disabled={!drawings.length}>Undo</button>
          <button type="button" onClick={() => { setDrawings([]); setPendingAnchor(null); setDrawMode("none"); }}>Clear drawings</button>
        </div>
        <div className="market-tool-group">
          <label><input type="checkbox" checked={showVolume} onChange={e => setShowVolume(e.target.checked)} /> Volume</label>
          <label><input type="checkbox" checked={showSma} onChange={e => setShowSma(e.target.checked)} /> SMA 20</label>
        </div>
      </div>

      <div className="market-ohlc-strip">
        {([["OPEN", latest?.open],["HIGH", latest?.high],["LOW", latest?.low],["CLOSE", latest?.close],["VOLUME", latest?.volume ?? undefined]] as const).map(([label, value]) =>
          <div key={label}><span>{label}</span><strong>{label === "VOLUME" && value != null ? Number(value).toLocaleString("en-US") : fmt(value)}</strong></div>
        )}
      </div>

      {loadingRegistry ? <div className="market-chart-state">Loading instrument and provider registry…</div> : null}
      {error && !candles.data ? <div className="market-chart-state is-error" role="alert"><strong>Market data request failed</strong><span>{error instanceof Error ? error.message : "Unable to load the selected data source."}</span><button type="button" onClick={() => { void providers.refetch(); void instruments.refetch(); void candles.refetch(); }}>Retry data request</button></div> : null}
      {candles.isFetching && chartData.length > 0 ? <div className="market-fetch-indicator">Updating candles…</div> : null}
      {chartFailure ? <div className="market-chart-state is-error" role="alert"><strong>Chart could not render</strong><span>{chartFailure}</span><button type="button" onClick={onRetry}>Reinitialize chart</button></div> : null}
      {!candles.isLoading && !error && chartData.length === 0 ? <div className="market-chart-state"><strong>No candle history for this selection</strong><span>Choose another provider or timeframe. The chart never invents missing market prices.</span></div> : null}
      <div ref={containerRef} className="market-chart-canvas" aria-label="Interactive TradingView Lightweight candlestick chart" />

      <div className="market-overlay-controls">
        <label><span>Stop loss price</span><input inputMode="decimal" value={sl} onChange={e => setSl(e.target.value)} placeholder="Enter SL price" /></label>
        <label><span>Take profit price</span><input inputMode="decimal" value={tp} onChange={e => setTp(e.target.value)} placeholder="Enter TP price" /></label>
        <button type="button" onClick={() => { setSl(""); setTp(""); }}>Clear levels</button>
        <span>Visual overlays only — no broker orders are placed.</span>
      </div>

      <section className="market-candle-history">
        <div className="market-subsection-heading"><div><h3>Recent candles</h3><p>Latest 30 candles · timestamps shown in UTC</p></div><span>{candles.data?.count ?? chartData.length} returned</span></div>
        {candles.isLoading ? <div className="market-chart-state">Loading candle history…</div> : chartData.length ? <div className="market-table-scroll"><table><thead><tr><th>Time (UTC)</th><th>Open</th><th>High</th><th>Low</th><th>Close</th><th>Volume</th></tr></thead><tbody>
          {[...chartData].slice(-30).reverse().map(c => <tr key={String(c.time)}><td>{new Date(Number(c.time) * 1000).toLocaleString("en-GB", { timeZone: "UTC", hour12: false })}</td><td>{fmt(c.open)}</td><td>{fmt(c.high)}</td><td>{fmt(c.low)}</td><td className={c.close >= c.open ? "is-up" : "is-down"}>{fmt(c.close)}</td><td>{c.volume == null ? "—" : c.volume.toLocaleString("en-US")}</td></tr>)}
        </tbody></table></div> : <p className="market-empty-history">No candles available for this instrument, provider and timeframe.</p>}
      </section>
      <footer className="market-chart-footer"><span>TradingView Lightweight Charts™ · Read-only market analysis</span><span>Updated: {lastUpdated ?? candles.data?.last_updated_at ?? "Not reported"} · Classification: {candles.data?.classification ?? "Unavailable"}</span><a href="https://www.tradingview.com/" target="_blank" rel="noreferrer">TradingView</a></footer>
    </section>
  );
}


export function MarketAnalysisChart() {
  const [attempt, setAttempt] = useState(0);
  return <MarketAnalysisChartInstance key={attempt} onRetry={() => setAttempt(value => value + 1)} />;
}
