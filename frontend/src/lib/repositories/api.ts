import { apiRequest } from "@/lib/api/client";
import type { DataRepositories } from "@/lib/repositories/types";

async function data<T>(path: string): Promise<T> {
  const response = await apiRequest<{ data: T }>(path);
  return response.data;
}

function byId<T extends { id: string }>(items: T[], id: string, kind: string): T {
  const found = items.find((item) => item.id === id);
  if (!found) throw new Error(`${kind} not found: ${id}`);
  return found;
}

/** API repositories use real, authenticated backend responses only. */
export const apiRepositories: DataRepositories = {
  accounts: {
    listAccounts: () => data("/api/v1/trading/accounts"),
    async getAccount(id) {
      return data(`/api/v1/trading/accounts/${encodeURIComponent(id)}`);
    },
  },
  portfolio: {
    getSnapshot: () => data("/api/v1/trading/portfolio"),
  },
  strategies: {
    listStrategies: () => data("/api/v1/trading/strategies"),
    async getStrategy(id) {
      return byId(await data("/api/v1/trading/strategies"), id, "Strategy");
    },
  },
  positions: {
    listPositions: () => data("/api/v1/trading/positions"),
  },
  orders: {
    listOrders: () => data("/api/v1/trading/orders?limit=500"),
    async getOrder(id) {
      return byId(await data("/api/v1/trading/orders?limit=500"), id, "Order");
    },
  },
  risk: {
    getOverview: () => data("/api/v1/trading/risk"),
    listDecisions: () => data("/api/v1/trading/risk/decisions"),
  },
  events: {
    listEvents: (limit = 50) => data(`/api/v1/trading/events?limit=${Math.max(1, Math.min(limit, 500))}`),
  },
  reconciliation: {
    getLatest: () => data("/api/v1/trading/reconciliation"),
  },
  system: {
    listComponents: () => data("/api/v1/trading/system/components"),
    getPipeline: () => data("/api/v1/trading/system/pipeline"),
  },
};
