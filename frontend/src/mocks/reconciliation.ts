import type { ReconciliationResult } from "@/types/domain";

export const mockReconciliation: ReconciliationResult = {
  accountId: "prop-account-001",
  venueId: "prop",
  status: "ALERT",
  lastRunAt: "2026-09-06T21:10:00.000Z",
  lines: [
    {
      field: "Balance",
      internal: "99850.00",
      venue: "99849.50",
      status: "MISMATCH",
      difference: "0.50",
    },
    {
      field: "Equity",
      internal: "100482.31",
      venue: "100481.81",
      status: "MISMATCH",
      difference: "0.50",
    },
    { field: "Positions", internal: "2", venue: "2", status: "MATCH" },
    { field: "Open Orders", internal: "1", venue: "1", status: "MATCH" },
  ],
  alerts: [
    {
      title: "EQUITY MISMATCH",
      detail:
        "Internal equity 100482.31 vs venue 100481.81 (fee timing). Action remains ALERT ONLY — no silent overwrite.",
      action: "ALERT_ONLY",
    },
  ],
};
