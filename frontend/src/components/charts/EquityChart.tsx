"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";
import { moneyToNumber } from "@/lib/format";
import type { EquityPoint } from "@/types/domain";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

function readCssVar(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  return value || fallback;
}

export function EquityChart({ series }: { series: EquityPoint[] }) {
  const { theme } = useTheme();

  const option = useMemo(() => {
    const muted = readCssVar("--chart-muted", "#a8b3c2");
    const dim = readCssVar("--chart-dim", "#7d8a9a");
    const grid = readCssVar("--chart-grid", "#1e2530");
    const axis = readCssVar("--chart-axis", "#2a3340");
    const line = readCssVar("--chart-line", "#4b93ff");
    const dd = readCssVar("--chart-dd", "#ef6b78");

    return {
      backgroundColor: "transparent",
      textStyle: {
        fontFamily: "var(--font-ibm-plex-sans), IBM Plex Sans, sans-serif",
        fontSize: 12,
      },
      grid: { left: 52, right: 20, top: 28, bottom: 36 },
      tooltip: { trigger: "axis" },
      legend: {
        data: ["Equity", "Drawdown %"],
        textStyle: { color: muted, fontSize: 12 },
        top: 0,
      },
      xAxis: {
        type: "category",
        data: series.map((p) => p.time),
        axisLabel: { color: dim, fontSize: 12 },
        axisLine: { lineStyle: { color: axis } },
      },
      yAxis: [
        {
          type: "value",
          scale: true,
          axisLabel: { color: dim, fontSize: 12 },
          splitLine: { lineStyle: { color: grid } },
        },
        {
          type: "value",
          axisLabel: { color: dim, fontSize: 12, formatter: "{value}%" },
          splitLine: { show: false },
        },
      ],
      series: [
        {
          name: "Equity",
          type: "line",
          smooth: true,
          showSymbol: false,
          data: series.map((p) => moneyToNumber(p.equity)),
          lineStyle: { color: line, width: 2 },
          areaStyle: {
            color:
              theme === "light"
                ? "rgba(31,111,235,0.08)"
                : "rgba(75,147,255,0.1)",
          },
        },
        {
          name: "Drawdown %",
          type: "line",
          yAxisIndex: 1,
          smooth: true,
          showSymbol: false,
          data: series.map((p) => p.drawdownPct),
          lineStyle: { color: dd, width: 1.5 },
        },
      ],
    };
  }, [series, theme]);

  return (
    <ReactECharts
      option={option}
      style={{ height: 300, width: "100%" }}
      opts={{ renderer: "canvas" }}
      notMerge
    />
  );
}
