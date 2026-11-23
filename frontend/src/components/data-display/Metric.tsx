import { formatPct } from "@/lib/format";
import { formatPerformanceValue, usePerformanceDisplay } from "@/features/performance/PerformanceDisplayProvider";

export function Metric({
  label,
  value,
  sub,
  signed,
  pct,
}: {
  label: string;
  value: string;
  sub?: string;
  signed?: boolean;
  pct?: number;
}) {
  const { mode } = usePerformanceDisplay();
  const display = Number.isFinite(Number(value)) ? formatPerformanceValue(value,"USD",mode) : value;
  const cls =
    signed && Number(value) !== 0
      ? Number(value) > 0
        ? "pos"
        : "neg"
      : undefined;

  return (
    <div className="metric panel">
      <div className="metric-label">{label}</div>
      <div className={`metric-value ${cls ?? ""}`}>{display}</div>
      {sub ? <div className="metric-sub">{sub}</div> : null}
      {pct !== undefined ? (
        <div className={`metric-sub ${pct >= 0 ? "pos" : "neg"}`}>
          {formatPct(pct)} today
        </div>
      ) : null}
    </div>
  );
}
