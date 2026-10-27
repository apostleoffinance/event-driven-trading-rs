# Quant OS — Product Requirements Document
Last updated: 2026-10-27. `RULES.md` is authoritative. This document describes intended behavior, not proof of implementation.

## Vision
Quant OS is a multi-strategy quantitative trading platform separating strategy research/signals from deterministic risk authorization, order management, venue connectivity, and auditable financial state.

## Product sequence
1. **Product B — Smart Multi-Account Risk Copier (first):** fan one master `TradeIntent` to independently evaluated accounts with account-specific sizing and independent order outcomes.
2. **Product A — Risk Co-Pilot:** monitor account risk, alert on breaches, and request only venue-supported enforcement actions.
3. **Product C — Hosted Quant Strategies:** isolated, versioned Python strategies emitting `TradeIntent` only, with strategy-level risk budgets and forward-performance reporting.
4. **EdgeNest Vault Protocol (later):** on-chain deposits/withdrawals, share/NAV accounting, fees, access controls, audits, and reconciliation.

## Users
Retail and prop traders managing multiple accounts; quantitative researchers deploying strategies; later, users of supported vault products subject to legal and technical readiness.

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
