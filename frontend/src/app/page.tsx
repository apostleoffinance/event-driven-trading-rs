"use client";

import { useEffect, useState } from "react";
import { DashboardWorkspace } from "@/components/dashboard/DashboardWorkspace";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { getRepositories } from "@/lib/repositories";
import type { Account, Order, PortfolioSnapshot, Position, RiskOverview } from "@/types/domain";

export default function CommandCenterPage() {
  const repos = getRepositories();
  const { events } = useSharedEventStream();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [portfolio, setPortfolio] = useState<PortfolioSnapshot | null>(null);
  const [positions, setPositions] = useState<Position[]>([]);
  const [orders, setOrders] = useState<Order[]>([]);
  const [risk, setRisk] = useState<RiskOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      const results = await Promise.allSettled([
        repos.accounts.listAccounts(),
        repos.portfolio.getSnapshot(),
        repos.positions.listPositions(),
        repos.orders.listOrders(),
        repos.risk.getOverview(),
      ]);
      if (cancelled) return;
      const [accountResult, portfolioResult, positionsResult, ordersResult, riskResult] = results;
      if (accountResult.status === "fulfilled") setAccounts(accountResult.value);
      if (portfolioResult.status === "fulfilled") setPortfolio(portfolioResult.value);
      if (positionsResult.status === "fulfilled") setPositions(positionsResult.value);
      if (ordersResult.status === "fulfilled") setOrders(ordersResult.value.slice(0, 8));
      if (riskResult.status === "fulfilled") setRisk(riskResult.value);
      const failures = results.filter((result): result is PromiseRejectedResult => result.status === "rejected");
      if (failures.length) setError(failures.map(result => result.reason instanceof Error ? result.reason.message : "An execution data source is unavailable").join(" · "));
      setLoading(false);
    };
    void load();
    return () => { cancelled = true; };
  }, [repos.accounts, repos.portfolio, repos.positions, repos.orders, repos.risk]);

  return <DashboardWorkspace
    executionAccounts={accounts}
    portfolio={portfolio}
    positions={positions}
    orders={orders}
    risk={risk}
    events={events}
    loading={loading}
    error={error}
  />;
}
