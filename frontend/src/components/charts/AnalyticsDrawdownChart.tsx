"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import type { JournalAnalytics } from "@/lib/api/journal";
import { useTheme } from "@/features/theme/ThemeProvider";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

export function AnalyticsDrawdownChart({ curve, currency = "USD" }: {
  curve: JournalAnalytics["equity_curve"];
  currency?: string;
}) {
  const { theme } = useTheme();
  const option = useMemo(() => {
    const muted = readToken("--chart-dim", theme === "light" ? "#5a687a" : "#a8abb2");
    const grid = readToken("--chart-grid", theme === "light" ? "#e4eaf2" : "#2a2d34");
    const negative = readToken("--negative", theme === "light" ? "#c6283a" : "#f07176");
    const dates = curve.map(point => new Date(point.closed_at).toLocaleDateString());
    const values = curve.map(point => -Math.abs(Number(point.drawdown ?? 0)));
    const format = (value: number) => value.toLocaleString(undefined, {
      style: "currency", currency, maximumFractionDigits: 2
    });
    return {
      animation: curve.length < 250,
      grid: { left: 72, right: 20, top: 20, bottom: 42 },
      tooltip: { trigger: "axis", formatter: (items: Array<{ dataIndex: number; value: number }>) => {
        const index = items[0]?.dataIndex ?? 0;
        return [dates[index], "Realized drawdown: " + format(values[index] ?? 0)].join("<br/>");
      } },
      xAxis: { type: "category", data: dates, boundaryGap: false, axisLabel: { color: muted, hideOverlap: true } },
      yAxis: { type: "value", max: 0, axisLabel: { color: muted, formatter: (value: number) => format(value) }, splitLine: { lineStyle: { color: grid } } },
      series: [{
        name: "Realized drawdown", type: "line", data: values, showSymbol: false,
        lineStyle: { color: negative, width: 2 },
        areaStyle: { color: theme === "light" ? "rgba(198,40,58,.10)" : "rgba(240,113,118,.13)" },
        markLine: { silent: true, symbol: "none", lineStyle: { color: muted, type: "dashed" }, data: [{ yAxis: 0 }] }
      }]
    };
  }, [curve, currency, theme]);

  return curve.length
    ? <ReactECharts option={option} style={{ height: 250, width: "100%" }} opts={{ renderer: "canvas" }} notMerge />
    : <p className="muted">No realized drawdown observations are available.</p>;
}

function readToken(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}
