# Quant OS / EdgeNest — Phases and Delivery Roadmap

Last updated: 2026-11-10. This document describes the target delivery sequence and current gate. Verified implementation status belongs in `PROGRESS.md`; acceptance rules in `RULES.md`.

## North-star architecture

Build EdgeNest as an EdgeNest-operated trading infrastructure platform. Customers should not need to install MT5, maintain their own VPS, or attach an EA to their own chart. EdgeNest owns the managed terminal/gateway fleet and exposes secure product interfaces.

Separate the platform into two planes:

- **Control plane (Quant OS):** tenant/account identity, policy, risk authorization, portfolio limits, audit and operator controls.
- **Execution plane (EdgeNest gateway):** managed MT5 terminals, broker adapters, order transport, acknowledgements, fills, broker-state reconciliation and recovery.

The broker remains the source of truth for accepted orders, fills, positions and account state. The internal ledger must reconcile to broker state. All exposure-changing requests must pass through shared risk authorization, OMS/idempotency, venue adapter and reconciliation. No product may bypass this shared path.

## Current position — 2026-11-10

<badge>QOS-015C IN PROGRESS</badge>

**We are in Phase 1, at QOS-015C: runtime and deployment reconciliation. Deployment is NO-GO; execution is disabled.**

Already achieved:
- Substantial Rust risk/OMS, typed MT5 bridge protocol, read-only daemon, Python MT5 provider, mTLS, identity binding, idempotency/checkpoints, session readiness and reconciliation foundations.
- QOS-013 instrument-aware sizing and fail-closed freshness/risk checks are implemented substantially in code; real broker-input acceptance remains open.
- QOS-015C read-only collector and reconciliation runbook are in the repository; the collector was run against both local checkouts from pinned main commit `17142139e1f0985f075db6f703279bd7db373c60`.
- Staging PostgreSQL identity/role/ownership checks and all six applied migration checksum comparisons passed. Staging PostgreSQL is healthy and its external volume is attached.
- CI for the QOS-015C collector implementation passed its five jobs.

Still open:
- Review and reconcile the latest primary and secondary inventories; preserve all dirty/untracked work.
- Pin a single reviewed immutable source revision and attest the worker, daemon, provider, EA source and EA binary against it.
- Resolve staging Compose/secret-source provenance, process identity/health, backup/restore evidence and independent sign-off.
- Demonstrate actual managed-terminal read-only TenTrade demo conformance, fresh data persistence and recovery.
- Certificate/private-key lifecycle and security review.

The current primary checkout has three dirty paths that must be preserved. Its inventory reports LaunchAgent worker/daemon last exit status 1 and no listeners on expected bridge/provider ports 19443/18765. A healthy PostgreSQL container does not prove the bridge runtime is healthy. See `PROGRESS.md` for exact current evidence and limitations.

## Phase 0 — Repository and engineering baseline

**Status: COMPLETE for the recorded repository baseline and CI foundation.**

Audit source, architecture, tests, venue capabilities, frontend and release rules. Establish reproducible format/check/test/Clippy CI and a documented baseline. This phase is not deployment acceptance and does not imply live trading capability.

## Phase 1 — MT5 read-only connectivity acceptance

**Status: IN PROGRESS — currently at QOS-015C. No order execution.**

The goal is to prove an EdgeNest-managed terminal can connect to a real broker demo account, identify it correctly, stream fresh market data, persist account/order/position reconciliation and recover safely. This is a read-only milestone.

### QOS-013 — Instrument-aware risk sizing

**Status: Core implementation substantially present; real broker acceptance outstanding.**

Validate account-currency conversion, tick/contract values, spreads and slippage, margin and existing exposure, pending orders, broker volume constraints, final modeled loss, persistent audit and fail-closed behavior using actual broker inputs.

### QOS-014 — Bridge protocol, daemon and read-only runtime

**Status: Protocol/runtime foundations implemented; real terminal conformance and operational recovery outstanding.**

Validate cross-language protocol behavior, account/terminal identity binding, authenticated transport, heartbeat/readiness, durable sequence/idempotency, market-data and reconciliation persistence, and broker-state recovery. The current daemon remains read-only.

### QOS-015A/B — Provider and market-data freshness

**Status: Substantial implementation and contract tests; target-runtime acceptance outstanding.**

The official MT5 Python integration on managed Windows is the primary production path. The macOS EA/socket provider is a development/conformance path, not proof of production deployment. Preserve strict quote freshness and explicit fresh/stale/unavailable/invalid states; never weaken freshness rules to force a pass.

### QOS-015C — Runtime and deployment reconciliation

**Status: IN PROGRESS — current task. Deployment NO-GO.**

- Reconcile both local checkouts and preserve dirty/untracked changes.
- Identify authoritative source tree and pin exact immutable source commit.
- Verify worker/daemon/provider/EA source and binary hashes against one reviewed manifest.
- Verify staging Compose configuration, secret mounts/source provenance, PostgreSQL identity and volume.
- Verify active process/executable identity, port isolation and v1/v2 separation.
- Complete backup and isolated restore test.
- Produce redacted evidence digest, independent review and operator sign-off.

**Exit:** every required gate is PASS or explicitly dispositioned by an authorized reviewer; no unknown is treated as pass; deployment is separately approved. Do not run migrations or restart/replace services during reconciliation.

### QOS-015D — EdgeNest-managed Windows MT5 terminal

**Status: NOT STARTED — blocked on QOS-015C acceptance.**

Provision an EdgeNest-controlled Windows host and terminal. Install the supported MT5 terminal and official Python integration; configure the TenTrade demo account; install the Rust daemon and Python worker as supervised services; configure staging connectivity and mTLS; bind the worker to exact terminal, broker server and account identity.

**Exit:** managed host and terminal are provisioned, supervised and reproducibly tied to the approved source/artifact manifest. No customer VPS or customer-attached EA is required.

### QOS-015E — TenTrade demo read-only conformance

**Status: NOT STARTED — blocked on QOS-015D.**

Run the documented Windows smoke test and prove:
- Exact account identity, broker server and account currency.
- Exact broker symbol mapping and instrument specifications.
- Fresh bid/ask ticks with valid timestamps and stale/unavailable classification.
- Historical candle retrieval.
- Authenticated bridge connection and advancing distinct market-data sequences.
- PostgreSQL market-data, account, order and position reconciliation persistence.

**Exit:** timestamped, redacted evidence demonstrates sustained fresh data and broker identity correctness on the managed terminal.

### QOS-015F — Recovery, isolation and security acceptance

**Status: NOT STARTED — blocked on QOS-015E.**

Test worker-only restart, daemon restart, terminal restart, network disconnect/reconnect, database interruption/recovery, sequence continuity, reconciliation after recovery, invalid/expired certificate rejection, certificate rotation/rollback, private-key ACLs and service identities. Include independent security review.

**Phase 1 exit:** the EdgeNest-operated terminal is connected to the TenTrade demo account; fresh EURUSD data and instrument metadata are validated; account/order/position snapshots persist; stale/degraded states fail closed; restarts recover without identity or sequence corruption; credential lifecycle and access controls are independently reviewed. Only then is read-only connectivity accepted.

## Phase 2 — Risk-authorized demo execution

**Status: PLANNED — cannot begin until Phase 1 is accepted and a separate execution scope is approved.**

Keep execution separate from the read-only connectivity daemon. Implement a broker-independent order intent/state model and a controlled execution service:
- Buy/sell, modify, cancel and close requests through a managed terminal.
- Pre-trade risk authorization, account/portfolio limits and kill switch.
- Durable idempotency and explicit order lifecycle states.
- Broker acknowledgements, rejects, partial fills and ambiguous outcomes.
- Persisted execution events and broker/internal-ledger reconciliation.
- Demo-only execution with strict account and instrument allowlists.

**Exit:** deterministic demo tests pass for duplicate requests, timeouts, lost acknowledgements, partial fills, rejects, cancel races, process restarts and broker reconciliation. No live account is enabled by this phase.

## Phase 3 — Multi-account and portfolio execution

**Status: PLANNED.**

Add account eligibility, per-account risk budgets, independent client order IDs and decisions, per-account sizing, non-atomic fan-out, partial-success reporting, portfolio exposure controls and failure isolation.

**Exit:** one account's failure cannot silently contaminate another; aggregate and per-account exposure reconcile; no risk bypass; failure-injection and demo acceptance pass.

## Phase 4 — Developer API and event platform

**Status: PLANNED.**

Expose versioned, tenant-scoped account, order, position, risk, event and reconciliation APIs through REST plus WebSocket/SSE. Add scoped API keys, quotas, idempotency, stable errors, demo sandbox, docs and SDK examples. The Trader Dashboard must consume these same contracts rather than implement a separate execution path.

**Exit:** contract, stream recovery, auth, tenant-isolation and end-to-end demo tests pass.

## Phase 5 — Hosted gateway fleet and operations

**Status: PLANNED; initial single managed terminal is brought up in Phase 1.**

Automate host/terminal provisioning, per-account terminal lifecycle, supervised restart/reconnect, capacity planning, secret/certificate rotation, observability, alerting, deployment rollback, backup/restore and disaster recovery.

**Exit:** repeatable provisioning, tested isolation/capacity boundaries, measured recovery objectives and operational runbooks.

## Phase 6 — First-party Trader Dashboard

**Status: PLANNED.**

Build onboarding, account health, market-data freshness, risk preview, order/position state, audit history, portfolio view and operator workflows on the shared API. No frontend-only execution path or optimistic claim of broker-confirmed actions.

**Exit:** dashboard workflows are tenant-isolated, API-backed and reconciled to broker truth.

## Phase 7 — Product A: Risk Co-Pilot

**Status: PLANNED.**

Add configurable monitoring, stale/disconnected detection, breach alerts, venue-specific enforcement capabilities and confirmed cancellation/close workflows. Separate detection from enforcement and never represent an unconfirmed action as complete.

**Exit:** detection, alerting and enforcement are independently tested; failures and unsupported capabilities remain visible.

## Phase 8 — Product C: Hosted Quant Strategies

**Status: PLANNED.**

Add strategy registry/versioning, tenant isolation, resource limits, secrets management, per-strategy risk budgets and reproducible forward-performance reporting including fees, slippage and drawdown.

**Exit:** repeatable runs, enforced resource/risk boundaries and independent security/operational review.

## Phase 9 — Commercial SaaS hardening

**Status: PLANNED — required before broad external launch.**

Authentication/authorization, tenant isolation, audit/export, usage metering and billing, support and incident processes, privacy/data retention, security assessment, legal review, developer documentation and versioning policy.

**Exit:** security, operational, legal and commercial launch gates are signed off.

## Phase 10 — EdgeNest Vault Protocol

**Status: DEFERRED — not a prerequisite for the trading infrastructure SaaS.**

Complete custody/funding and chain/venue threat model, legal scope, share/NAV/fee/accounting invariants, deposit/withdrawal state machines, smart-contract tests/fuzzing, independent audit, off-chain reconciliation and emergency procedures before any controlled launch.

## Non-negotiable sequencing and safety rules

1. Finish and accept QOS-015C before provisioning or changing the target runtime.
2. Complete QOS-015D, QOS-015E and QOS-015F before Phase 1 is declared complete.
3. Keep the read-only daemon read-only. Execution belongs in a separately reviewed service and milestone.
4. Never enable live order execution from green CI, a socket handshake, MetaEditor compilation or demo success alone.
5. Treat broker state as authoritative for orders, fills, positions and account state; reconcile it to the internal ledger.
6. Preserve all dirty checkouts, staging data, secrets and existing terminal state unless an explicit reviewed change plan authorizes a mutation.
7. Unknown, missing, stale or mismatched evidence is not PASS.
8. Vault work is independent and deferred; it must not delay or weaken the trading platform's operational and risk controls.

## Delivery metrics

- **Connectivity:** supported broker/server coverage, provisioning success, identity mismatches, terminal uptime, reconnect recovery time, quote freshness and persisted snapshot continuity.
- **Execution reliability:** duplicate orders, ambiguous/unknown-state duration, reconciliation breaks, adapter latency/availability and per-account partial outcomes.
- **Risk:** blocked unsafe intents, limit breaches, stale-data blocks, detection false positives and action-confirmation accuracy.
- **Platform:** API availability/latency, stream recovery, tenant-isolation failures, active developers, integration time and cost per connected account.
- **Commercial:** activation, retention, gross margin, support burden and churn.
- **Strategies:** forward sample size, net performance after fees/slippage, drawdown and execution quality. Do not optimize gross returns or raw order count alone.
