"use client";

import type { OrderStatus } from "@/types/domain";

type StepKey =
  | "TradeIntent"
  | "Risk"
  | "Created"
  | "Submitted"
  | "Accepted"
  | "Filled";

const ORDER: StepKey[] = [
  "TradeIntent",
  "Risk",
  "Created",
  "Submitted",
  "Accepted",
  "Filled",
];

function reachedIndex(status: OrderStatus): number {
  switch (status) {
    case "Created":
      return 2;
    case "PendingRisk":
    case "Approved":
      return 2;
    case "Rejected":
      return 1;
    case "Submitted":
    case "Unknown":
      return 3;
    case "Accepted":
    case "CancelPending":
    case "Cancelled":
      return 4;
    case "PartiallyFilled":
    case "Filled":
      return 5;
    case "Failed":
      return 3;
    default:
      return 0;
  }
}

function nodeClass(status: OrderStatus, key: StepKey, index: number): string {
  if (status === "Unknown" && key === "Submitted") return "lifecycle-node unknown";
  if (status === "Rejected" && key === "Risk") return "lifecycle-node bad";
  if (status === "Failed" && (key === "Submitted" || key === "Created"))
    return "lifecycle-node bad";
  const reached = reachedIndex(status);
  if (index < reached) return "lifecycle-node done";
  if (index === reached) return "lifecycle-node active";
  return "lifecycle-node idle";
}

export function OrderLifecycleChain({ status }: { status: OrderStatus }) {
  return (
    <div className="lifecycle" role="list" aria-label="Order lifecycle">
      {ORDER.map((key, i) => (
        <div key={key} style={{ display: "contents" }}>
          {i > 0 ? <div className="pipeline-arrow">→</div> : null}
          <div className={nodeClass(status, key, i)} role="listitem">
            <div className="metric-label">{key}</div>
            {status === "Unknown" && key === "Submitted" ? (
              <div className="metric-sub">UNKNOWN — reconcile</div>
            ) : null}
            {status === "PartiallyFilled" && key === "Filled" ? (
              <div className="metric-sub">partial</div>
            ) : null}
          </div>
        </div>
      ))}
    </div>
  );
}
