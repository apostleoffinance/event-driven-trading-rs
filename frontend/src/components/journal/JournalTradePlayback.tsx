"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { createChart, type IChartApi, type ISeriesApi, type Time } from "lightweight-charts";
import { marketDataApi, type Candle } from "@/lib/api/market-data";
import type { JournalTrade } from "@/lib/api/journal";
import { formatPrice } from "@/lib/format";
import { journalApi } from "@/lib/api/journal";

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
  const currencies = new Set(["USD", "EUR", "GBP", "JPY", "CHF", "CAD", "AUD", "NZD", "SEK", "NOK", "DKK", "PLN", "CZK", "HUF", "TRY", "SGD", "HKD", "MXN", "ZAR", "CNH", "CNY", "INR", "BRL", "RUB", "ILS", "THB", "KRW", "PHP", "IDR", "MYR", "AED", "SAR"]);
  const normalized = symbol.replace(/[^A-Z]/gi, "").toUpperCase().slice(0, 6);
  if (!/^[A-Z]{6}$/.test(normalized) || !currencies.has(normalized.slice(0, 3)) || !currencies.has(normalized.slice(3, 6))) return null;
  const pip = normalized.endsWith("JPY") ? 0.01 : 0.0001;
  return ((current - entry) * (direction === "long" ? 1 : -1)) / pip;
}

export function JournalTradePlayback({ trade, initiallyExpanded = false, alwaysExpanded = false }: { trade: JournalTrade; initiallyExpanded?: boolean; alwaysExpanded?: boolean }) {
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = useState(initiallyExpanded);
  const [frame, setFrame] = useState<Frame>("H1");
  const isExpanded = alwaysExpanded || expanded;
  const [providerOverride, setProviderOverride] = useState("");
  const [currentIndex, setCurrentIndex] = useState(0);
  const [speed, setSpeed] = useState(1);
  const [playing, setPlaying] = useState(false);
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const persistedExcursionRef = useRef<string>("");
  const openedTimestampMs = new Date(trade.opened_at).getTime();
  // A valid close timestamp is the authoritative replay prerequisite. The Analytics
  // investigation already selects closed-trade facts; imported APIs may serialize
  // status labels differently, so do not let a status-label mismatch hide replay.
  const isClosedTrade = Boolean(trade.closed_at) && Number.isFinite(openedTimestampMs) && new Date(trade.closed_at || "").getTime() > openedTimestampMs;
  const closeTimestampMs = new Date(trade.closed_at || "").getTime();
  const closeIsInFuture = isClosedTrade && Number.isFinite(closeTimestampMs) && closeTimestampMs > Date.now() + 60_000;

  const instrumentsQuery = useQuery({
    queryKey: ["market-data", "instruments"],
    queryFn: () => marketDataApi.instruments(),
    enabled: isClosedTrade,
  });
  const providersQuery = useQuery({
    queryKey: ["market-data", "providers"],
    queryFn: () => marketDataApi.providers(),
    enabled: isClosedTrade,
  });

  const instrument = instrumentsQuery.data?.find((item) =>
    item.id === trade.symbol || item.symbol?.toUpperCase() === trade.symbol.toUpperCase(),
  );
  const fallbackProvider = providersQuery.data?.find((item) => {
    if (item.id === "demo") return false;
    if (!item.status || typeof item.status !== "object") return true;
    const state = String((item.status as { state?: unknown }).state ?? "").toLowerCase();
    return !["disabled", "unavailable", "not_implemented"].includes(state);
  })?.id ?? "";
  const configuredProvider = String(instrument?.default_provider ?? "");
  const selectedProvider = providerOverride || (configuredProvider && configuredProvider !== "demo" ? configuredProvider : fallbackProvider);
  const candleQuery = useQuery({
    queryKey: ["journal", "trade-replay", trade.id, trade.symbol, trade.opened_at, trade.closed_at, frame, selectedProvider],
    queryFn: async () => {
      const durationMs: Record<Frame, number> = { M1: 60_000, M5: 300_000, M15: 900_000, M30: 1_800_000, H1: 3_600_000, H4: 14_400_000, D1: 86_400_000 };
      const start = new Date(trade.opened_at).getTime();
      const end = new Date(trade.closed_at || "").getTime();
      if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) throw new Error("Invalid trade coverage interval.");
      const all: Candle[] = [];
      let cursor = start;
      for (let pageIndex = 0; cursor < end && pageIndex < 64; pageIndex += 1) {
        const pageEnd = Math.min(end, cursor + durationMs[frame] * 900);
        const page = await marketDataApi.candles({
          instrument: instrument?.id ?? trade.symbol,
          timeframe: frame,
          provider: selectedProvider || undefined,
          from: new Date(cursor).toISOString(),
          to: new Date(pageEnd).toISOString(),
          limit: 1000,
        });
        all.push(...page.candles);
        cursor = pageEnd;
      }
      return { candles: all };
    },
    enabled: isClosedTrade && !closeIsInFuture && Boolean(selectedProvider),
    retry: 1,
  });

  const candles = useMemo(() => normalizeCandles(candleQuery.data?.candles ?? []), [candleQuery.data?.candles]);
  const current = candles[Math.min(currentIndex, Math.max(0, candles.length - 1))];
  const pips = current ? movementPips(trade.symbol, Number(trade.entry_price), current.close, trade.direction) : null;
  const replayPath = candles.slice(0, Math.max(1, currentIndex + 1));
  const entryPrice = Number(trade.entry_price);
  const favorablePrice = trade.direction === "long"
    ? replayPath.reduce((value, candle) => Math.max(value, candle.high), entryPrice)
    : replayPath.reduce((value, candle) => Math.min(value, candle.low), entryPrice);
  const adversePrice = trade.direction === "long"
    ? replayPath.reduce((value, candle) => Math.min(value, candle.low), entryPrice)
    : replayPath.reduce((value, candle) => Math.max(value, candle.high), entryPrice);
  const mfePips = movementPips(trade.symbol, entryPrice, favorablePrice, trade.direction);
  const maePips = movementPips(trade.symbol, entryPrice, adversePrice, trade.direction);

  useEffect(() => {
    if (!isClosedTrade || closeIsInFuture || candles.length === 0 || !Number.isFinite(entryPrice)) return;
    const first = Number(candles[0].time) * 1000;
    const last = Number(candles[candles.length - 1].time) * 1000;
    const opened = new Date(trade.opened_at).getTime();
    const closed = new Date(trade.closed_at || "").getTime();
    const tolerance = ({ M1: 60_000, M5: 300_000, M15: 900_000, M30: 1_800_000, H1: 3_600_000, H4: 14_400_000, D1: 86_400_000 } as Record<Frame, number>)[frame] * 2;
    if (first > opened + tolerance || last < closed - tolerance) return;
    const fullFavorable = trade.direction === "long"
      ? candles.reduce((value, candle) => Math.max(value, candle.high), entryPrice)
      : candles.reduce((value, candle) => Math.min(value, candle.low), entryPrice);
    const fullAdverse = trade.direction === "long"
      ? candles.reduce((value, candle) => Math.min(value, candle.low), entryPrice)
      : candles.reduce((value, candle) => Math.max(value, candle.high), entryPrice);
    const directionSign = trade.direction === "long" ? 1 : -1;
    const riskDistance = trade.stop_loss ? Math.abs(entryPrice - Number(trade.stop_loss)) : 0;
    const mfeR = riskDistance > 0 ? Math.max(0, ((fullFavorable - entryPrice) * directionSign) / riskDistance) : null;
    const maeR = riskDistance > 0 ? Math.min(0, ((fullAdverse - entryPrice) * directionSign) / riskDistance) : null;
    const key = [trade.id, trade.status, frame, selectedProvider, first, last, entryPrice, trade.stop_loss, trade.direction].join(":");
    if (persistedExcursionRef.current === key) return;
    persistedExcursionRef.current = key;
    void journalApi.saveExcursions(trade.id, {
      mfe_price: fullFavorable,
      mae_price: fullAdverse,
      mfe_r: mfeR,
      mae_r: maeR,
      timeframe: frame,
      provider: selectedProvider,
      candle_count: candles.length,
      coverage_start: new Date(first).toISOString(),
      coverage_end: new Date(last).toISOString(),
    }).then(async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["journal", "trades"] }),
        queryClient.invalidateQueries({ queryKey: ["journal", "analytics"] }),
        queryClient.invalidateQueries({ queryKey: ["journal", "recent-closed-trades"] }),
      ]);
    }).catch(() => {
      if (persistedExcursionRef.current === key) persistedExcursionRef.current = "";
    });
  }, [isClosedTrade, closeIsInFuture, candles, entryPrice, trade.id, trade.opened_at, trade.closed_at, trade.direction, trade.stop_loss, frame, selectedProvider, queryClient]);

  const coverageToleranceMs = ({ M1: 60_000, M5: 300_000, M15: 900_000, M30: 1_800_000, H1: 3_600_000, H4: 14_400_000, D1: 86_400_000 } as Record<Frame, number>)[frame] * 2;
  const hasFullCandleCoverage = candles.length > 0
    && Number(candles[0].time) * 1000 <= new Date(trade.opened_at).getTime() + coverageToleranceMs
    && Number(candles[candles.length - 1].time) * 1000 >= new Date(trade.closed_at || "").getTime() - coverageToleranceMs;

  const availableProviders = providersQuery.data?.filter((item) => {
    if (item.id === "demo") return false;
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
    if (!isExpanded || !chartContainerRef.current) return;
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
    const entry = Number(trade.entry_price);
    if (Number.isFinite(entry)) series.createPriceLine({ price: entry, color: "#3b82f6", lineWidth: 1, lineStyle: 2, axisLabelVisible: true, title: "Entry" });
    const stop = Number(trade.stop_loss);
    if (trade.stop_loss && Number.isFinite(stop)) series.createPriceLine({ price: stop, color: "#ef5350", lineWidth: 1, lineStyle: 2, axisLabelVisible: true, title: "SL" });
    const targetPrice = Number(trade.take_profit);
    if (trade.take_profit && Number.isFinite(targetPrice)) series.createPriceLine({ price: targetPrice, color: "#16a34a", lineWidth: 1, lineStyle: 2, axisLabelVisible: true, title: "TP" });
    const exitPrice = Number(trade.exit_price);
    if (trade.exit_price && Number.isFinite(exitPrice)) series.createPriceLine({ price: exitPrice, color: "#f59e0b", lineWidth: 1, lineStyle: 1, axisLabelVisible: true, title: "Exit" });
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
  }, [isExpanded, candles.length, trade.symbol, trade.entry_price, trade.stop_loss, trade.take_profit, trade.exit_price]);

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
        shape: trade.direction === "long" ? "arrowDown" as const : "arrowUp" as const,
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
        {!alwaysExpanded ? <button className="btn primary" type="button" onClick={() => { setExpanded((value) => !value); setPlaying(false); }} aria-expanded={isExpanded}>
          {isExpanded ? "Close replay" : "▶ Replay trade"}
        </button> : <span className="badge">Replay ready</span>}
      </div>
      {!isClosedTrade ? <p className="muted">Replay is available for closed trades with valid entry and exit timestamps.</p> : null}
      {closeIsInFuture ? <div className="panel error" role="status">
        <strong>Recorded close time is in the future.</strong>
        <p>The trade is marked closed at {new Date(closeTimestampMs).toLocaleString()}. Historical MFE and MAE cannot be calculated for a period that has not happened yet. Correct the close date/time in Journal, or wait until the recorded close time has passed.</p>
      </div> : null}
      {isExpanded && isClosedTrade ? (
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
          {!providersQuery.isLoading && providersQuery.isError ? <p className="muted" role="status">Market-data providers could not be loaded. Retry after checking the market-data service.</p> : null}
          {!providersQuery.isLoading && providersQuery.isSuccess && availableProviders.length === 0 ? <p className="muted" role="status">No historical market-data provider is currently available. MFE and MAE cannot be calculated until a provider is configured.</p> : null}
          {!providersQuery.isLoading && providersQuery.isSuccess && availableProviders.length > 0 && !selectedProvider ? <p className="muted" role="status">No provider is selected for this instrument. Choose an available historical provider to load candles.</p> : null}
          {candleQuery.isError ? <div className="panel error" role="alert">
            <strong>Historical candles unavailable</strong>
            <p>{candleQuery.error instanceof Error ? candleQuery.error.message : "The configured provider could not return candles for this trade period."}</p>
            <button className="btn" type="button" onClick={() => candleQuery.refetch()}>Retry</button>
          </div> : null}
          {!candleQuery.isLoading && !candleQuery.isError && candles.length > 0 && !hasFullCandleCoverage ? <div className="panel" role="status">
            <strong>Historical candle coverage is incomplete.</strong>
            <p className="muted">The returned candles do not cover the complete entry-to-exit interval. Replay is available, but MFE/MAE will not be saved from partial data. Try another provider or a coarser timeframe.</p>
          </div> : null}
          {providersQuery.isSuccess && Boolean(selectedProvider) && !closeIsInFuture && !candleQuery.isLoading && !candleQuery.isError && candles.length === 0 ? <div className="panel">
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
              <span><small>Replay MFE</small><strong className="pos">{mfePips === null ? "—" : mfePips.toFixed(1) + " pips"}</strong></span>
              <span><small>Replay MAE</small><strong className="neg">{maePips === null ? "—" : Math.abs(maePips).toFixed(1) + " pips"}</strong></span>
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
