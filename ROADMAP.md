# Quant OS Product Roadmap
Last updated: 2026-10-27. Planned roadmap only. `RULES.md` is authoritative.

## Phase 0 — Verify baseline
Audit branch/commit, source, workspace, tests, current venue capabilities, frontend, and deployment. Run format/check/test/clippy. **Exit:** reproducible baseline and capability matrix.

## Phase 1 — EdgeNest MT5 Connectivity Alpha
Prove EdgeNest-owned managed MT5 connectivity using two distinct broker demo environments. Keep the existing Rust risk/OMS as the authority. Compare official Python-terminal integration against an EdgeNest-managed EA bridge; do not build both paths fully in parallel. Implement account/terminal/server identity binding, read-only account/order/position snapshots, instrument metadata and fresh quotes, secure transport, health monitoring, and durable reconciliation. **Exit:** real Windows host conformance, worker restart/reconnect recovery, protected credential/certificate lifecycle review, broker-specific mapping evidence, and independently reviewed demo evidence. Order execution remains disabled until the existing QOS-013/014/015 release gates pass.

## Phase 2 — Product B: Smart Multi-Account Execution
Per-account eligibility, risk-budget/stop-aware sizing, independent order IDs/decisions/outcomes, OMS routing, audit, reconciliation, simulator and failure injection. Fan-out remains non-atomic. **Exit:** no risk bypass; duplicate and ambiguous-submit tests pass; partial success is visible; demo orders pass the controlled execution gate.

## Phase 3 — Developer API Beta
Expose account, order, position, risk, event and reconciliation operations through a versioned, tenant-scoped REST API and WebSocket/SSE streams. Add scoped API keys, quotas/rate limits, idempotency, stable errors, demo sandbox, API docs and SDK examples. Build the dashboard as a first-party API client. **Exit:** API contract tests, stream recovery tests, tenant isolation/security tests, and end-to-end demo tests pass.

## Phase 4 — Hosted Gateway Platform
Automate Windows host and terminal provisioning, per-account terminal lifecycle, supervised restart/reconnect, capacity management, secret/certificate rotation, monitoring, alerting and deployment rollback. Define per-account capacity and isolation tests before scale-out.

## Phase 5 — First-party Trader Dashboard
Use the shared public service contracts; no frontend-only execution path. Connect account onboarding, one-click parent requests, account-specific risk preview, independent child outcomes, and order/position reconciliation.

## Phase 6 — Product A: Risk Co-Pilot
Configurable monitoring, stale/disconnected detection, breach alerts, venue-specific enforcement capabilities, and confirmed cancellation/close workflows. **Exit:** detection and enforcement tested separately; failures visible; no unsupported guarantees.

## Phase 7 — Product C: Hosted Quant Strategies
Versioned registry, isolation, resource limits, per-strategy risk budgets, reproducible performance reports, tenant isolation, and secrets management. **Exit:** repeatable runs and security/operational review.

## Phase 8 — Commercial SaaS hardening
Authentication, authorization, tenant isolation, audit/export, usage metering/billing, support/incident process, privacy/data-retention review, security assessment, legal review, developer docs and versioning policy. This gates broad external launch.

## Phase 3 — Product A: Risk Co-Pilot
Configurable monitoring, stale/disconnected detection, breach alerts, venue-specific enforcement capabilities, and confirmed cancellation/close workflows. **Exit:** detection and enforcement tested separately; failures visible; no unsupported guarantees.

## Phase 4 — Product C: Hosted Quant Strategies
Versioned registry, isolation, resource limits, per-strategy risk budgets, reproducible performance reports, tenant isolation, and secrets management. **Exit:** repeatable runs and security/operational review.

## Phase 5 — Commercial SaaS hardening
Authentication, authorization, tenant isolation, audit/export, billing if needed, support/incident process, privacy/data-retention review, security assessment, and legal review. This gates broad external launch.

## Phase 6 — EdgeNest Vault Protocol (deferred)
Threat model; custody/funding and chain/venue choice; share/NAV/fee invariants; deposit/withdrawal state machines; smart-contract tests/fuzzing; independent audit; off-chain reconciliation; emergency procedures; legal/regulatory review. **Exit:** invariants verified, audit findings addressed, legal readiness, controlled launch.

## Metrics
Connectivity: supported broker/server coverage, provisioning success, account identity mismatches, terminal uptime, reconnect recovery time and quote freshness. Execution reliability: duplicate orders, unknown-state duration, reconciliation breaks, adapter uptime/latency and per-account partial outcomes. Risk: blocked unsafe intents, breach detection, false positives, action confirmation. Developer platform: API availability/latency, error rate, stream recovery, active developers, integration time, usage and cost per connected account. Product: trader activation, retention and support load. Strategy: forward sample, net results after fees/slippage, drawdown and execution quality. Commercial: conversion, gross margin and churn. Do not optimize gross returns or raw order count alone.

## Dependency rule
Shared infrastructure is reusable, but execution remains venue-aware. Vault contracts are not required to validate SaaS products and cannot replace off-chain reconciliation.
