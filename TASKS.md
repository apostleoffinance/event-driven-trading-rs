# Quant OS — Task Queue
Last updated: 2026-10-27. Follow `RULES.md`. One task at a time; only mark DONE with evidence.

## Status definitions
READY = scoped; IN PROGRESS = active; BLOCKED = prerequisite missing; DONE = acceptance passed; DEFERRED = intentionally postponed.

## Milestone 0 — Baseline
- [ ] **QOS-001 [READY]** Audit source, branch/commit, workspace, tests, venue status, frontend status.
  - Acceptance: source-backed report distinguishes simulated/paper/live; no capability inferred from docs alone.
- [ ] **QOS-002 [READY]** Run `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and applicable clippy.
  - Acceptance: exact commands/results recorded; failures preserved.
- [ ] **QOS-003 [READY]** Inspect frontend scripts/dependencies/tokens before UI changes.

## Milestone 1 — Product B
- [ ] **QOS-010 [READY]** Extend the existing `VenueAdapter` contract specification. Current trait has account state, submit/cancel, open orders, positions, and get-order; specify gaps for async events, explicit capabilities, instrument specs, health/readiness, ambiguous outcomes, and reconciliation.
- [ ] **QOS-011 [BLOCKED by QOS-001]** Implement/refine adapter contract with simulator tests; duplicate submits and unknown outcomes covered; no risk bypass.
- [ ] **QOS-012 [READY]** Specify per-account sizing inputs/outputs: risk budget, stop distance, contract/tick value, pip/tick value, quote-currency conversions, exposure, limits, lot step/min/max, and venue constraints. Existing sizing has stop-distance caps but is not proven instrument-complete.
- [ ] **QOS-013 [BLOCKED by QOS-001/QOS-012]** Implement allocator behind risk authorization. Test boundary values, rounding, min/max volume, stale state, drawdown, and independent account decisions.
- [ ] **QOS-014 [READY]** Specify MT5 bridge v1: schema version, IDs, decimal strings, units, heartbeat, reconnect, idempotency, ambiguous submit.
- [ ] **QOS-015 [DEFERRED]** Implement first live venue after access/deployment choice; begin with simulator/paper.
- [ ] **QOS-016 [READY]** Add multi-account failure injection: partial success, disconnect, delayed ack, partial fill, duplicate intent, reconciliation break, and cancel request acknowledged while order remains open.
- [ ] **QOS-017 [READY]** Design multi-account orchestration above the current single-account `TradingRuntime::execute_intent` path; preserve independent per-account risk decisions, idempotency keys, and outcomes.

## Milestone 2 — Product A
- [ ] **QOS-020 [DEFERRED]** Document venue-specific monitoring/enforcement capability matrix.
- [ ] **QOS-021 [DEFERRED]** Implement monitoring/alerts with stale/disconnected states.
- [ ] **QOS-022 [DEFERRED]** Implement confirmed-action workflow for supported cancellation/closure.
- [ ] **QOS-023 [DEFERRED]** Test breach detection and enforcement; UI must not claim unconfirmed closure.

## Milestone 3 — Product C
- [ ] **QOS-030 [DEFERRED]** Strategy registry, versioning, isolation, configuration.
- [ ] **QOS-031 [DEFERRED]** Runtime resource limits and secrets isolation.
- [ ] **QOS-032 [DEFERRED]** Reproducible forward-performance reports including fees/slippage/drawdown.

## Milestone 4 — EdgeNest Vault Protocol
- [ ] **QOS-040 [DEFERRED]** Threat model, custody/funding, chain/venue, legal scope.
- [ ] **QOS-041 [DEFERRED]** Vault share/NAV/fee/accounting invariants.
- [ ] **QOS-042 [DEFERRED]** Contracts with invariant/fuzz tests and independent audit.
- [ ] **QOS-043 [DEFERRED]** Explicit off-chain bridge and reconciliation.

## Release gate
- [ ] RULES.md checklist passed
- [ ] Test commands/results recorded
- [ ] PROGRESS.md matches actual state
- [ ] Corrections logged in REVIEWS.md
- [ ] Documentation work has not enabled live money movement
