import type { HealthStatus, OrderStatus, RiskDecisionKind, RiskTone } from "@/types/domain";

export function StatusBadge({
  tone,
  children,
}: {
  tone: "ok" | "warn" | "bad" | "info";
  children: React.ReactNode;
}) {
  return <span className={`badge ${tone}`}>{children}</span>;
}

export function healthTone(status: HealthStatus): "ok" | "warn" | "bad" {
  if (status === "HEALTHY") return "ok";
  if (status === "DEGRADED") return "warn";
  return "bad";
}

export function orderTone(status: OrderStatus): "ok" | "warn" | "bad" | "info" {
  if (status === "Filled" || status === "Accepted" || status === "Approved")
    return "ok";
  if (status === "Rejected" || status === "Failed") return "bad";
  if (status === "CancelPending" || status === "Cancelled") return "warn";
  return "info";
}

export function riskDecisionTone(
  decision: RiskDecisionKind,
): "ok" | "warn" | "bad" | "info" {
  if (decision === "APPROVED") return "ok";
  if (decision === "RESIZED") return "info";
  if (decision === "REJECTED") return "bad";
  return "warn";
}

export function riskToneClass(tone: RiskTone): "ok" | "warn" | "bad" {
  if (tone === "HEALTHY") return "ok";
  if (tone === "WARNING") return "warn";
  return "bad";
}
