"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { createChart, type IChartApi, type IPriceLine, type ISeriesApi } from "lightweight-charts";
import { marketDataApi, type Candle } from "@/lib/api/market-data";

type PreviewTab = "setup" | "chart" | "analysis";
type TradeSetupPreviewProps = {
  symbol: string;
  direction: "long" | "short";
  currency: string;
  entry: number;
  stop: number;
  target: number;
  quantity: number;
  risk: number | null;
  reward: number | null;
  riskPct: number | null;
  stopPips: number | null;
  targetPips: number | null;
  rr: number | null;
  balance: number | null;
  valid: boolean;
  validationMessage?: string;
  journalHref: string;
};

const money = (value: number | null, currency: string) =>
  value === null || !Number.isFinite(value)
    ? "—"
    : value.toLocaleString("en-US", { style: "currency", currency, maximumFractionDigits: 2 });

const price = (value: number, symbol = "") => {
  if (!Number.isFinite(value)) return "—";
  const fxCurrencies = ["EUR", "GBP", "AUD", "NZD", "USD", "CAD", "CHF", "JPY"];
  const isFx = symbol.length === 6 && fxCurrencies.includes(symbol.slice(0, 3)) && fxCurrencies.includes(symbol.slice(3));
  const decimals = ["XAUUSD", "XAGUSD"].includes(symbol) ? 2 : isFx && symbol.endsWith("JPY") ? 3 : isFx ? 5 : Math.min(8, Math.max(2, value.toString().split(".")[1]?.length ?? 2));
  return value.toLocaleString("en-US", { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
};

function PreviewCandlestickChart({ symbol, entry, stop, target }: { symbol: string; entry: number; stop: number; target: number }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const linesRef = useRef<IPriceLine[]>([]);
  const fittedKeyRef = useRef("");
  const instruments = useQuery({ queryKey: ["market-data", "instruments"], queryFn: () => marketDataApi.instruments() });
  const instrument = instruments.data?.find((item) => item.id === symbol || item.symbol === symbol);
  const instrumentId = instrument?.id ?? symbol;
  const provider = String(instrument?.default_provider ?? "biquote-live");
  const candles = useQuery({
    queryKey: ["market-data", "trade-preview-candles", instrumentId, provider],
    queryFn: () => marketDataApi.candles({ instrument: instrumentId, timeframe: "M15", provider, limit: 80 }),
    enabled: Boolean(instrumentId),
    refetchInterval: 15_000,
    refetchIntervalInBackground: false,
    retry: 1,
  });
  const validCandles = useMemo(() => {
    const byTime = new Map<number, Candle>();
    for (const candle of candles.data?.candles ?? []) {
      if (![candle.timestamp_ms, candle.open, candle.high, candle.low, candle.close].every(Number.isFinite)) continue;
      if (candle.timestamp_ms <= 0 || candle.high < Math.max(candle.open, candle.close, candle.low) || candle.low > Math.min(candle.open, candle.close, candle.high)) continue;
      byTime.set(Math.floor(candle.timestamp_ms / 1000), candle);
    }
    return [...byTime.entries()].sort((a, b) => a[0] - b[0]);
  }, [candles.data?.candles]);

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    const chart = createChart(node, {
      width: Math.max(node.clientWidth, 260),
      height: 230,
      layout: { background: { color: "transparent" }, textColor: "var(--text-muted, #94a3b8)", fontSize: 10 },
      grid: { vertLines: { color: "rgba(130,130,130,.10)" }, horzLines: { color: "rgba(130,130,130,.10)" } },
      rightPriceScale: { borderColor: "rgba(130,130,130,.20)", scaleMargins: { top: .12, bottom: .12 } },
      timeScale: { borderColor: "rgba(130,130,130,.20)", timeVisible: true, secondsVisible: false },
      crosshair: { mode: 1 },
      handleScroll: { mouseWheel: false, pressedMouseMove: true },
      handleScale: { mouseWheel: false, pinch: true },
    });
    const series = chart.addCandlestickSeries({
      upColor: "#168653", downColor: "#d94b4b",
      borderUpColor: "#168653", borderDownColor: "#d94b4b",
      wickUpColor: "#168653", wickDownColor: "#d94b4b",
    });
    chartRef.current = chart;
    seriesRef.current = series;
    const resize = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (width && width > 0) chart.applyOptions({ width });
    });
    resize.observe(node);
    return () => {
      resize.disconnect();
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
      linesRef.current = [];
    };
  }, []);

  useEffect(() => {
    const series = seriesRef.current;
    if (!series) return;
    series.setData(validCandles.map(([time, candle]) => ({
      time: time as import("lightweight-charts").Time,
      open: candle.open, high: candle.high, low: candle.low, close: candle.close,
    })));
    const viewKey = `${symbol}:${provider}`;
    if (validCandles.length && fittedKeyRef.current !== viewKey) {
      chartRef.current?.timeScale().fitContent();
      fittedKeyRef.current = viewKey;
    }
  }, [validCandles, symbol, provider]);

  useEffect(() => {
    const series = seriesRef.current;
    if (!series) return;
    for (const line of linesRef.current) series.removePriceLine(line);
    linesRef.current = [];
    const levels = [
      { value: target, color: "#168653", title: "TP" },
      { value: entry, color: "#94a3b8", title: "ENTRY" },
      { value: stop, color: "#d94b4b", title: "SL" },
    ];
    for (const level of levels) {
      if (Number.isFinite(level.value) && level.value > 0) {
        linesRef.current.push(series.createPriceLine({
          price: level.value, color: level.color, lineWidth: 1, lineStyle: 2,
          axisLabelVisible: true, title: level.title,
        }));
      }
    }
  }, [entry, stop, target, validCandles]);

  if (instruments.isLoading || candles.isLoading) {
    return <div className="trade-preview-chart-state">Loading real market candles…</div>;
  }
  if (instruments.isError || candles.isError) {
    return <div className="trade-preview-chart-state" role="status">Market data is unavailable. The setup levels are still shown in the Setup tab.</div>;
  }
  if (!validCandles.length) {
    return <div className="trade-preview-chart-state" role="status">No historical candles are available for {symbol} from the configured provider. EdgeNest will not generate placeholder prices.</div>;
  }
  return (
    <div className="trade-preview-chart-wrap">
      <div ref={containerRef} className="trade-preview-chart" aria-label={symbol + " M15 candlestick preview with entry, stop loss and take profit levels"} />
      <div className="trade-preview-chart-meta">
        <span>{symbol} · M15</span>
        <span>{String(candles.data?.freshness ?? "provider freshness unavailable")}</span>
        <span>{candles.data?.count ?? validCandles.length} candles</span>
      </div>
    </div>
  );
}

export function TradeSetupPreview(props: TradeSetupPreviewProps) {
  const [tab, setTab] = useState<PreviewTab>("setup");
  const [copyState, setCopyState] = useState<"idle" | "copied" | "error">("idle");
  const isLong = props.direction === "long";
  const stopDistance = Math.abs(props.entry - props.stop);
  const targetDistance = Math.abs(props.target - props.entry);
  const distancesValid = stopDistance > 0 && targetDistance > 0 && Number.isFinite(stopDistance + targetDistance);
  const totalDistance = stopDistance + targetDistance;
  const profitPct = distancesValid ? targetDistance / totalDistance * 100 : 0;
  const lossPct = distancesValid ? stopDistance / totalDistance * 100 : 0;
  const entryPosition = isLong ? profitPct : lossPct;
  const topIsTarget = isLong;
  const topPrice = topIsTarget ? props.target : props.stop;
  const bottomPrice = topIsTarget ? props.stop : props.target;
  const topLabel = topIsTarget ? "TAKE PROFIT" : "STOP LOSS";
  const bottomLabel = topIsTarget ? "STOP LOSS" : "TAKE PROFIT";
  const topZone = topIsTarget ? "profit" : "loss";
  const bottomZone = topIsTarget ? "loss" : "profit";
  const topZonePct = topIsTarget ? profitPct : lossPct;
  const bottomZonePct = topIsTarget ? lossPct : profitPct;
  const hasRatio = props.rr !== null && Number.isFinite(props.rr) && props.rr > 0;
  const canCopy = props.valid && Number.isFinite(props.entry) && Number.isFinite(props.stop) && Number.isFinite(props.target);

  const copySetup = async () => {
    const lines = [
      `${props.symbol} · ${isLong ? "LONG" : "SHORT"}`,
      `Entry: ${price(props.entry, props.symbol)}`,
      `Stop Loss: ${price(props.stop, props.symbol)}`,
      `Take Profit: ${price(props.target, props.symbol)}`,
      `Position size: ${props.quantity}`,
      `Risk: ${money(props.risk, props.currency)}`,
      `Potential reward: ${money(props.reward, props.currency)}`,
      `Risk / Reward: ${props.rr === null ? "—" : "1 : " + props.rr.toFixed(2)}`,
    ];
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      setCopyState("copied");
    } catch {
      setCopyState("error");
    }
  };

  return (
    <section className="panel trade-setup-preview" aria-label="Trade setup preview">
      <div className="trade-preview-heading">
        <div>
          <span className="trade-preview-eyebrow">TRADE PLAN VISUALIZATION</span>
          <h2 className="panel-title">TRADE SETUP PREVIEW</h2>
        </div>
        <span className={props.valid ? "trade-preview-status is-valid" : "trade-preview-status"}>
          {props.valid ? "Calculated" : "Needs attention"}
        </span>
      </div>

      <div className="trade-preview-instrument">
        <div>
          <strong>{props.symbol}</strong>
          <span className={isLong ? "trade-preview-direction long" : "trade-preview-direction short"}>
            {isLong ? "↗ BUY / LONG" : "↘ SELL / SHORT"}
          </span>
        </div>
        <span className="trade-preview-quantity">{Number.isFinite(props.quantity) ? props.quantity.toLocaleString(undefined, { maximumFractionDigits: 8 }) : "—"} lots / units</span>
      </div>

      <div className="trade-preview-tabs" role="tablist" aria-label="Trade preview views">
        {(["setup", "chart", "analysis"] as const).map((view) => (
          <button key={view} type="button" role="tab" aria-selected={tab === view} className={tab === view ? "trade-preview-tab active" : "trade-preview-tab"} onClick={() => setTab(view)}>
            {view === "setup" ? "Setup" : view === "chart" ? "Chart" : "Analysis"}
          </button>
        ))}
      </div>

      {tab === "setup" ? (
        <div className="trade-preview-setup-view">
          {props.validationMessage ? <p className="trade-preview-validation" role="alert">{props.validationMessage}</p> : null}
          <div className="trade-preview-level-summary">
            <div><span>ENTRY PRICE</span><strong>{price(props.entry, props.symbol)}</strong></div>
            <span className="trade-preview-rr-pill">{props.rr !== null && Number.isFinite(props.rr) ? `1 : ${props.rr.toFixed(2)} R/R` : "R/R —"}</span>
          </div>
          <div className="trade-preview-plot" aria-label="Price-level visualization">
            <div className="trade-preview-zone-label top">
              <span className={topZone === "profit" ? "profit-text" : "loss-text"}>{topLabel}</span>
              <strong>{price(topPrice, props.symbol)}</strong>
            </div>
            <div className="trade-preview-track" style={{ height: 264 }}>
              <div className={`trade-preview-zone ${topZone}`} style={{ top: 0, height: `${topZonePct}%` }}>
                <span>{topZone === "profit" ? `+${money(props.reward, props.currency)}` : `−${money(props.risk, props.currency)}`}</span>
                <small>{topZone === "profit" ? `Potential reward · ${props.targetPips === null ? "—" : props.targetPips.toFixed(1) + " pips"}` : `Potential loss · ${props.stopPips === null ? "—" : props.stopPips.toFixed(1) + " pips"}`}</small>
              </div>
              <div className="trade-preview-entry-line" style={{ top: `${entryPosition}%` }}>
                <span>ENTRY</span><strong>{price(props.entry, props.symbol)}</strong>
              </div>
              <div className={`trade-preview-zone ${bottomZone}`} style={{ top: `${entryPosition}%`, height: `${bottomZonePct}%` }}>
                <span>{bottomZone === "profit" ? `+${money(props.reward, props.currency)}` : `−${money(props.risk, props.currency)}`}</span>
                <small>{bottomZone === "profit" ? `Potential reward · ${props.targetPips === null ? "—" : props.targetPips.toFixed(1) + " pips"}` : `Potential loss · ${props.stopPips === null ? "—" : props.stopPips.toFixed(1) + " pips"}`}</small>
              </div>
            </div>
            <div className="trade-preview-zone-label bottom">
              <span className={bottomZone === "profit" ? "profit-text" : "loss-text"}>{bottomLabel}</span>
              <strong>{price(bottomPrice, props.symbol)}</strong>
            </div>
          </div>
          {!distancesValid ? <p className="trade-preview-validation">Enter valid Stop Loss and Take Profit prices to complete the proportional setup visualization.</p> : null}
          <div className="trade-preview-outcomes">
            <div className="trade-preview-outcome loss-outcome"><span>MAXIMUM RISK</span><strong>{money(props.risk, props.currency)}</strong></div>
            <div className="trade-preview-outcome profit-outcome"><span>POTENTIAL REWARD</span><strong>{money(props.reward, props.currency)}</strong></div>
          </div>
          <div className="trade-preview-ratio">
            <div><span>RISK / REWARD</span><strong>{props.rr !== null && Number.isFinite(props.rr) ? `1 : ${props.rr.toFixed(2)}` : "—"}</strong></div>
            {hasRatio ? <div className="trade-preview-ratio-bar" aria-label="Risk and reward proportion">
              <span style={{ flex: 1 }} /><span style={{ flex: props.rr as number }} />
            </div> : <span className="trade-preview-ratio-empty">Complete the setup to calculate the ratio</span>}
          </div>
        </div>
      ) : null}

      {tab === "chart" ? (
        <div className="trade-preview-chart-view">
          <div className="trade-preview-chart-levels">
            <span><i className="entry-dot" /> Entry {price(props.entry, props.symbol)}</span>
            <span><i className="stop-dot" /> Stop {price(props.stop, props.symbol)}</span>
            <span><i className="target-dot" /> Target {price(props.target, props.symbol)}</span>
          </div>
          <PreviewCandlestickChart symbol={props.symbol} entry={props.entry} stop={props.stop} target={props.target} />
          <p className="trade-preview-disclaimer">Historical candles are retrieved from the configured EdgeNest market-data provider. This preview does not simulate or fabricate market prices.</p>
        </div>
      ) : null}

      {tab === "analysis" ? (
        <div className="trade-preview-analysis">
          <div className="trade-preview-analysis-grid">
            <div><span>Position size</span><strong>{props.quantity.toLocaleString(undefined, { maximumFractionDigits: 8 })}</strong></div>
            <div><span>Entry price</span><strong>{price(props.entry, props.symbol)}</strong></div>
            <div><span>Stop distance</span><strong>{props.stopPips === null ? "—" : props.stopPips.toFixed(2) + " pips"}</strong></div>
            <div><span>Target distance</span><strong>{props.targetPips === null ? "—" : props.targetPips.toFixed(2) + " pips"}</strong></div>
            <div><span>Estimated risk</span><strong className="loss-text">{money(props.risk, props.currency)}</strong></div>
            <div><span>Potential reward</span><strong className="profit-text">{money(props.reward, props.currency)}</strong></div>
            <div><span>Risk / reward</span><strong>{props.rr === null ? "—" : `1 : ${props.rr.toFixed(2)}`}</strong></div>
            <div><span>Risk / equity</span><strong>{props.riskPct === null ? "—" : props.riskPct.toFixed(3) + "%"}</strong></div>
            <div><span>Account balance / equity</span><strong>{props.balance === null ? "Not provided" : money(props.balance, props.currency)}</strong></div>
            <div><span>Trade direction</span><strong>{isLong ? "Long / Buy" : "Short / Sell"}</strong></div>
          </div>
          <p className="trade-preview-disclaimer">Potential outcomes are estimates before commissions, swaps, slippage and execution differences. They are not guaranteed returns or maximum-loss guarantees.</p>
        </div>
      ) : null}

      <div className="trade-preview-actions">
        <button type="button" className="btn" onClick={() => void copySetup()} disabled={!canCopy}> {copyState === "copied" ? "Copied setup" : "Copy trade setup"} </button>
        <Link className={props.valid ? "btn primary" : "btn primary disabled"} aria-disabled={!props.valid} tabIndex={props.valid ? 0 : -1} href={props.valid ? props.journalHref : "#"} onClick={(event) => { if (!props.valid) event.preventDefault(); }}>
          Add plan to Journal
        </Link>
      </div>
      {copyState === "error" ? <p className="trade-preview-validation" role="alert">Clipboard access was blocked. Select and copy the trade levels manually.</p> : null}
      <p className="trade-preview-disclaimer">Planning only · No order is placed or sent to a broker.</p>
    </section>
  );
}
