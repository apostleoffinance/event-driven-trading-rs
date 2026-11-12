"use client";

import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";
import type { JournalAnalytics } from "@/lib/api/journal";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });
type Group = NonNullable<JournalAnalytics["by_symbol"]>[number];

function css(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
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
  onSelect,
}: {
  title: string;
  groups: Group[];
  currency?: string;
  onSelect?: (key: string) => void;
}) {
  const { theme } = useTheme();
  const ranked = useMemo(
    () =>
      groups
        .filter((group) => Number.isFinite(Number(group.net_pnl)))
        .slice()
        .sort((a, b) => Number(b.net_pnl) - Number(a.net_pnl)),
    [groups],
  );

  const option = useMemo(() => {
    const dim = css("--chart-dim", "#7d8a9a");
    const grid = css("--chart-grid", "#1e2530");
    const axis = css("--chart-axis", "#2a3340");
    const positive = css("--positive", theme === "light" ? "#1a7f4b" : "#4fd4a1");
    const negative = css("--negative", theme === "light" ? "#c6283a" : "#f07176");
    const values = ranked.map((group) => Number(group.net_pnl));
    const height = Math.max(220, Math.min(520, ranked.length * 38 + 82));

    return {
      backgroundColor: "transparent",
      animation: ranked.length < 100,
      textStyle: {
        fontFamily: '"IBM Plex Sans", -apple-system, "Segoe UI", sans-serif',
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
            group.key,
            "Net P&L: " + formatMoney(Number(group.net_pnl), currency),
            "Trades: " + group.trades,
            "Win rate: " + (Number.isFinite(Number(group.win_rate_percent)) ? Number(group.win_rate_percent).toFixed(1) + "%" : "Not available"),
            "Expectancy: " + (group.expectancy == null ? "Not available" : formatMoney(Number(group.expectancy), currency)),
          ].join("<br/>");
        },
      },
      xAxis: {
        type: "value",
        scale: true,
        axisLabel: { color: dim, formatter: (value: number) => formatMoney(value, currency) },
        axisLine: { lineStyle: { color: axis } },
        splitLine: { lineStyle: { color: grid } },
      },
      yAxis: {
        type: "category",
        inverse: true,
        data: ranked.map((group) => group.key),
        axisLabel: { color: dim, width: 132, overflow: "truncate" },
        axisLine: { lineStyle: { color: axis } },
        axisTick: { show: false },
      },
      series: [
        {
          name: "Net P&L",
          type: "bar",
          data: values.map((value) => ({
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
            formatter: (params: { value: number }) => formatMoney(params.value, currency),
          },
          emphasis: { focus: "series" },
        },
      ],
      aria: { enabled: true, decal: { show: true } },
      animationDuration: 250,
      animationEasing: "cubicOut",
      __height: height,
    };
  }, [currency, ranked, theme]);

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
        <p>Ranked by net realized P&amp;L · Select a bar to investigate its trades</p>
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
