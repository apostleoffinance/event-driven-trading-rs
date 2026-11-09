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
import { marketDataApi, type Candle } from "@/lib/api/market-data";
import { formatPrice } from "@/lib/format";

const FRAMES = ["M1", "M5", "M15", "M30", "H1", "H4", "D1", "W1"] as const;
type Frame = (typeof FRAMES)[number];

type ChartCandle = {
  time: Time;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number | null;
};

function toChartCandles(candles: Candle[]): ChartCandle[] {
  const byTime = new Map<number, ChartCandle>();
  for (const candle of candles) {
    const values = [candle.timestamp_ms, candle.open, candle.high, candle.low, candle.close];
    if (!values.every(Number.isFinite) || candle.timestamp_ms <= 0) continue;
    if (candle.high < Math.max(candle.open, candle.close, candle.low)) continue;
    if (candle.low > Math.min(candle.open, candle.close, candle.high)) continue;

    const seconds = Math.floor(candle.timestamp_ms / 1000);
    const volume = candle.volume;
    byTime.set(seconds, {
      time: seconds as Time,
      open: candle.open,
      high: candle.high,
      low: candle.low,
      close: candle.close,
      volume: typeof volume === "number" && Number.isFinite(volume) && volume >= 0 ? volume : null,
    });
  }
  return [...byTime.values()].sort((a, b) => Number(a.time) - Number(b.time));
}

function movingAverage(values: ChartCandle[], period: number) {
  if (values.length < period) return [];
  const result: { time: Time; value: number }[] = [];
  let sum = 0;
  for (let index = 0; index < values.length; index += 1) {
    sum += values[index].close;
    if (index >= period) sum -= values[index - period].close;
    if (index >= period - 1) result.push({ time: values[index].time, value: sum / period });
  }
  return result;
}

export function MarketAnalysisChart() {
  const [symbol, setSymbol] = useState("EURUSD");
  const [frame, setFrame] = useState<Frame>("H1");
  const [provider, setProvider] = useState("mt5-bridge");
  const [showVolume, setShowVolume] = useState(true);
  const [showSma, setShowSma] = useState(false);
  const [sl, setSl] = useState("");
  const [tp, setTp] = useState("");
  const [chartReady, setChartReady] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candleRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const volumeRef = useRef<ISeriesApi<"Histogram"> | null>(null);
  const smaRef = useRef<ISeriesApi<"Line"> | null>(null);
  const slLineRef = useRef<IPriceLine | null>(null);
  const tpLineRef = useRef<IPriceLine | null>(null);

  const providers = useQuery({
    queryKey: ["market-data", "providers"],
    queryFn: () => marketDataApi.providers(),
    enabled: true,
  });
  const instruments = useQuery({
    queryKey: ["market-data", "instruments"],
    queryFn: () => marketDataApi.instruments(),
    enabled: frontendConfig.dataMode === "api",
  });
  const selectedProvider = providers.data?.some((item) => item.id === provider)
    ? provider
    : providers.data?.[0]?.id ?? "";
  const selectedSymbol = instruments.data?.some((item) => item.id === symbol)
    ? symbol
    : instruments.data?.[0]?.id ?? "";

  const candles = useQuery({
    queryKey: ["market-data", "analysis-candles", selectedSymbol, frame, selectedProvider],
    queryFn: () => marketDataApi.candles({
      instrument: selectedSymbol,
      timeframe: frame,
      provider: selectedProvider,
      limit: 500,
    }),
    enabled: Boolean(selectedSymbol && selectedProvider),
  });

  const chartData = useMemo(
    () => toChartCandles(candles.data?.candles ?? []),
    [candles.data?.candles],
  );
  const latest = chartData.at(-1);
  const selectedInstrument = instruments.data?.find((item) => item.id === selectedSymbol);
  const normalizedSymbol = (selectedInstrument?.symbol ?? selectedInstrument?.name ?? selectedSymbol).replace(/[^A-Z]/gi, "").toUpperCase();
  const fxCurrencies = ["EUR", "GBP", "AUD", "NZD", "USD", "CAD", "CHF", "JPY", "SEK", "NOK", "DKK", "SGD", "HKD", "ZAR", "MXN", "PLN", "TRY", "CNH"];
  const isFxPair = normalizedSymbol.length === 6 && fxCurrencies.includes(normalizedSymbol.slice(0, 3)) && fxCurrencies.includes(normalizedSymbol.slice(3));
  const decimalPlaces = (value: number) => {
    const text = value.toString().toLowerCase();
    if (text.includes("e-")) {
      const [coefficient, exponent] = text.split("e-");
      return (coefficient.split(".")[1]?.length ?? 0) + Number(exponent);
    }
    return text.split(".")[1]?.length ?? 0;
  };
  const inferredPrecision = Math.max(2, ...chartData.flatMap((candle) => [candle.open, candle.high, candle.low, candle.close].map(decimalPlaces)));
  const pricePrecision = isFxPair && normalizedSymbol.endsWith("JPY")
    ? 3
    : isFxPair
      ? 5
      : Math.min(8, inferredPrecision);

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;

    const chart = createChart(node, {
      width: Math.max(node.clientWidth, 320),
      height: 480,
      layout: {
        background: { color: "transparent" },
        textColor: "var(--foreground, #d1d4dc)",
      },
      grid: {
        vertLines: { color: "rgba(130,130,130,.12)" },
        horzLines: { color: "rgba(130,130,130,.12)" },
      },
      rightPriceScale: { borderColor: "rgba(130,130,130,.25)" },
      timeScale: {
        borderColor: "rgba(130,130,130,.25)",
        timeVisible: true,
        secondsVisible: false,
      },
      crosshair: { mode: 1 },
    });

    candleRef.current = chart.addCandlestickSeries({
      upColor: "#26a69a",
      downColor: "#ef5350",
      borderUpColor: "#26a69a",
      borderDownColor: "#ef5350",
      wickUpColor: "#26a69a",
      wickDownColor: "#ef5350",
      priceFormat: { type: "price", precision: pricePrecision, minMove: 1 / (10 ** pricePrecision) },
    });
    volumeRef.current = chart.addHistogramSeries({
      priceFormat: { type: "volume" },
      priceScaleId: "volume",
      lastValueVisible: false,
      priceLineVisible: false,
    });
    chart.priceScale("volume").applyOptions({ scaleMargins: { top: 0.82, bottom: 0 } });
    smaRef.current = chart.addLineSeries({
      color: "#f59e0b",
      lineWidth: 2,
      priceLineVisible: false,
      lastValueVisible: false,
      title: "SMA 20",
      priceFormat: { type: "price", precision: pricePrecision, minMove: 1 / (10 ** pricePrecision) },
    });
    chartRef.current = chart;
    setChartReady(true);

    const resize = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (width && width > 0) chart.applyOptions({ width });
    });
    resize.observe(node);

    return () => {
      resize.disconnect();
      chart.remove();
      chartRef.current = null;
      candleRef.current = null;
      volumeRef.current = null;
      smaRef.current = null;
      slLineRef.current = null;
      tpLineRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (!chartReady || !candleRef.current || !volumeRef.current || !smaRef.current) return;
    candleRef.current.setData(
      chartData.map(({ time, open, high, low, close }) => ({ time, open, high, low, close })),
    );
    volumeRef.current.setData(
      chartData
        .filter((candle) => candle.volume !== null)
        .map((candle) => ({
          time: candle.time,
          value: candle.volume as number,
          color: candle.close >= candle.open
            ? "rgba(38,166,154,.35)"
            : "rgba(239,83,80,.35)",
        })),
    );
    smaRef.current.setData(showSma ? movingAverage(chartData, 20) : []);
    chartRef.current?.timeScale().fitContent();
  }, [chartData, showSma, chartReady]);

  useEffect(() => {
    volumeRef.current?.applyOptions({ visible: showVolume });
  }, [showVolume, chartReady]);

  useEffect(() => {
    const series = candleRef.current;
    if (!series) return;
    if (slLineRef.current) series.removePriceLine(slLineRef.current);
    if (tpLineRef.current) series.removePriceLine(tpLineRef.current);
    slLineRef.current = null;
    tpLineRef.current = null;

    const stop = Number(sl);
    const target = Number(tp);
    if (sl.trim() && Number.isFinite(stop) && stop > 0) {
      slLineRef.current = series.createPriceLine({
        price: stop, color: "#ef5350", lineWidth: 2, lineStyle: 2,
        axisLabelVisible: true, title: "SL",
      });
    }
    if (tp.trim() && Number.isFinite(target) && target > 0) {
      tpLineRef.current = series.createPriceLine({
        price: target, color: "#26a69a", lineWidth: 2, lineStyle: 2,
        axisLabelVisible: true, title: "TP",
      });
    }
  }, [sl, tp, chartData, chartReady]);

  const error = providers.error ?? instruments.error ?? candles.error;
  return (
    <section className="panel market-analysis">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <div>
          <h2 className="panel-title" style={{ marginBottom: 4 }}>MARKET ANALYSIS</h2>
          <p style={{ margin: 0, fontSize: 12, opacity: 0.7 }}>TradingView Lightweight Charts · Read-only analysis</p>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <label className="journal-field">
            <span>Instrument</span>
            <select value={selectedSymbol} onChange={(event) => setSymbol(event.target.value)}>
              {(instruments.data ?? []).map((item) => (
                <option key={item.id} value={item.id}>{item.symbol ?? item.id}</option>
              ))}
            </select>
          </label>
          <label className="journal-field">
            <span>Provider</span>
            <select value={selectedProvider} onChange={(event) => setProvider(event.target.value)}>
              {(providers.data ?? []).map((item) => (
                <option key={item.id} value={item.id}>{item.name ?? item.id}</option>
              ))}
            </select>
          </label>
        </div>
      </div>

      <div style={{ display: "flex", gap: 5, flexWrap: "wrap", margin: "14px 0" }}>
        {FRAMES.map((timeframe) => (
          <button
            key={timeframe}
            type="button"
            className={frame === timeframe ? "btn primary" : "btn"}
            aria-pressed={frame === timeframe}
            onClick={() => setFrame(timeframe)}
          >
            {timeframe}
          </button>
        ))}
        <span style={{ flex: 1 }} />
        <label style={{ display: "flex", alignItems: "center", gap: 5, fontSize: 12 }}>
          <input type="checkbox" checked={showVolume} onChange={(event) => setShowVolume(event.target.checked)} />
          Volume
        </label>
        <label style={{ display: "flex", alignItems: "center", gap: 5, fontSize: 12 }}>
          <input type="checkbox" checked={showSma} onChange={(event) => setShowSma(event.target.checked)} />
          SMA 20
        </label>
      </div>

      {latest ? (
        <div style={{ display: "flex", gap: 14, flexWrap: "wrap", fontSize: 12, marginBottom: 8 }}>
          <strong>{selectedSymbol}</strong>
          <span>O {formatPrice(latest.open)}</span>
          <span>H {formatPrice(latest.high)}</span>
          <span>L {formatPrice(latest.low)}</span>
          <span style={{ color: latest.close >= latest.open ? "#26a69a" : "#ef5350" }}>C {formatPrice(latest.close)}</span>
          <span>{candles.data?.classification ?? "Source classification unavailable"}</span>
        </div>
      ) : null}

      {providers.isLoading || instruments.isLoading || candles.isLoading ? <p>Loading market candles…</p> : null}
      {error ? (
        <div role="alert" className="error">
          <p>{error instanceof Error ? error.message : "Failed to load market data."}</p>
          <button className="btn" type="button" onClick={() => {
            void providers.refetch();
            void instruments.refetch();
            void candles.refetch();
          }}>Retry</button>
        </div>
      ) : null}
      {!providers.isLoading && !instruments.isLoading && !candles.isLoading && !error && chartData.length === 0 ? (
        <p>No candle history is available for this provider, instrument and timeframe. Connect a supported data provider or MT5 bridge; the chart will not fabricate prices.</p>
      ) : null}

      <div ref={containerRef} style={{ width: "100%", minHeight: 480 }} aria-label="Interactive candlestick chart" />

      <div style={{ display: "flex", gap: 10, alignItems: "end", flexWrap: "wrap", marginTop: 12 }}>
        <label className="journal-field">
          <span>Stop Loss price</span>
          <input inputMode="decimal" value={sl} onChange={(event) => setSl(event.target.value)} placeholder="Draw SL line" />
        </label>
        <label className="journal-field">
          <span>Take Profit price</span>
          <input inputMode="decimal" value={tp} onChange={(event) => setTp(event.target.value)} placeholder="Draw TP line" />
        </label>
        <button className="btn" type="button" onClick={() => { setSl(""); setTp(""); }}>Clear levels</button>
        <span style={{ fontSize: 12, opacity: 0.65 }}>SL/TP lines are visual analysis only; they do not place broker orders.</span>
      </div>

      <p style={{ fontSize: 11, opacity: 0.65, marginTop: 12 }}>
        TradingView Lightweight Charts™ · Copyright © 2024 TradingView, Inc.{" "}
        <a href="https://www.tradingview.com/" target="_blank" rel="noreferrer">TradingView</a>
      </p>
      <p style={{ fontSize: 11, opacity: 0.65, marginTop: 4 }}>
        Data source: {candles.data?.last_updated_at ?? "provider timestamp unavailable"} ·{" "}
        {candles.data?.count ?? chartData.length} candles · Provider classification:{" "}
        {candles.data?.classification ?? "unavailable"}
      </p>
    </section>
  );
}
