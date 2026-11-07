import { apiRequest } from "@/lib/api/client";
import type { DataRepositories } from "@/lib/repositories/types";
import type { Account, Order, PipelineNode, PortfolioSnapshot, Position, ReconciliationResult, RiskDecisionRecord, RiskOverview, Strategy, SystemComponent, TradingEvent } from "@/types/domain";

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
    listAccounts: () => data<Account[]>("/api/v1/trading/accounts"),
    async getAccount(id) {
      return data<Account>(`/api/v1/trading/accounts/${encodeURIComponent(id)}`);
    },
  },
  portfolio: {
    getSnapshot: () => data<PortfolioSnapshot>("/api/v1/trading/portfolio"),
  },
  strategies: {
    listStrategies: () => data<Strategy[]>("/api/v1/trading/strategies"),
    async getStrategy(id) {
      return byId(await data<Strategy[]>("/api/v1/trading/strategies"), id, "Strategy");
    },
  },
  positions: {
    listPositions: () => data<Position[]>("/api/v1/trading/positions"),
  },
  orders: {
    listOrders: () => data<Order[]>("/api/v1/trading/orders?limit=500"),
    async getOrder(id) {
      return byId(await data<Order[]>("/api/v1/trading/orders?limit=500"), id, "Order");
    },
  },
  risk: {
    getOverview: () => data<RiskOverview>("/api/v1/trading/risk"),
    listDecisions: () => data<RiskDecisionRecord[]>("/api/v1/trading/risk/decisions"),
  },
  events: {
    listEvents: (limit = 50) => data<TradingEvent[]>(`/api/v1/trading/events?limit=${Math.max(1, Math.min(limit, 500))}`),
  },
  reconciliation: {
    getLatest: () => data<ReconciliationResult>("/api/v1/trading/reconciliation"),
  },
  system: {
    listComponents: () => data<SystemComponent[]>("/api/v1/trading/system/components"),
    getPipeline: () => data<PipelineNode[]>("/api/v1/trading/system/pipeline"),
  },
};
