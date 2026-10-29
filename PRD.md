# Quant OS — Product Requirements Document
Last updated: 2026-10-27. `RULES.md` is authoritative. This document describes intended behavior, not proof of implementation.

## Vision
EdgeNest is a trading infrastructure platform with two first-party product surfaces sharing one execution foundation:
1. **Trader Dashboard:** a user-facing control plane for connecting eligible accounts, configuring risk, submitting a single trade request, and monitoring independent account outcomes.
2. **Developer Platform:** a multi-tenant API and connectivity service that lets external developers build trading applications, bots, analytics, and execution workflows without using the EdgeNest frontend or operating MT5 terminals themselves.

The shared platform owns account connectivity, tenant/security boundaries, risk authorization, order management, venue adapters, streaming events, persistence, and reconciliation. The frontend is one client of the platform, not the source of financial truth. This is the target product direction, not a claim that the developer API or hosted gateway is already implemented.

## Product sequence
1. **EdgeNest MT5 Connectivity Alpha:** prove managed MT5 connectivity across two distinct broker demo environments, with account-bound snapshots, secure gateway transport, and read-only reconciliation before enabling order submission.
2. **Product B — Smart Multi-Account Risk Copier:** expose one parent trade request that is independently authorized and sized per eligible account, with account-scoped order identities and non-atomic, independently reconciled outcomes.
3. **Developer API Beta:** provide tenant-scoped account provisioning, REST operations, real-time event streams, idempotency, rate limits, a demo sandbox, documentation and initial SDK examples on the same core used by the dashboard.
4. **Trader Dashboard:** build the first-party UI as a client of the same backend contracts and execution services.
5. **Product A — Risk Co-Pilot:** monitor account risk, alert on breaches, and request only venue-supported enforcement actions.
6. **Product C — Hosted Quant Strategies:** isolated, versioned strategies emitting `TradeIntent` only, with strategy-level risk budgets and forward-performance reporting.
7. **EdgeNest Vault Protocol (later):** on-chain deposits/withdrawals, share/NAV accounting, fees, access controls, audits, and reconciliation.

## Users
- Traders managing multiple eligible broker/platform accounts through the EdgeNest dashboard.
- External developers and SaaS teams using EdgeNest APIs to connect accounts and build their own trading experiences.
- Quantitative researchers deploying strategies through controlled runtime interfaces.
- Later, users of supported vault products subject to legal and technical readiness.

## Platform product requirements
- Treat the developer API as a first-class product, not merely an internal frontend backend.
- The trader dashboard and external clients must use shared, versioned service contracts and the same account, risk, OMS, adapter, event, and reconciliation services.
- Provide tenant-scoped authentication/authorization, scoped API credentials, rate limits, usage metering, auditability, and isolation before external developer access.
- Provide a demo/sandbox mode that is explicitly separate from paper and live modes.
- Public API and SDKs must never expose MT5 passwords, bridge certificates, private keys, or other provider credentials.
- Account connectivity should be provider-independent at the product boundary. MT5 is the first target; future platform adapters must not require rewriting the public API or risk engine.
- A connected account is not automatically eligible for automated trading or copying. Enforce account/program compatibility and permission checks before execution.
- Hosted terminal provisioning, health monitoring, restart/reconnect and terminal/account binding are EdgeNest infrastructure responsibilities in the target design; these capabilities remain unverified until tested on real broker demo environments.

## Product B MVP
- Register accounts and venue capabilities.
- Accept validated master intents; no strategy-to-venue bypass.
- Check each account independently for status, fresh equity/margin, policy, instrument eligibility, drawdown, existing exposure, and venue constraints.
- Size from remaining risk budget, stop distance, contract/tick value, conversion assumptions, and valid volume increments. Equity ratios alone are insufficient.
- Route only approved requests through risk → OMS → adapter.
- Track independent accepted/rejected/unknown/partial-fill outcomes and reconciliation per account.
- Preserve a trace from intent to policy version, sizing inputs, decision, order, fills, and final state.

## Product A — Risk Co-Pilot
Configurable risk monitoring, stale/disconnected state detection, alerts, and a per-venue capability matrix for blocking new orders, cancellation, or close requests. A request is not success until confirmed.

## Product C — Hosted Quant Strategies
Strategy registry/versioning, process isolation, resource limits, per-strategy risk budgets, secrets separation, and reproducible performance reports including fees, slippage, rejected trades, and drawdown.

## EdgeNest vaults
Deferred until off-chain reliability is evidenced. Define custody/funding, share/NAV/fee invariants, deposit/withdrawal state machines, chain and venue scope, emergency pause, independent contract audit, and legal/regulatory review. On-chain assets are not automatically traditional broker margin.

## Success criteria
- No order without traceable executable risk authorization.
- Duplicate and ambiguous-submit tests preserve idempotency and `Unknown` state.
- Reconciliation breaks are visible, never silently overwritten.
- Strategy performance claims disclose period, sample size, fees, slippage, and drawdown.
- User onboarding, retention, support load, and action-confirmation time are measured.

## Release gates
Unit/invariant tests → simulator integration → failure injection → paper forward test → explicitly approved monitored live pilot. SaaS adds security, privacy, tenancy, support, and legal gates. Vaults add independent audit and legal review.

## Open decisions
First live venue; MT5 transport/topology; cTrader eligibility/auth; instrument scope; tenancy/auth/billing; enforcement capability by venue; vault chain/custody/jurisdiction.

Before editing, check `RULES.md`; log corrections and rule failures in `REVIEWS.md`.
