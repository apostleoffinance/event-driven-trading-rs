"use client";
import { useEffect, useState } from "react";
import { DashboardWorkspace } from "@/components/dashboard/DashboardWorkspace";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { getRepositories } from "@/lib/repositories";
import { hasSession } from "@/lib/auth/session";
import type { Account, Order, PortfolioSnapshot, Position, RiskOverview } from "@/types/domain";

export default function CommandCenterPage() {
  const repos = getRepositories();
  const { events } = useSharedEventStream();
  const [authenticated, setAuthenticated] = useState(false);
  const [authResolved, setAuthResolved] = useState(false);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [portfolio, setPortfolio] = useState<PortfolioSnapshot | null>(null);
  const [positions, setPositions] = useState<Position[]>([]);
  const [orders, setOrders] = useState<Order[]>([]);
  const [risk, setRisk] = useState<RiskOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  useEffect(() => { setAuthenticated(hasSession()); setAuthResolved(true); }, []);
  useEffect(() => {
    if (!authenticated) return;
    let cancelled = false;
    const load = async () => {
      const results = await Promise.allSettled([repos.accounts.listAccounts(),repos.portfolio.getSnapshot(),repos.positions.listPositions(),repos.orders.listOrders(),repos.risk.getOverview()]);
      if (cancelled) return;
      const [a,p,pos,o,r] = results;
      if (a.status === "fulfilled") setAccounts(a.value);
      if (p.status === "fulfilled") setPortfolio(p.value);
      if (pos.status === "fulfilled") setPositions(pos.value);
      if (o.status === "fulfilled") setOrders(o.value.slice(0,8));
      if (r.status === "fulfilled") setRisk(r.value);
      const failures = results.filter((x): x is PromiseRejectedResult => x.status === "rejected");
      if (failures.length) setError(failures.map(x => x.reason instanceof Error ? x.reason.message : "An execution data source is unavailable").join(" · "));
      setLoading(false);
    };
    void load();
    return () => { cancelled = true; };
  }, [authenticated,repos.accounts,repos.portfolio,repos.positions,repos.orders,repos.risk]);
  if (!authResolved || !authenticated) return null;
  return <DashboardWorkspace executionAccounts={accounts} portfolio={portfolio} positions={positions} orders={orders} risk={risk} events={events} loading={loading} error={error} />;
}
