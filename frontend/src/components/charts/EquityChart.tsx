"use client";

import dynamic from "next/dynamic";
import { moneyToNumber } from "@/lib/format";
import type { EquityPoint } from "@/types/domain";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

export function EquityChart({ series }: { series: EquityPoint[] }) {
  const option = {
    backgroundColor: "transparent",
    grid: { left: 48, right: 16, top: 24, bottom: 32 },
    tooltip: { trigger: "axis" },
    legend: {
      data: ["Equity", "Drawdown %"],
      textStyle: { color: "#8b97a8", fontSize: 11 },
      top: 0,
    },
    xAxis: {
      type: "category",
      data: series.map((p) => p.time),
      axisLabel: { color: "#5f6b7a" },
      axisLine: { lineStyle: { color: "#2a3340" } },
    },
    yAxis: [
      {
        type: "value",
        scale: true,
        axisLabel: { color: "#5f6b7a" },
        splitLine: { lineStyle: { color: "#1e2530" } },
      },
      {
        type: "value",
        axisLabel: { color: "#5f6b7a", formatter: "{value}%" },
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
        lineStyle: { color: "#3d8bfd", width: 2 },
        areaStyle: { color: "rgba(61,139,253,0.08)" },
      },
      {
        name: "Drawdown %",
        type: "line",
        yAxisIndex: 1,
        smooth: true,
        showSymbol: false,
        data: series.map((p) => p.drawdownPct),
        lineStyle: { color: "#e35d6a", width: 1.5 },
      },
    ],
  };

  return (
    <ReactECharts
      option={option}
      style={{ height: 280, width: "100%" }}
      opts={{ renderer: "canvas" }}
    />
  );
}
