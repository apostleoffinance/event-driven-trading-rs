"use client";

import { useMemo, type ReactNode } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import Link from "next/link";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "@/components/data-display/DataTable";
import { riskDecisionTone, riskToneClass, StatusBadge } from "@/components/data-display/StatusBadge";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatMoney, formatQuantity, formatSignedMoney } from "@/lib/format";
import { getRepositories } from "@/lib/repositories";
import { journalApi, type JournalRiskPolicyEnvelope } from "@/lib/api/journal";
import type { Position, RiskDecisionRecord } from "@/types/domain";

const amount = (value: string | number | null | undefined) => {
  const n = Number(value ?? 0);
  return Number.isFinite(n) ? n : 0;
};
const numeric = (value: unknown): number | null => {
  if (value == null || value === "") return null;
  const parsed = typeof value === "number" ? value : Number(value);
  return Number.isFinite(parsed) ? parsed : null;
};
const pct = (value: unknown) => { const parsed = numeric(value); return parsed == null ? "—" : `${parsed.toFixed(2)}%`; };
const limit = (value: unknown) => { const parsed = numeric(value); return parsed == null ? "Not set" : `${parsed.toFixed(2)}%`; };
const fixed = (value: unknown, digits = 2, suffix = "") => { const parsed = numeric(value); return parsed == null ? "—" : `${parsed.toFixed(digits)}${suffix}`; };

function MetricCard({ label, value, detail, tone = "neutral", progress }: {
  label: string; value: string; detail: string; tone?: "neutral" | "good" | "warning" | "danger"; progress?: number | null;
}) {
  return <article className={`risk-center-metric tone-${tone}`}>
    <div className="risk-center-metric-top"><span>{label}</span><i /></div>
    <strong>{value}</strong>
    {progress != null ? <div className="risk-center-progress"><span style={{ width: `${Math.max(0, Math.min(100, progress))}%` }} /></div> : null}
    <small>{detail}</small>
  </article>;
}

function PanelHeading({ eyebrow, title, action }: { eyebrow: string; title: string; action?: ReactNode }) {
  return <div className="risk-center-panel-heading"><div><span>{eyebrow}</span><h2>{title}</h2></div>{action}</div>;
}

export default function RiskPage() {
  const overviewQ = useQuery({ queryKey: ["risk", "overview"], queryFn: () => getRepositories().risk.getOverview(), refetchInterval: 15000 });
  const decisionsQ = useQuery({ queryKey: ["risk", "decisions"], queryFn: () => getRepositories().risk.listDecisions(), refetchInterval: 15000 });
  const tradingAccountsQ = useQuery({ queryKey: ["accounts"], queryFn: () => getRepositories().accounts.listAccounts(), refetchInterval: 30000 });
  const portfolioQ = useQuery({ queryKey: ["portfolio"], queryFn: () => getRepositories().portfolio.getSnapshot(), refetchInterval: 15000 });
  const positionsQ = useQuery({ queryKey: ["positions"], queryFn: () => getRepositories().positions.listPositions(), refetchInterval: 15000 });
  const journalAccountsQ = useQuery({ queryKey: ["journal", "accounts"], queryFn: journalApi.listAccounts, refetchInterval: 30000 });

  const policyQueries = useQueries({
    queries: (journalAccountsQ.data ?? []).filter(account => !account.archived).map(account => ({
      queryKey: ["journal", "risk-policy", account.id],
      queryFn: () => journalApi.getRiskPolicy(account.id),
      staleTime: 15000,
      refetchInterval: 30000,
    })),
  });

  const decisions = decisionsQ.data ?? [];
  const positions = positionsQ.data ?? [];
  const journalAccounts = (journalAccountsQ.data ?? []).filter(account => !account.archived);
  const tradingAccounts = tradingAccountsQ.data ?? [];
  const policies = useMemo(() => journalAccounts.map((account, index) => ({
    account,
    envelope: policyQueries[index]?.data as JournalRiskPolicyEnvelope | undefined,
    loading: policyQueries[index]?.isLoading ?? false,
    error: policyQueries[index]?.isError ?? false,
  })), [journalAccounts, policyQueries]);
  const configuredPolicies = policies.filter(item => (item.envelope?.version ?? 0) > 0);
  const needsReview = policies.filter(item => item.envelope && item.envelope.version === 0);
  const overview = overviewQ.data;
  const portfolio = portfolioQ.data;
  const engineTone = overview?.tone === "HEALTHY" ? "good" : overview?.tone === "BREACH" ? "danger" : "warning";
  const dailyUtilization = overview?.dailyLossLimitPct && overview.dailyLossLimitPct > 0
    ? (overview.dailyLossPct / overview.dailyLossLimitPct) * 100 : null;
  const drawdownUtilization = overview?.maxDrawdownLimitPct && overview.maxDrawdownLimitPct > 0
    ? (overview.maxDrawdownPct / overview.maxDrawdownLimitPct) * 100 : null;
  const openPositionsPnl = positions.reduce((sum, position) => sum + amount(position.unrealizedPnl), 0);
  const linkedAccounts = journalAccounts.filter(account => account.linked_trading_account_id).length;

  const decisionColumns = useMemo<ColumnDef<RiskDecisionRecord>[]>(() => [
    { accessorKey: "time", header: "Time", cell: ({ getValue }) => <span className="num">{new Date(String(getValue())).toLocaleString()}</span> },
    { accessorKey: "strategyId", header: "Strategy" },
    { accessorKey: "instrumentId", header: "Instrument" },
    { accessorKey: "accountId", header: "Account", cell: ({ getValue }) => <span className="num">{String(getValue()).slice(0, 12)}</span> },
    { accessorKey: "decision", header: "Decision", cell: ({ row }) => <StatusBadge tone={riskDecisionTone(row.original.decision)}>{row.original.decision}</StatusBadge> },
    { accessorKey: "reason", header: "Assessment" },
  ], []);

  if (overviewQ.isLoading && journalAccountsQ.isLoading && tradingAccountsQ.isLoading) {
    return <div className="loading">Loading risk intelligence…</div>;
  }
  if (overviewQ.isError && journalAccountsQ.isError && tradingAccountsQ.isError) {
    return <div className="error" role="alert">Risk data is unavailable. Check the Trading and Journal API connections.</div>;
  }

  return <main className="risk-center-page">
    <PageHeader title="Risk Center" description="A consolidated view of execution controls, account-level risk policies, portfolio exposure and recent risk decisions." />
    <div className="risk-center-toolbar">
      <div className="risk-center-live"><span className={!overview || overview.killSwitch ? "is-danger" : "is-live"} />{!overview ? "RISK DATA UNAVAILABLE" : overview.killSwitch ? "KILL SWITCH ACTIVE" : "RISK DATA LIVE"}<small>Auto-refresh · 15 seconds</small></div>
      <div className="risk-center-toolbar-actions"><Link className="btn" href="/risk/policies">Manage account policies ↗</Link><Link className="btn" href="/accounts">View accounts</Link></div>
    </div>

    <section className="risk-center-metrics-grid" aria-label="Risk overview">
      <MetricCard label="RISK ENGINE" value={overview?.engineStatus ?? "Unavailable"} detail={overview?.killSwitch ? "Trading halt is active" : "Execution service state"} tone={engineTone} />
      <MetricCard label="DAILY LOSS" value={pct(overview?.dailyLossPct)} detail={overview?.dailyLossLimitPct == null ? "No execution limit reported" : `Limit ${limit(overview.dailyLossLimitPct)}`} tone={dailyUtilization != null && dailyUtilization >= 80 ? "danger" : "neutral"} progress={dailyUtilization} />
      <MetricCard label="MAX DRAWDOWN" value={pct(overview?.maxDrawdownPct)} detail={overview?.maxDrawdownLimitPct == null ? "No execution limit reported" : `Limit ${limit(overview.maxDrawdownLimitPct)}`} tone={drawdownUtilization != null && drawdownUtilization >= 80 ? "warning" : "neutral"} progress={drawdownUtilization} />
      <MetricCard label="GROSS EXPOSURE" value={portfolio ? formatMoney(portfolio.grossExposure) : "—"} detail={portfolio ? `${fixed(portfolio.exposurePct, 1, "%")} of equity · ${positions.length} open positions` : "Portfolio exposure unavailable"} />
    </section>

    {overview?.killSwitch ? <div className="risk-center-alert danger" role="alert"><strong>Trading kill switch is active.</strong><span>New execution may be blocked by the risk engine. Review the engine and account controls before resuming.</span></div> : null}
    {overview?.engineStatus === "NOT_CONFIGURED" ? <div className="risk-center-alert warning"><strong>Execution risk limits are not configured.</strong><span>The trading engine currently reports no active daily-loss or drawdown limits. Account policies below are stored separately and are not automatically enforced by the execution engine.</span><Link href="/risk/policies">Review account policies ↗</Link></div> : null}

    <div className="risk-center-main-grid">
      <section className="panel risk-center-panel">
        <PanelHeading eyebrow="ACCOUNT GOVERNANCE" title="Risk policy hierarchy" action={<Link href="/risk/policies">Configure policies ↗</Link>} />
        <p className="risk-center-intro">Firm and personal limits are stored per Journal account. Linked execution accounts are shown where a trading-account connection exists.</p>
        {journalAccountsQ.isLoading ? <div className="loading">Loading Journal accounts…</div> : journalAccountsQ.isError ? <div className="error">Journal accounts could not be loaded.</div> : journalAccounts.length === 0 ? <div className="risk-center-empty"><strong>No Journal accounts found</strong><span>Create a Journal account to configure and monitor account-specific risk limits.</span><Link className="btn primary" href="/journal">Open Journal</Link></div> : (
          <div className="risk-policy-list">
            {policies.map(({ account, envelope, loading, error }) => {
              const policy = envelope?.policy;
              const tradingAccount = tradingAccounts.find(item => item.id === account.linked_trading_account_id);
              const configured = (envelope?.version ?? 0) > 0;
              return <article className="risk-policy-row" key={account.id}>
                <div className="risk-policy-identity"><span className="risk-policy-avatar">{String(account.name ?? "?").slice(0, 1).toUpperCase()}</span><div><strong>{account.name || "Unnamed account"}</strong><small>{account.broker || "Journal"} · {account.account_type} · {account.currency}</small></div></div>
                <div className="risk-policy-limits">
                  <div><small>PER TRADE</small><strong>{policy ? `${policy.risk_per_trade_pct}%` : "—"}</strong></div>
                  <div><small>DAILY LOSS</small><strong>{policy ? limit(policy.max_daily_loss_pct) : "—"}</strong></div>
                  <div><small>MAX DRAWDOWN</small><strong>{policy ? limit(policy.max_total_drawdown_pct) : "—"}</strong></div>
                  <div><small>MIN. R:R</small><strong>{policy?.minimum_rr == null ? "—" : `${fixed(policy.minimum_rr, 2, "R")}`}</strong></div>
                </div>
                <div className="risk-policy-state">{loading ? <StatusBadge tone="warn">Loading</StatusBadge> : error ? <StatusBadge tone="bad">Unavailable</StatusBadge> : configured ? <StatusBadge tone="ok">Saved · v{envelope?.version}</StatusBadge> : <StatusBadge tone="warn">Needs setup</StatusBadge>}<small>{tradingAccount ? `Linked · ${tradingAccount.name}` : account.linked_trading_account_id ? "Trading link unavailable" : "Journal only"}</small></div>
                <Link className="risk-policy-open" href="/risk/policies" aria-label={`Edit risk policy for ${account.name}`}>↗</Link>
              </article>;
            })}
          </div>
        )}
        <div className="risk-center-footnote">Policy configuration is advisory until explicitly integrated with and enforced by the execution risk engine. A saved Journal policy is not represented as an active trading-engine limit.</div>
      </section>

      <aside className="risk-center-side">
        <section className="panel risk-center-panel">
          <PanelHeading eyebrow="CONTROL STATUS" title="Risk readiness" />
          <div className="risk-readiness-row"><span>Execution engine</span><StatusBadge tone={overview ? riskToneClass(overview.tone) : "warn"}>{overview?.engineStatus ?? "Unavailable"}</StatusBadge></div>
          <div className="risk-readiness-row"><span>Kill switch</span><StatusBadge tone={overview?.killSwitch ? "bad" : "ok"}>{overview?.killSwitch ? "Active" : "Off"}</StatusBadge></div>
          <div className="risk-readiness-row"><span>Account policies saved</span><strong>{configuredPolicies.length} / {journalAccounts.length}</strong></div>
          <div className="risk-readiness-row"><span>Policies needing setup</span><strong>{needsReview.length}</strong></div>
          <div className="risk-readiness-row"><span>Trading accounts linked</span><strong>{linkedAccounts} / {journalAccounts.length}</strong></div>
          <div className="risk-readiness-row"><span>Open positions</span><strong>{positions.length}</strong></div>
        </section>
        <section className="panel risk-center-panel">
          <PanelHeading eyebrow="LIVE PORTFOLIO" title="Open position risk" />
          <div className="risk-position-total"><strong className={openPositionsPnl >= 0 ? "pos" : "neg"}>{formatSignedMoney(openPositionsPnl)}</strong><small>Unrealized P&amp;L · {positions.length} positions</small></div>
          <div className="risk-readiness-row"><span>Gross exposure</span><strong>{portfolio ? formatMoney(portfolio.grossExposure) : "—"}</strong></div>
          <div className="risk-readiness-row"><span>Net exposure</span><strong>{portfolio ? formatMoney(portfolio.netExposure) : "—"}</strong></div>
          <div className="risk-readiness-row"><span>Portfolio equity</span><strong>{portfolio ? formatMoney(portfolio.totalEquity) : "—"}</strong></div>
          <small className="risk-center-footnote">Position stop-risk is not supplied by the current trading positions API; exposure is not a substitute for stop-loss risk.</small>
        </section>
      </aside>
    </div>

    <section className="panel risk-center-panel risk-center-table-panel">
      <PanelHeading eyebrow="ACTIVE MARKET RISK" title="Open positions" action={<Link href="/positions">View positions ↗</Link>} />
      {positionsQ.isLoading ? <div className="loading">Loading positions…</div> : positionsQ.isError ? <div className="error">Open positions are unavailable.</div> : positions.length === 0 ? <div className="risk-center-empty compact"><strong>No open positions</strong><span>Open exposure will appear here when positions are available.</span></div> : (
        <div className="risk-center-position-list">
          {positions.slice(0, 8).map((position: Position) => <div className="risk-center-position-row" key={position.id}>
            <div><strong>{position.instrumentId}</strong><small>{position.side} · {String(position.accountId ?? "Unknown").slice(0, 12)}</small></div>
            <div><small>QUANTITY</small><strong>{formatQuantity(position.quantity)}</strong></div>
            <div><small>ENTRY / MARK</small><strong>{position.entryPrice} / {position.markPrice}</strong></div>
            <div><small>EXPOSURE</small><strong>{formatMoney(position.exposure)}</strong></div>
            <div><small>UNREALIZED P&amp;L</small><strong className={amount(position.unrealizedPnl) >= 0 ? "pos" : "neg"}>{formatSignedMoney(position.unrealizedPnl)}</strong></div>
            <StatusBadge tone={position.riskPct == null ? "warn" : "info"}>{position.riskPct == null ? "Stop risk N/A" : `${fixed(position.riskPct, 2, "%")} risk`}</StatusBadge>
          </div>)}
        </div>
      )}
    </section>

    <section className="panel risk-center-panel risk-center-table-panel">
      <PanelHeading eyebrow="AUDIT TRAIL" title="Recent risk decisions" action={<Link href="/orders">Review order lifecycle ↗</Link>} />
      {decisionsQ.isLoading ? <div className="loading">Loading risk decisions…</div> : decisionsQ.isError ? <div className="error">Risk decisions are unavailable.</div> : <DataTable data={decisions.slice(0, 10)} columns={decisionColumns} emptyMessage="No execution risk decisions have been recorded yet." />}
    </section>
    <div className="risk-center-data-note">Data sources: Trading API (risk engine, portfolio, positions and execution decisions) · Journal API (account risk policies). Values are shown only where the connected services provide them; unavailable stop-risk and enforcement status are not inferred.</div>
  </main>;
}
