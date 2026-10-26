# Quant OS Product Roadmap
Last updated: 2026-10-27. Planned roadmap only. `RULES.md` is authoritative.

## Phase 0 — Verify baseline
Audit branch/commit, source, workspace, tests, current venue capabilities, frontend, and deployment. Run format/check/test/clippy. **Exit:** reproducible baseline and capability matrix.

## Phase 1 — Product B: Smart Multi-Account Risk Copier
Per-account eligibility, risk-budget/stop-aware sizing, independent order IDs/decisions/outcomes, OMS routing, audit, reconciliation, simulator and failure injection. **Exit:** no risk bypass; duplicate and ambiguous-submit tests pass; partial success is visible.

## Phase 2 — First traditional execution integration
Choose one MT5 bridge or cTrader Open API path based on actual access, deployment, permissions, and testability. Do not build both simultaneously by default. **Exit:** adapter contract tests, paper forward run, reconnect/reconciliation evidence, explicit live-pilot approval.

## Phase 3 — Product A: Risk Co-Pilot
Configurable monitoring, stale/disconnected detection, breach alerts, venue-specific enforcement capabilities, and confirmed cancellation/close workflows. **Exit:** detection and enforcement tested separately; failures visible; no unsupported guarantees.

## Phase 4 — Product C: Hosted Quant Strategies
Versioned registry, isolation, resource limits, per-strategy risk budgets, reproducible performance reports, tenant isolation, and secrets management. **Exit:** repeatable runs and security/operational review.

## Phase 5 — Commercial SaaS hardening
Authentication, authorization, tenant isolation, audit/export, billing if needed, support/incident process, privacy/data-retention review, security assessment, and legal review. This gates broad external launch.

## Phase 6 — EdgeNest Vault Protocol (deferred)
Threat model; custody/funding and chain/venue choice; share/NAV/fee invariants; deposit/withdrawal state machines; smart-contract tests/fuzzing; independent audit; off-chain reconciliation; emergency procedures; legal/regulatory review. **Exit:** invariants verified, audit findings addressed, legal readiness, controlled launch.

## Metrics
Reliability: duplicate orders, unknown-state duration, reconciliation breaks, adapter uptime/latency. Risk: blocked unsafe intents, breach detection, false positives, action confirmation. Product: activation, retention, support load. Strategy: forward sample, net results after fees/slippage, drawdown, execution quality. Commercial: conversion and churn. Do not optimize gross returns or raw order count alone.

## Dependency rule
Shared infrastructure is reusable, but execution remains venue-aware. Vault contracts are not required to validate SaaS products and cannot replace off-chain reconciliation.
