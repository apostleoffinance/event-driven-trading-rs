"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";
import { usePerformanceDisplay } from "@/features/performance/PerformanceDisplayProvider";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

export type HourlyAnalyticsGroup = {
  key: string;
  trades: number;
  net_pnl: string | number;
  win_rate_percent: string | number;
};

export function HourlyPerformanceHeatmap({ groups, currency = "USD", baseline = 0, onSelect }: {
  groups: HourlyAnalyticsGroup[];
  currency?: string;
  baseline?: number;
  onSelect?: (hour: string) => void;
}) {
  const { theme } = useTheme();
  const { mode } = usePerformanceDisplay();
  const option = useMemo(() => {
    const positive = getComputedStyleSafe("--positive", theme === "light" ? "#1a7f4b" : "#4fd4a1");
    const negative = getComputedStyleSafe("--negative", theme === "light" ? "#c6283a" : "#f07176");
    const muted = getComputedStyleSafe("--chart-dim", "#7d8a9a");
    const grid = getComputedStyleSafe("--chart-grid", "#e4eaf2");
    const hours = Array.from({ length: 24 }, (_, hour) => String(hour).padStart(2, "0") + ":00");
    const byHour = new Map(groups.map(group => [normaliseHour(group.key), group]));
    const values = hours.map(hour => {
      const group = byHour.get(hour);
      return group ? Number(group.net_pnl) : null;
    });
    const displayValues = values.map(value=>value==null?null:mode==="percentage"&&baseline>0?value/baseline*100:value);
    const finite = displayValues.filter((value): value is number => value != null && Number.isFinite(value));
    const extent = Math.max(1, ...finite.map(Math.abs));
    return {
      animation: false,
      aria: { enabled: true, decal: { show: true } },
      textStyle: { fontFamily: '"IBM Plex Sans", -apple-system, "Segoe UI", sans-serif', fontSize: 12 },
      tooltip: {
        position: "top",
        formatter: (params: { data: [number, number, number | null] }) => {
          const [index, , value] = params.data;
          const group = byHour.get(hours[index]);
          if (!group) return hours[index] + " UTC<br/>No trades recorded";
          return [
            hours[index] + " UTC",
            "Trades: " + group.trades,
            "Sample size: " + group.trades + " closed trades",
            "Net performance: " + (mode==="currency" ? Number(value).toLocaleString(undefined,{style:"currency",currency,maximumFractionDigits:2}) : mode==="percentage"&&baseline>0 ? (Number(value)/baseline*100).toFixed(2)+"%" : mode==="hidden" ? "••••••" : "—"),
            "Win rate: " + Number(group.win_rate_percent).toFixed(1) + "%"
          ].join("<br/>");
        }
      },
      grid: { left: 54, right: 24, top: 24, bottom: 44 },
      xAxis: { type: "category", data: hours, splitArea: { show: true }, axisLabel: { color: muted, interval: 1, rotate: 35 }, axisLine: { lineStyle: { color: grid } } },
      yAxis: { type: "category", data: ["Net P&L"], axisLabel: { color: muted }, splitArea: { show: true } },
      visualMap: { min: -extent, max: extent, calculable: false, orient: "horizontal", left: "center", bottom: 0, itemWidth: 12, itemHeight: 120, text: ["Profit", "Loss"], textStyle: { color: muted }, inRange: { color: [negative, "#eef2f7", positive] } },
      series: [{
        type: "heatmap",
        data: hours.map((_, index) => [index, 0, displayValues[index]]),
        label: { show: false },
        itemStyle: { borderColor: theme === "light" ? "#ffffff" : "#17181c", borderWidth: 2, borderRadius: 3 },
        emphasis: { itemStyle: { borderColor: muted, borderWidth: 2, shadowBlur: 6, shadowColor: "rgba(0,0,0,.15)" } }
      }]
    };
  }, [groups, currency, theme, mode, baseline]);

  return groups.length
    ? <ReactECharts option={option} onEvents={{ click: (params: { dataIndex: number }) => onSelect?.(String(params.dataIndex).padStart(2, "0") + ":00") }} style={{ height: 190, width: "100%" }} opts={{ renderer: "canvas" }} notMerge />
    : <p className="muted">No hourly trading activity in this period.</p>;
}

function normaliseHour(value: string) {
  const match = value.match(/\b(\d{1,2})(?::\d{2})?/);
  return match ? String(Number(match[1])).padStart(2, "0") + ":00" : value;
}

function getComputedStyleSafe(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}
