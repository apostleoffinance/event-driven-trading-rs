"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";
import type { JournalAnalytics } from "@/lib/api/journal";
import { usePerformanceDisplay } from "@/features/performance/PerformanceDisplayProvider";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

function css(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

export function AnalyticsEquityChart({ curve, currency = "USD", baseline: suppliedBaseline }: { curve: JournalAnalytics["equity_curve"]; currency?: string; baseline?: number }) {
  const { theme } = useTheme();
  const { mode } = usePerformanceDisplay();
  const option = useMemo(() => {
    const muted = css("--chart-muted", "#a8b3c2");
    const dim = css("--chart-dim", "#7d8a9a");
    const grid = css("--chart-grid", "#1e2530");
    const axis = css("--chart-axis", "#2a3340");
    const line = css("--chart-line", "#4b93ff");
    const baseline = suppliedBaseline && suppliedBaseline > 0 ? suppliedBaseline : Number(curve[0]?.equity ?? 0);
    const shown = (v: unknown) => { const n=Number(v??0); if(mode==="hidden"||mode==="r")return "••••"; if(mode==="percentage")return n.toFixed(2)+"%"; return n.toLocaleString(undefined,{style:"currency",currency,maximumFractionDigits:2}); };
    const values = curve.map(point => Number(point.equity));
    const dates = curve.map(p => new Date(p.closed_at).toLocaleDateString("en-GB", { timeZone: "UTC" }));
    return {
      backgroundColor: "transparent",
      aria: { enabled: true, decal: { show: true } },
      animation: curve.length < 250,
      textStyle: { fontFamily: '"Inter", -apple-system, "Segoe UI", sans-serif', fontSize: 12 },
      grid: { left: 58, right: 24, top: 34, bottom: 42 },
      legend: { data: ["Cumulative Journal P&L"], top: 0, textStyle: { color: muted } },
      tooltip: {
        trigger: "axis",
        formatter: (items: Array<{seriesName:string; data:number; dataIndex:number}>) => {
          const i = items?.[0]?.dataIndex ?? 0;
          const point = curve[i];
          if (!point) return "";
          return [dates[i], ...items.map(item => item.seriesName + ": " + shown(item.data))].join("<br/>");
        }
      },
      xAxis: { type: "category", data: dates, boundaryGap: false, axisLabel: { color: dim, hideOverlap: true }, axisLine: { lineStyle: { color: axis } } },
      yAxis: { type: "value", scale: true, axisLabel: { color: dim, formatter: (v:number) => mode==="percentage"||mode==="hidden"||mode==="r" ? (mode==="percentage"?Number(v).toFixed(1)+"%":"••••") : shown(v) }, splitLine: { lineStyle: { color: grid } }, axisLine: { lineStyle: { color: axis } } },
      dataZoom: curve.length > 35 ? [{ type: "inside" }, { type: "slider", height: 16, bottom: 8, borderColor: axis, textStyle: { color: dim } }] : [],
      series: [
        { name: mode==="percentage" ? "Cumulative return" : "Cumulative Journal P&L", type: "line", data: mode==="percentage" ? values.map(value=>baseline>0?(value/baseline)*100:0) : values, showSymbol: curve.length < 30, symbolSize: 5, lineStyle: { color: line, width: 2.5 }, areaStyle: { color: theme === "light" ? "rgba(31,111,235,0.08)" : "rgba(75,147,255,0.10)" }, markLine: { silent: true, symbol: "none", lineStyle: { color: dim, type: "dashed" }, data: [{ yAxis: 0 }] } }
      ]
    };
  }, [curve, currency, theme, mode, suppliedBaseline]);
  return <ReactECharts option={option} style={{ height: 340, width: "100%" }} opts={{ renderer: "canvas" }} notMerge />;
}
