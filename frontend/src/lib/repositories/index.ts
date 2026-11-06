import { frontendConfig } from "@/config/frontend";
import { mockRepositories } from "@/lib/repositories/mock";
import type { DataRepositories } from "@/lib/repositories/types";

const apiNotImplemented = async (): Promise<never> => {
  throw new Error(
    "This trading dashboard endpoint is not implemented in the Rust API yet. Mock data is disabled in API mode.",
  );
};

/**
 * API mode deliberately fails closed for dashboard resources whose backend
 * endpoints are not implemented. Never silently substitute demo fixtures.
 */
const unavailableApiRepositories: DataRepositories = {
  accounts: { listAccounts: apiNotImplemented, getAccount: apiNotImplemented },
  portfolio: { getSnapshot: apiNotImplemented },
  strategies: { listStrategies: apiNotImplemented, getStrategy: apiNotImplemented },
  positions: { listPositions: apiNotImplemented },
  orders: { listOrders: apiNotImplemented, getOrder: apiNotImplemented },
  risk: { getOverview: apiNotImplemented, listDecisions: apiNotImplemented },
  events: { listEvents: apiNotImplemented },
  reconciliation: { getLatest: apiNotImplemented },
  system: { listComponents: apiNotImplemented, getPipeline: apiNotImplemented },
};

export function getRepositories(): DataRepositories {
  return frontendConfig.dataMode === "api" ? unavailableApiRepositories : mockRepositories;
}
