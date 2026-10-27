# Quant OS — Task Queue
Last updated: 2026-10-27. Follow `RULES.md`. One task at a time; only mark DONE with evidence.

## Status definitions
READY = scoped; IN PROGRESS = active; BLOCKED = prerequisite missing; DONE = acceptance passed; DEFERRED = intentionally postponed.

## Milestone 0 — Baseline
- [x] **QOS-001 [DONE]** Audit source, branch/commit, workspace, tests, venue status, frontend status.
  - Evidence: [`BASELINE_AUDIT.md`](BASELINE_AUDIT.md), audited `main` at `14619a758ac9864edb52958195de57fb5ca4b685`; 13 Cargo workspace members, checked-in test inventory, simulated/prop-paper vs live-not-implemented status, frontend mock/API fallback and synthetic events recorded. Source audit only; executable checks remain QOS-002.
- [x] **QOS-002 [DONE]** Run `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and applicable Clippy.
  - Verified on GitHub Actions run 40425656667 (latest PR head): formatting, workspace check, workspace tests, and `cargo clippy --workspace --all-targets` passed. Frontend `npm ci`, lint, typecheck, and build also passed in the same run.
  - The initial formatting failure is retained in REV-004; successful rerun is recorded in REV-005. This is CI verification, not proof of production or live trading readiness.
- [x] **QOS-003 [DONE]** Inspect frontend scripts/dependencies/tokens before UI changes.
  - Evidence: `FRONTEND_AUDIT.md`; source inspection covered scripts/dependencies, environment/data modes, theme tokens, mock/synthetic data, and operational status presentation. CI run 40425954540 passed frontend lint, typecheck, and production build. No UI or runtime behavior changed.

## Next recommended task
- **QOS-011 [IMPLEMENTED; PR OPEN]** Adapter idempotency and cancel-confirmation implementation is CI-verified. Latest PR head `8b413f5127e906650f8621b84543b5bb4ee0741d` passed Rust format/check/test/Clippy and frontend lint/typecheck/build in run 40434342385. Event-stream deduplication remains limited by the current trait having no event-stream surface.

## Milestone 1 — Product B
- [x] **QOS-010 [DONE]** Specify the existing `VenueAdapter` contract and gaps. Evidence: `VENUE_ADAPTER_CONTRACT.md` documents capabilities, health/readiness, instrument constraints, async events, idempotency, ambiguity, cancel confirmation, reconnect/reconciliation, and 18 test scenarios. Spec-only; no runtime behavior changed.
- [x] **QOS-011 [IMPLEMENTED; PR OPEN]** Enforce same-client-ID/same-payload idempotency and reject changed payloads in simulated and prop venues; close concurrent-submit race; require authoritative terminal cancellation confirmation in OMS bridge. Tests cover simulator replay/conflict and cancel-ack-with-open-order; existing ambiguous-submit test retained. Latest-head run 40434342385 passed Rust format/check/test/Clippy and frontend lint/typecheck/build. See REV-010–REV-013. Event-stream deduplication is not implemented because no event stream is exposed today; PR #7 remains open/unmerged for review.
- [x] **QOS-012 [DONE]** Specify per-account sizing inputs/outputs and loss models. Evidence: `INSTRUMENT_AWARE_SIZING.md` covers account-currency risk budget, tick-value and linear models, FX conversion, costs, exposure/margin/pending-order reserves, volume-step rounding, fail-closed behavior, and 26 acceptance scenarios. Specification only; no risk code changed.
- [ ] **QOS-013 [READY]** Implement allocator behind risk authorization using the merged QOS-012 sizing specification. Test boundary values, rounding, min/max volume, stale state, drawdown, and independent account decisions.
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
