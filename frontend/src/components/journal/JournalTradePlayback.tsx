"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { createChart, type IChartApi, type ISeriesApi, type Time } from "lightweight-charts";
import { marketDataApi, type Candle } from "@/lib/api/market-data";
import type { JournalTrade } from "@/lib/api/journal";
import { formatPrice } from "@/lib/format";

const FRAMES = ["M1", "M5", "M15", "M30", "H1", "H4", "D1"] as const;
type Frame = (typeof FRAMES)[number];
type ReplayCandle = { time: Time; open: number; high: number; low: number; close: number; volume?: number };

function normalizeCandles(candles: Candle[]): ReplayCandle[] {
  const byTime = new Map<number, ReplayCandle>();
  for (const candle of candles) {
    if (![candle.timestamp_ms, candle.open, candle.high, candle.low, candle.close].every(Number.isFinite)) continue;
    if (candle.timestamp_ms <= 0 || candle.high < candle.low) continue;
    if (candle.high < Math.max(candle.open, candle.close) || candle.low > Math.min(candle.open, candle.close)) continue;
    const seconds = Math.floor(candle.timestamp_ms / 1000);
    byTime.set(seconds, {
      time: seconds as Time, open: candle.open, high: candle.high, low: candle.low, close: candle.close,
      ...(typeof candle.volume === "number" && Number.isFinite(candle.volume) && candle.volume >= 0 ? { volume: candle.volume } : {}),
    });
  }
  return [...byTime.values()].sort((a, b) => Number(a.time) - Number(b.time));
}

function timeLabel(time: Time): string {
  return new Date(Number(time) * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function movementPips(symbol: string, entry: number, current: number, direction: JournalTrade["direction"]): number | null {
  const normalized = symbol.replace(/[^A-Z]/gi, "").toUpperCase();
  if (!/^[A-Z]{6}$/.test(normalized)) return null;
  const pip = normalized.endsWith("JPY") ? 0.01 : 0.0001;
  return ((current - entry) * (direction === "long" ? 1 : -1)) / pip;
}

export function JournalTradePlayback({ trade }: { trade: JournalTrade }) {
  const [expanded, setExpanded] = useState(false);
  const [frame, setFrame] = useState<Frame>(
    trade.timeframe && FRAMES.includes(trade.timeframe.toUpperCase() as Frame)
      ? trade.timeframe.toUpperCase() as Frame : "M5",
  );
  const [providerOverride, setProviderOverride] = useState("");
  const [currentIndex, setCurrentIndex] = useState(0);
  const [speed, setSpeed] = useState(1);
  const [playing, setPlaying] = useState(false);
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);

  const instrumentsQuery = useQuery({
    queryKey: ["market-data", "instruments"],
    queryFn: () => marketDataApi.instruments(),
    enabled: expanded,
  });
  const providersQuery = useQuery({
    queryKey: ["market-data", "providers"],
    queryFn: () => marketDataApi.providers(),
    enabled: expanded,
  });

  const instrument = instrumentsQuery.data?.find((item) =>
    item.id === trade.symbol || item.symbol?.toUpperCase() === trade.symbol.toUpperCase(),
  );
  const fallbackProvider = providersQuery.data?.find((item) => {\n    if (!item.status || typeof item.status !== "object") return true;\n    const state = String((item.status as { state?: unknown }).state ?? "").toLowerCase();\n    return !["disabled", "unavailable", "not_implemented"].includes(state);\n  })?.id ?? "";\n  const selectedProvider = providerOverride || String(instrument?.default_provider ?? fallbackProvider);
  const isClosedTrade = trade.status === "closed" && Boolean(trade.closed_at) &&
    new Date(trade.closed_at || "").getTime() > new Date(trade.opened_at).getTime();

  const candleQuery = useQuery({
    queryKey: ["journal", "trade-replay", trade.id, frame, selectedProvider],
    queryFn: () => marketDataApi.candles({
      instrument: instrument?.id ?? trade.symbol,
      timeframe: frame,
      provider: selectedProvider || undefined,
      from: trade.opened_at,
      to: trade.closed_at || undefined,
      limit: 5000,
    }),
    enabled: expanded && isClosedTrade && Boolean(selectedProvider),
    retry: 1,
  });

  const candles = useMemo(() => normalizeCandles(candleQuery.data?.candles ?? []), [candleQuery.data?.candles]);
  const current = candles[Math.min(currentIndex, Math.max(0, candles.length - 1))];
  const pips = current ? movementPips(trade.symbol, Number(trade.entry_price), current.close, trade.direction) : null;

  const availableProviders = providersQuery.data?.filter((item) => {
    if (!item.status || typeof item.status !== "object") return true;
    const state = String((item.status as { state?: unknown }).state ?? "").toLowerCase();
    return !["disabled", "unavailable", "not_implemented"].includes(state);
  }) ?? [];

  useEffect(() => {
    setCurrentIndex(0);
    setPlaying(false);
  }, [trade.id, frame, selectedProvider]);

  useEffect(() => {
    if (currentIndex >= candles.length) setCurrentIndex(Math.max(0, candles.length - 1));
  }, [currentIndex, candles.length]);

  useEffect(() => {
    if (!expanded || !chartContainerRef.current) return;
    const node = chartContainerRef.current;
    const chart = createChart(node, {
      width: Math.max(node.clientWidth, 320),
      height: 380,
      layout: { background: { color: "transparent" }, textColor: "var(--foreground, #d1d4dc)" },
      grid: { vertLines: { color: "rgba(130,130,130,.12)" }, horzLines: { color: "rgba(130,130,130,.12)" } },
      rightPriceScale: { borderColor: "rgba(130,130,130,.25)" },
      timeScale: { borderColor: "rgba(130,130,130,.25)", timeVisible: true, secondsVisible: false },
      crosshair: { mode: 1 },
    });
    const jpy = trade.symbol.toUpperCase().endsWith("JPY");
    const series = chart.addCandlestickSeries({
      upColor: "#26a69a", downColor: "#ef5350", borderUpColor: "#26a69a", borderDownColor: "#ef5350",
      wickUpColor: "#26a69a", wickDownColor: "#ef5350",
      priceFormat: { type: "price", precision: jpy ? 3 : 5, minMove: jpy ? 0.001 : 0.00001 },
    });
    chartRef.current = chart;
    seriesRef.current = series;
    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (width && width > 0) chart.applyOptions({ width });
    });
    observer.observe(node);
    return () => {
      observer.disconnect();
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, [expanded, trade.symbol]);

  useEffect(() => {
    const chart = chartRef.current;
    const series = seriesRef.current;
    if (!chart || !series || candles.length === 0) return;
    const visible = candles.slice(0, Math.max(1, currentIndex + 1));
    series.setData(visible);
    const markers = [
      {
        time: candles[0].time,
        position: trade.direction === "long" ? "belowBar" as const : "aboveBar" as const,
        color: "#3b82f6",
        shape: trade.direction === "long" ? "arrowUp" as const : "arrowDown" as const,
        text: "Entry",
      },
    ];
    if (currentIndex >= candles.length - 1) {
      markers.push({
        time: candles[candles.length - 1].time,
        position: trade.direction === "long" ? "aboveBar" as const : "belowBar" as const,
        color: "#f59e0b",
        shape: "circle" as const,
        text: "Exit",
      });
    }
    series.setMarkers(markers);
    if (currentIndex === 0) chart.timeScale().fitContent();
  }, [candles, currentIndex, trade.direction]);

  useEffect(() => {
    if (!playing) return;
    if (candles.length < 2 || currentIndex >= candles.length - 1) {
      setPlaying(false);
      return;
    }
    const timer = window.setInterval(() => {
      setCurrentIndex((index) => {
        if (index >= candles.length - 1) {
          setPlaying(false);
          return index;
        }
        return index + 1;
      });
    }, Math.max(80, 700 / speed));
    return () => window.clearInterval(timer);
  }, [playing, candles.length, currentIndex, speed]);

  return (
    <section className="journal-playback" aria-label="Historical trade playback">
      <div className="journal-section-heading">
        <div>
          <h3 className="panel-title">TRADE PLAYBACK</h3>
          <p className="muted">Replay historical provider candles from entry toward exit. This is analysis only—no orders are placed.</p>
        </div>
        <button className="btn primary" type="button" onClick={() => { setExpanded((value) => !value); setPlaying(false); }} aria-expanded={expanded}>
          {expanded ? "Close replay" : "▶ Replay trade"}
        </button>
      </div>
      {!isClosedTrade ? <p className="muted">Replay is available for closed trades with valid entry and exit timestamps.</p> : null}
      {expanded && isClosedTrade ? (
        <div className="journal-playback-body">
          <div className="journal-playback-toolbar">
            <label>Timeframe <select value={frame} onChange={(event) => setFrame(event.target.value as Frame)}>{FRAMES.map((value) => <option key={value}>{value}</option>)}</select></label>
            <label>Provider <select value={selectedProvider} onChange={(event) => setProviderOverride(event.target.value)} disabled={providersQuery.isLoading}>
              {availableProviders.map((item) => <option key={item.id} value={item.id}>{item.name || item.id}</option>)}
            </select></label>
            <label>Speed <select value={speed} onChange={(event) => setSpeed(Number(event.target.value))}>
              <option value={0.5}>0.5×</option><option value={1}>1×</option><option value={2}>2×</option><option value={4}>4×</option><option value={8}>8×</option>
            </select></label>
          </div>
          {instrumentsQuery.isLoading || providersQuery.isLoading || candleQuery.isLoading ? <p className="muted">Loading historical candles…</p> : null}
          {candleQuery.isError ? <div className="panel error" role="alert">
            <strong>Historical candles unavailable</strong>
            <p>{candleQuery.error instanceof Error ? candleQuery.error.message : "The configured provider could not return candles for this trade period."}</p>
            <button className="btn" type="button" onClick={() => candleQuery.refetch()}>Retry</button>
          </div> : null}
          {!candleQuery.isLoading && !candleQuery.isError && candles.length === 0 ? <div className="panel">
            <strong>No historical candles found for this trade.</strong>
            <p className="muted">Try a different provider or timeframe. EdgeNest does not generate placeholder market data.</p>
          </div> : null}
          {candles.length > 0 ? <>
            <div ref={chartContainerRef} className="journal-playback-chart" aria-label="Historical candlestick playback chart" />
            <div className="journal-playback-current">
              <span><small>Replay time</small><strong>{current ? timeLabel(current.time) : "—"}</strong></span>
              <span><small>Current candle close</small><strong>{current ? formatPrice(current.close) : "—"}</strong></span>
              <span><small>Trade entry</small><strong>{formatPrice(trade.entry_price)}</strong></span>
              <span><small>Directional movement</small><strong className={pips !== null && pips > 0 ? "pos" : pips !== null && pips < 0 ? "neg" : ""}>{pips === null ? "—" : pips.toFixed(1) + " pips"}</strong></span>
            </div>
            <div className="journal-playback-controls">
              <button className="btn" type="button" onClick={() => { setPlaying(false); setCurrentIndex(0); }}>↺ Restart</button>
              <button className="btn primary" type="button" disabled={candles.length < 2} onClick={() => { if (currentIndex >= candles.length - 1) setCurrentIndex(0); setPlaying((value) => !value); }}>{playing ? "Ⅱ Pause" : "▶ Play"}</button>
              <button className="btn" type="button" disabled={currentIndex >= candles.length - 1} onClick={() => { setPlaying(false); setCurrentIndex((value) => Math.min(candles.length - 1, value + 1)); }}>Next candle →</button>
              <span className="muted">{Math.min(currentIndex + 1, candles.length)} / {candles.length} candles</span>
            </div>
            <label className="journal-playback-timeline"><span>Timeline</span><input type="range" min={0} max={Math.max(0, candles.length - 1)} value={Math.min(currentIndex, Math.max(0, candles.length - 1))} onChange={(event) => { setPlaying(false); setCurrentIndex(Number(event.target.value)); }} aria-label="Trade playback timeline" /></label>
            <p className="muted">Coverage: {timeLabel(candles[0].time)} – {timeLabel(candles[candles.length - 1].time)}. Entry/exit markers align to the first/last returned candles; candle provider timestamps may not match exact fills.</p>
          </> : null}
        </div>
      ) : null}
    </section>
  );
}
