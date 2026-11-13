"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";
import { formatMoney, moneyToNumber } from "@/lib/format";
import type { JournalAnalytics } from "@/lib/api/journal";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

export function JournalEquityCurve({ analytics, currency }: { analytics: JournalAnalytics; currency: string }) {
  const { theme } = useTheme();
  const option = useMemo(() => {
    const styles = typeof document === "undefined" ? null : getComputedStyle(document.documentElement);
    const css = (key: string, fallback: string) => styles?.getPropertyValue(key).trim() || fallback;
    const line = css("--chart-line", "#1f6feb");
    const axis = css("--chart-axis", "#d5dde8");
    const muted = css("--chart-dim", "#5a687a");
    const grid = css("--chart-grid", "#e4eaf2");
    const points = [...analytics.equity_curve].sort((a,b)=>a.closed_at.localeCompare(b.closed_at));
    return {
      backgroundColor: "transparent",
      animationDuration: 350,
      grid: { left: 64, right: 22, top: 24, bottom: 42 },
      tooltip: { trigger: "axis", formatter: (items: Array<{axisValue:string;value:number;seriesName:string}>) => {
        return items.length ? items[0].axisValue + items.map(item => "<br/>" + item.seriesName + ": " + formatMoney(item.value, currency)).join("") : "";
      }},
      xAxis: { type: "category", boundaryGap: false, data: points.map(p=>new Date(p.closed_at).toLocaleDateString()), axisLabel: { color: muted, hideOverlap: true }, axisLine: { lineStyle: { color: axis } } },
      yAxis: { type: "value", scale: true, axisLabel: { color: muted, formatter: (v:number)=>formatMoney(v,currency) }, splitLine: { lineStyle: { color: grid } } },
      legend: { data: ["Cumulative realised P&L", "Drawdown from peak"], textStyle: { color: muted, fontSize: 11 }, top: 0 },\n      series: [{ name: "Cumulative realised P&L", type: "line", smooth: true, showSymbol: false, data: points.map(p=>moneyToNumber(p.equity)), lineStyle: { color: line, width: 2.5 }, areaStyle: { color: theme === "light" ? "rgba(31,111,235,0.09)" : "rgba(107,163,247,0.12)" } }, { name: "Drawdown from peak", type: "line", smooth: true, showSymbol: false, data: points.map(p=>moneyToNumber(p.drawdown)), lineStyle: { color: css("--chart-dd", "#c6283a"), width: 1.8 } }],
    };
  }, [analytics, currency, theme]);
  return <ReactECharts option={option} style={{height:280,width:"100%"}} opts={{renderer:"canvas"}} notMerge />;
}
