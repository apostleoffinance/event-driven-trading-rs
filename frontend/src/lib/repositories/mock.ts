import { mockAccounts } from "@/mocks/accounts";
import { mockEvents } from "@/mocks/events";
import { mockOrders } from "@/mocks/orders";
import { mockPortfolio } from "@/mocks/portfolio";
import { mockPositions } from "@/mocks/positions";
import { mockReconciliation } from "@/mocks/reconciliation";
import { mockRiskDecisions, mockRiskOverview } from "@/mocks/risk";
import { mockStrategies } from "@/mocks/strategies";
import { mockPipeline, mockSystemComponents } from "@/mocks/system";
import type { DataRepositories } from "@/lib/repositories/types";

const delay = async <T>(value: T, ms = 120): Promise<T> => {
  await new Promise((r) => setTimeout(r, ms));
  return value;
};

export const mockRepositories: DataRepositories = {
  accounts: {
    async listAccounts() {
      return delay([...mockAccounts]);
    },
    async getAccount(id) {
      const account = mockAccounts.find((a) => a.id === id);
      if (!account) throw new Error(`Account not found: ${id}`);
      return delay({ ...account });
    },
  },
  portfolio: {
    async getSnapshot() {
      return delay({ ...mockPortfolio, equitySeries: [...mockPortfolio.equitySeries] });
    },
  },
  strategies: {
    async listStrategies() {
      return delay([...mockStrategies]);
    },
    async getStrategy(id) {
      const s = mockStrategies.find((x) => x.id === id);
      if (!s) throw new Error(`Strategy not found: ${id}`);
      return delay({ ...s });
    },
  },
  positions: {
    async listPositions() {
      return delay([...mockPositions]);
    },
  },
  orders: {
    async listOrders() {
      return delay([...mockOrders]);
    },
    async getOrder(id) {
      const o = mockOrders.find((x) => x.id === id);
      if (!o) throw new Error(`Order not found: ${id}`);
      return delay({ ...o });
    },
  },
  risk: {
    async getOverview() {
      return delay({ ...mockRiskOverview });
    },
    async listDecisions() {
      return delay([...mockRiskDecisions]);
    },
  },
  events: {
    async listEvents(limit = 50) {
      return delay(mockEvents.slice(0, limit));
    },
  },
  reconciliation: {
    async getLatest() {
      return delay({
        ...mockReconciliation,
        lines: [...mockReconciliation.lines],
        alerts: [...mockReconciliation.alerts],
      });
    },
  },
  system: {
    async listComponents() {
      return delay([...mockSystemComponents]);
    },
    async getPipeline() {
      return delay([...mockPipeline]);
    },
  },
};
