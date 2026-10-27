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
- **QOS-014 [READY]** Implement the MT5 bridge v1 contract and boundary validation. QOS-013's runtime sizing, timestamped snapshot cache, pending-order/margin controls, persistent sizing audit, and fail-closed capability/account-semantics gates are implemented and CI-verified on PR #13 run 40445467228. Live venue integration remains disabled; see `QOS-013_IMPLEMENTATION_PLAN.md`.
- **QOS-011 [DONE — merged]** PR #7 merged on 2026-10-27 at merge commit `324d327f035ca5ccb573404cc8e446b816990ada`. The PR adds same-client-ID/same-payload replay semantics, typed conflicts, concurrent duplicate protection in paper connectors, and confirmed cancellation checks. Event-stream deduplication remains limited by the current trait having no event-stream surface.

## Milestone 1 — Product B
- [x] **QOS-010 [DONE]** Specify the existing `VenueAdapter` contract and gaps. Evidence: `VENUE_ADAPTER_CONTRACT.md` documents capabilities, health/readiness, instrument constraints, async events, idempotency, ambiguity, cancel confirmation, reconnect/reconciliation, and 18 test scenarios. Spec-only; no runtime behavior changed.
- [x] **QOS-011 [DONE — merged]** Enforce same-client-ID/same-payload idempotency and reject changed payloads in simulated and prop venues; close concurrent-submit race; require authoritative terminal cancellation confirmation in OMS bridge. Tests cover simulator replay/conflict and cancel-ack-with-open-order; existing ambiguous-submit test retained. PR #7 merged 2026-10-27 at `324d327f035ca5ccb573404cc8e446b816990ada`. See REV-009–REV-015 for implementation, CI history, and merge-status correction. Event-stream deduplication is not implemented because no event stream is exposed today.
- [x] **QOS-012 [DONE]** Specify per-account sizing inputs/outputs and loss models. Evidence: `INSTRUMENT_AWARE_SIZING.md` covers account-currency risk budget, tick-value and linear models, FX conversion, costs, exposure/margin/pending-order reserves, volume-step rounding, fail-closed behavior, and 26 acceptance scenarios. Specification only; no risk code changed.
- [x] **QOS-013 [DONE — venue-neutral runtime sizing core; live release remains gated]** Implemented explicit linear/tick-value sizing, timestamped runtime snapshot cache, source/conversion-path validation, account/instrument identity and account-state freshness checks, fail-closed venue-capability and netting/hedging-semantics gates, pending-order exposure/margin reserves, volume-step and min/max/min-notional controls, final modeled-loss checks, and append-only structured sizing audit with exact decimal strings plus the complete policy snapshot. Instrument-aware execution requires persistent audit storage; missing storage blocks new exposure. CI run 40445467228 passed Rust format/check/test/Clippy and frontend lint/typecheck/build. QOS-014/015 must provide real venue/account/quote/instrument data before any live use; no live mode is enabled.
- [ ] **QOS-014 [READY]** Specify MT5 bridge v1: schema version, IDs, decimal strings, units, heartbeat, reconnect, idempotency, ambiguous submit.
- [ ] **QOS-015 [READY — paper/sim first; live access blocked]** Implement the first adapter behind the bridge contract using paper/simulated transport. A live MT5 connection remains blocked until broker/account access and deployment environment are explicitly selected.
- [ ] **QOS-013 [IN PROGRESS — merged implementation; acceptance gaps remain]** PR #12 was merged by the user. Merged code includes timestamped snapshot cache/provider boundary, pending-order exposure and margin reserves, detailed sizing outcomes, append-only sizing audit persistence, and focused sizing tests. Still open: concrete authoritative provider wired to real account/quote/instrument/conversion sources, explicit policy requiring margin inputs, remaining SZ-01–SZ-26/property coverage, and verification against the exact merged main head. Do not mark DONE until these gaps are resolved or explicitly accepted by review.
- [ ] **QOS-014 [IN PROGRESS — protocol boundary implemented; transport integration pending]** Added versioned Rust message models and validation, JSON schema, protocol doc, account/message/correlation IDs, timestamps, decimal-string validation, heartbeat/capabilities, accepted/rejected/unknown submit outcomes, cancellation-vs-terminal distinction, snapshots and reconciliation request. Added a bounded incremental NDJSON decoder (64 KiB per line; split chunks and multiple frames supported; CRLF accepted; blank, invalid UTF-8, malformed, oversized and incomplete-EOF frames fail closed) plus a canonical submit fixture. Focused tests cover these framing cases and existing protocol invariants. Still open: authenticated handshake/connection binding, durable idempotency and reconnect/sequence-gap reconciliation integration, executable cross-language conformance against an MT5-side implementation, and latest-head full CI.
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
