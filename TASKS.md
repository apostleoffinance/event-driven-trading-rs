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

## Current recommended task
- **QOS-014 [IN PROGRESS]** Add durable MT5 order idempotency and reconnect/sequence checkpoints on `feat/qos-014-mt5-idempotency-v1`. Latest scope includes PostgreSQL claim/outcome APIs and fail-closed sequence readiness; transport wiring and end-to-end restart/reconnect tests remain open.
- QOS-014's typed MT5 v1 protocol boundary is merged in PR #14; authenticated transport, durable idempotency, reconnect/reconciliation integration, and MT5-side conformance remain outstanding.
- **QOS-011 [DONE — merged]** PR #7 merged on 2026-10-27 at merge commit `324d327f035ca5ccb573404cc8e446b816990ada`. The PR adds same-client-ID/same-payload replay semantics, typed conflicts, concurrent duplicate protection in paper connectors, and confirmed cancellation checks. Event-stream deduplication remains limited by the current trait having no event-stream surface.

## Milestone 1 — Product B
- [x] **QOS-010 [DONE]** Specify the existing `VenueAdapter` contract and gaps. Evidence: `VENUE_ADAPTER_CONTRACT.md` documents capabilities, health/readiness, instrument constraints, async events, idempotency, ambiguity, cancel confirmation, reconnect/reconciliation, and 18 test scenarios. Spec-only; no runtime behavior changed.
- [x] **QOS-011 [DONE — merged]** Enforce same-client-ID/same-payload idempotency and reject changed payloads in simulated and prop venues; close concurrent-submit race; require authoritative terminal cancellation confirmation in OMS bridge. Tests cover simulator replay/conflict and cancel-ack-with-open-order; existing ambiguous-submit test retained. PR #7 merged 2026-10-27 at `324d327f035ca5ccb573404cc8e446b816990ada`. See REV-009–REV-015 for implementation, CI history, and merge-status correction. Event-stream deduplication is not implemented because no event stream is exposed today.
- [x] **QOS-012 [DONE]** Specify per-account sizing inputs/outputs and loss models. Evidence: `INSTRUMENT_AWARE_SIZING.md` covers account-currency risk budget, tick-value and linear models, FX conversion, costs, exposure/margin/pending-order reserves, volume-step rounding, fail-closed behavior, and 26 acceptance scenarios. Specification only; no risk code changed.
- [x] **QOS-013 [DONE — venue-neutral runtime sizing core; live release remains gated]** Implemented explicit linear/tick-value sizing, timestamped runtime snapshot cache, source/conversion-path validation, account/instrument identity and account-state freshness checks, fail-closed venue-capability and netting/hedging-semantics gates, pending-order exposure/margin reserves, volume-step and min/max/min-notional controls, final modeled-loss checks, and append-only structured sizing audit with exact decimal strings plus the complete policy snapshot. Instrument-aware execution requires persistent audit storage; missing storage blocks new exposure. CI run 40445467228 passed Rust format/check/test/Clippy and frontend lint/typecheck/build. QOS-014/015 must provide real venue/account/quote/instrument data before any live use; no live mode is enabled.
- [ ] **QOS-014 [IN PROGRESS — protocol boundary merged; transport pending]** Versioned MT5 v1 frames/schema/NDJSON decoder are merged in PR #14; authenticated handshake, durable idempotency, reconnect/reconciliation integration, cross-language conformance, and live connector remain open.
- [ ] **QOS-015 [READY — paper/sim first; live access blocked]** Implement the first adapter behind the bridge contract using paper/simulated transport. A live MT5 connection remains blocked until broker/account access and deployment environment are explicitly selected.
- [ ] **QOS-013 [IN PROGRESS — merged implementation; acceptance gaps remain]** PR #12 was merged by the user. Merged code includes timestamped snapshot cache/provider boundary, pending-order exposure and margin reserves, detailed sizing outcomes, append-only sizing audit persistence, and focused sizing tests. Still open: concrete authoritative provider wired to real account/quote/instrument/conversion sources, explicit policy requiring margin inputs, remaining SZ-01–SZ-26/property coverage, and verification against the exact merged main head. Do not mark DONE until these gaps are resolved or explicitly accepted by review.
- [ ] **QOS-014 [IN PROGRESS — protocol boundary merged; transport pending]** PR #14 merged the versioned Rust message models, validation, JSON Schema, protocol docs, account/message/correlation IDs, timestamps, decimal validation, heartbeat/capabilities, accepted/rejected/unknown submit outcomes, cancel-vs-terminal distinction, snapshots, reconciliation request, and bounded 64 KiB incremental NDJSON framing. Still open: authenticated handshake/connection binding, durable idempotency and reconnect/sequence-gap reconciliation integration, executable cross-language conformance against an MT5-side implementation, and a live connector.
- [ ] **QOS-015 [DEFERRED]** Implement first live venue after access/deployment choice; begin with simulator/paper.
- [x] **QOS-016 [DONE — initial deterministic failure-injection suite merged]** PR #15 merged at `0555e365e647d188528a364f51e16a147afc4c96`. Tests cover definite disconnect before submit, ambiguous response loss after venue commit with no blind OMS retry, delayed acknowledgement preserving order identity, and truncated fill acknowledgement exposing a venue/OMS reconciliation mismatch. Multi-account fan-out is QOS-017; additional venue-specific cancel/reconciliation cases remain follow-up coverage.
- [x] **QOS-017 [DONE — merged]** PR #17 merged at `5865020df31b5c64fddd7a189ddf6f0895e924e7`. Sequential non-atomic fan-out, account-scoped client order IDs, per-account sizing-provider requirement, preflight validation, independent results, and failure isolation passed latest-head CI run `40451323267`.

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
