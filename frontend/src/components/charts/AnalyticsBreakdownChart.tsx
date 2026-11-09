"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";
import type { JournalAnalytics } from "@/lib/api/journal";
import { formatNumber } from "@/lib/format";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });
type Group = NonNullable<JournalAnalytics["by_symbol"]>[number];

function css(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

export function AnalyticsBreakdownChart({ title, groups, currency = "USD", onSelect }: { title: string; groups: Group[]; currency?: string; onSelect?: (key: string) => void }) {
  const { theme } = useTheme();
  const option = useMemo(() => {
    const dim = css("--chart-dim", "#7d8a9a");
    const grid = css("--chart-grid", "#1e2530");
    const axis = css("--chart-axis", "#2a3340");
    const line = css("--chart-line", "#4b93ff");
    const positive = css("--positive", "#4fd4a1");
    const negative = css("--negative", "#f07176");
    const pnl = groups.map(g => Number(g.net_pnl));
    const wins = groups.map(g => Number(g.win_rate_percent));
    return {
      backgroundColor: "transparent",
      textStyle: { fontFamily: '"IBM Plex Sans", -apple-system, "Segoe UI", sans-serif', fontSize: 12 },
      title: { text: title, left: 8, top: 0, textStyle: { fontSize: 13, fontWeight: 600, color: dim } },
      grid: { left: 60, right: 50, top: 36, bottom: 48 },
      tooltip: { trigger: "axis", axisPointer: { type: "shadow" }, formatter: (items: Array<{seriesName:string;value:number;dataIndex:number}>) => { const g=groups[items[0]?.dataIndex ?? 0]; return g ? [g.key, "Trades: "+g.trades, "Net P&L: "+new Intl.NumberFormat(undefined,{style:"currency",currency,maximumFractionDigits:2}).format(Number(g.net_pnl)), "Win rate: "+Number(g.win_rate_percent).toFixed(1)+"%", "Expectancy: "+new Intl.NumberFormat(undefined,{style:"currency",currency,maximumFractionDigits:2}).format(Number(g.expectancy))].join("<br/>") : ""; } },
      legend: { data: ["Net P&L", "Win rate"], top: 18, right: 8, textStyle: { color: dim } },
      xAxis: { type: "category", data: groups.map(g => g.key), axisLabel: { color: dim, rotate: groups.length > 6 ? 25 : 0, interval: "auto", hideOverlap: true }, axisLine: { lineStyle: { color: axis } } },
      yAxis: [{ type: "value", scale: true, axisLabel: { color: dim, formatter: (v:number) => new Intl.NumberFormat(undefined, { style: "currency", currency, maximumFractionDigits: 2 }).format(v) }, splitLine: { lineStyle: { color: grid } } }, { type: "value", min: 0, max: 100, axisLabel: { color: dim, formatter: (v:number) => formatNumber(v, 1) + "%" }, splitLine: { show: false } }],
      dataZoom: groups.length > 8 ? [{ type: "inside" }, { type: "slider", height: 14, bottom: 4, textStyle: { color: dim } }] : [],
      series: [
        { name: "Net P&L", type: "bar", data: pnl, itemStyle: { color: (params: { value: number }) => params.value >= 0 ? positive : negative, borderRadius: [3, 3, 0, 0] }, barMaxWidth: 34, emphasis: { focus: "series" } },
        { name: "Win rate", type: "line", yAxisIndex: 1, data: wins, smooth: false, symbolSize: 6, lineStyle: { color: theme === "light" ? "#c77719" : "#f4b740", width: 2.5 } }
      ]
    };
  }, [currency, groups, theme, title]);
  return <ReactECharts option={option} onEvents={{ click: (params: { dataIndex: number }) => { const group = groups[params.dataIndex]; if (group) onSelect?.(group.key); } }} style={{ height: 320, width: "100%" }} opts={{ renderer: "canvas" }} notMerge />;
}
