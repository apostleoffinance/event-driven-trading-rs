"use client";
import { useEffect, useState } from "react";
import { DashboardWorkspace } from "@/components/dashboard/DashboardWorkspace";
import { useSharedEventStream } from "@/features/events/EventStreamProvider";
import { getRepositories } from "@/lib/repositories";
import { hasSession } from "@/lib/auth/session";
import type { Account, Order, PortfolioSnapshot, Position, RiskOverview } from "@/types/domain";
import { useOrganization } from "@/features/organization/OrganizationProvider";

export default function CommandCenterPage() {
  const repos = getRepositories();
  const { organizations, hasMemberships, activeOrganizationId, loading: organizationLoading, error: organizationError, refreshOrganizations, createOrganization } = useOrganization();
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
  const [retryVersion, setRetryVersion] = useState(0);
  useEffect(() => { setAuthenticated(hasSession()); setAuthResolved(true); }, []);
  useEffect(() => {
    if (!authenticated || organizationLoading) return;
    if (!activeOrganizationId) {
      setAccounts([]); setPortfolio(null); setPositions([]); setOrders([]); setRisk(null);
      setError(organizationError ?? "Your account is not connected to an active organization. Ask an organization owner for access.");
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
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
      setLoading(false);
    };
    void load();
    return () => { cancelled = true; };
  }, [authenticated,organizationLoading,activeOrganizationId,organizationError,repos.accounts,repos.portfolio,repos.positions,repos.orders,repos.risk,retryVersion]);
  if (!authResolved || !authenticated) return null;
  if (organizationLoading) return <div className="dashboard-loading" role="status">Loading your organization…</div>;
  return <DashboardWorkspace executionAccounts={accounts} portfolio={portfolio} positions={positions} orders={orders} risk={risk} events={events} loading={loading} error={error ?? (!activeOrganizationId ? organizationError ?? "Your account is not connected to an active organization." : null)} canCreateWorkspace={!organizationError && organizations.length === 0 && !hasMemberships} onCreateWorkspace={createOrganization} onRetry={() => { void refreshOrganizations(); setRetryVersion(value => value + 1); }} />;
}
