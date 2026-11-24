import type { HTMLAttributes } from "react";
import { InfoTooltip } from "@/components/data-display/InfoTooltip";

type MetricCardProps = HTMLAttributes<HTMLDivElement> & {
  label: string;
  value: string;
  sub?: string;
  tone?: "neutral" | "positive" | "negative";
  help?: string;
};

export function MetricCard({
  label,
  value,
  sub,
  tone = "neutral",
  help,
  className,
  ...props
}: MetricCardProps) {
  const classes = ["metric-card", className].filter(Boolean).join(" ");

  return (
    <div className={classes} {...props}>
      <span className="metric-card__label">
        {label}
        {help ? <InfoTooltip label={label} content={help} /> : null}
      </span>
      <span className="metric-card__value" data-tone={tone}>
        {value}
      </span>
      {sub ? <span className="metric-card__sub">{sub}</span> : null}
    </div>
  );
}
