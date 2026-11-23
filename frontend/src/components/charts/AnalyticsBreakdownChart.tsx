"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";
import type { JournalAnalytics } from "@/lib/api/journal";
import { usePerformanceDisplay } from "@/features/performance/PerformanceDisplayProvider";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });
type Group = NonNullable<JournalAnalytics["by_symbol"]>[number];

function css(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

function displayGroupLabel(value: string) {
  const labels: Record<string, string> = {
    london: "London",
    london_ny_overlap: "London–New York overlap",
    new_york: "New York",
    unassigned: "Unassigned",
    unspecified: "Unspecified",
  };
  return labels[value] ?? value.replaceAll("_", " ").replace(/\b\w/g, letter => letter.toUpperCase());
}

function formatMoney(value: number, currency: string) {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: 2,
  }).format(value);
}

export function AnalyticsBreakdownChart({
  title,
  groups,
  currency = "USD",
  baseline = 0,
  onSelect,
  sortBy = "pnl",
}: {
  title: string;
  groups: Group[];
  currency?: string;
  baseline?: number;
  onSelect?: (key: string) => void;
  sortBy?: "pnl" | "source";
}) {
  const { theme } = useTheme();
  const { mode } = usePerformanceDisplay();
  const ranked = useMemo(
    () =>
      groups
        .filter((group) => Number.isFinite(Number(group.net_pnl)))
        .slice()
        .sort((a, b) => sortBy === "pnl" ? Number(b.net_pnl) - Number(a.net_pnl) : 0),
    [groups, sortBy],
  );

  const option = useMemo(() => {
    const dim = css("--chart-dim", "#7d8a9a");
    const grid = css("--chart-grid", "#1e2530");
    const axis = css("--chart-axis", "#2a3340");
    const positive = css("--positive", theme === "light" ? "#1a7f4b" : "#4fd4a1");
    const negative = css("--negative", theme === "light" ? "#c6283a" : "#f07176");
    const values = ranked.map((group) => Number(group.net_pnl));
    const chartValues = mode==="percentage" && baseline>0 ? values.map(value=>value/baseline*100) : values;
    const shownMoney = (value:number) => mode==="currency" ? formatMoney(value,currency) : mode==="percentage" && baseline>0 ? (value/baseline*100).toFixed(2)+"%" : mode==="hidden" ? "••••••" : "—";
    const height = Math.max(220, Math.min(520, ranked.length * 38 + 82));

    return {
      backgroundColor: "transparent",
      animation: ranked.length < 100,
      textStyle: {
        fontFamily: '"Inter", -apple-system, "Segoe UI", sans-serif',
        fontSize: 12,
      },
      grid: { left: 18, right: 30, top: 18, bottom: 28, containLabel: true },
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        formatter: (items: Array<{ dataIndex: number }>) => {
          const group = ranked[items[0]?.dataIndex ?? -1];
          if (!group) return "";
          return [
            displayGroupLabel(group.key),
            "Net performance: " + shownMoney(Number(group.net_pnl)),
            "Trades: " + group.trades,
            "Win rate: " + (Number.isFinite(Number(group.win_rate_percent)) ? Number(group.win_rate_percent).toFixed(1) + "%" : "Not available"),
            "Expectancy: " + (group.expectancy == null ? "Not available" : shownMoney(Number(group.expectancy))),
          ].join("<br/>");
        },
      },
      xAxis: {
        type: "value",
        scale: true,
        axisLabel: { color: dim, formatter: (value: number) => shownMoney(mode==="percentage"&&baseline>0?value*baseline/100:value) },
        axisLine: { lineStyle: { color: axis } },
        splitLine: { lineStyle: { color: grid } },
      },
      yAxis: {
        type: "category",
        inverse: true,
        data: ranked.map((group) => displayGroupLabel(group.key)),
        axisLabel: { color: dim, width: 132, overflow: "truncate" },
        axisLine: { lineStyle: { color: axis } },
        axisTick: { show: false },
      },
      dataZoom: ranked.length > 12
        ? [
            { type: "inside", yAxisIndex: 0, startValue: 0, endValue: 11 },
            { type: "slider", yAxisIndex: 0, width: 12, right: 4, startValue: 0, endValue: 11, borderColor: axis, textStyle: { color: dim } },
          ]
        : [],
      series: [
        {
          name: mode==="percentage" ? "Net return" : "Net P&L",
          type: "bar",
          data: chartValues.map((value) => ({
            value,
            itemStyle: {
              color: value >= 0 ? positive : negative,
              borderRadius: value >= 0 ? [0, 4, 4, 0] : [4, 0, 0, 4],
            },
          })),
          barMaxWidth: 24,
          label: {
            show: ranked.length <= 12,
            position: "right",
            color: dim,
            formatter: (params: { value: number }) => shownMoney(mode==="percentage"&&baseline>0?params.value*baseline/100:params.value),
          },
          emphasis: { focus: "series" },
        },
      ],
      aria: { enabled: true, decal: { show: true } },
      animationDuration: 250,
      animationEasing: "cubicOut",
      __height: height,
    };
  }, [currency, ranked, theme, mode, baseline]);

  if (ranked.length === 0) {
    return <div className="analytics-chart-empty" role="status">No qualifying data for this breakdown.</div>;
  }

  const chartHeight = Number(option.__height ?? 260);
  const chartOption = { ...option };
  delete (chartOption as { __height?: number }).__height;

  return (
    <div className="analytics-ranked-chart" aria-label={title}>
      <div className="analytics-ranked-chart-heading">
        <h3>{title}</h3>
        <p>{sortBy === "pnl" ? "Ranked by net realized P&L" : "Ordered by time"} · Select a bar to investigate its trades</p>
      </div>
      <ReactECharts
        option={chartOption}
        onEvents={{
          click: (params: { dataIndex: number }) => {
            const group = ranked[params.dataIndex];
            if (group) onSelect?.(group.key);
          },
        }}
        style={{ height: chartHeight, width: "100%" }}
        opts={{ renderer: "canvas" }}
        notMerge
      />
    </div>
  );
}
