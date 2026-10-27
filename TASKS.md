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
- **QOS-015A [IN PROGRESS — daemon wiring merged; Windows demo smoke test pending]** PR #27 merged the Python worker and reconciliation snapshot contract; PR #28 merged the `mt5-bridge-daemon` with loopback-first mTLS, certificate-fingerprint/terminal/server/account allow-list, PostgreSQL-backed runtime, reconciliation persistence, and fail-closed connection discard. CI run 40499291725 passed the Rust, Python, and frontend checks on its code commit. The real Windows demo-terminal smoke test and certificate lifecycle review remain required.
- **QOS-015B [IN PROGRESS — instrument metadata and fresh quote feed]** Branch `feat/qos-015b-mt5-market-data` adds typed market-data frames, exact configured broker symbols, tick/contract/volume constraints, fresh bid/ask validation, and separate PostgreSQL persistence. CI and integration checks are pending. This is a read-only prerequisite for connecting MT5 data to QOS-013 sizing; order execution remains deferred.
- QOS-015 order execution stays deferred until QOS-015A Windows conformance and QOS-015B market-data persistence/consumer tests pass, then QOS-013 must consume the data with account-currency conversion, costs, margin/exposure controls, and broker-specific volume rounding.
- **QOS-011 [DONE — merged]** PR #7 merged on 2026-10-27 at merge commit `324d327f035ca5ccb573404cc8e446b816990ada`. The PR adds same-client-ID/same-payload replay semantics, typed conflicts, concurrent duplicate protection in paper connectors, and confirmed cancellation checks. Event-stream deduplication remains limited by the current trait having no event-stream surface.

## Milestone 1 — Product B
- [x] **QOS-010 [DONE]** Specify the existing `VenueAdapter` contract and gaps. Evidence: `VENUE_ADAPTER_CONTRACT.md` documents capabilities, health/readiness, instrument constraints, async events, idempotency, ambiguity, cancel confirmation, reconnect/reconciliation, and 18 test scenarios. Spec-only; no runtime behavior changed.
- [x] **QOS-011 [DONE — merged]** Enforce same-client-ID/same-payload idempotency and reject changed payloads in simulated and prop venues; close concurrent-submit race; require authoritative terminal cancellation confirmation in OMS bridge. Tests cover simulator replay/conflict and cancel-ack-with-open-order; existing ambiguous-submit test retained. PR #7 merged 2026-10-27 at `324d327f035ca5ccb573404cc8e446b816990ada`. See REV-009–REV-015 for implementation, CI history, and merge-status correction. Event-stream deduplication is not implemented because no event stream is exposed today.
- [x] **QOS-012 [DONE]** Specify per-account sizing inputs/outputs and loss models. Evidence: `INSTRUMENT_AWARE_SIZING.md` covers account-currency risk budget, tick-value and linear models, FX conversion, costs, exposure/margin/pending-order reserves, volume-step rounding, fail-closed behavior, and 26 acceptance scenarios. Specification only; no risk code changed.
- [ ] **QOS-013 [IN PROGRESS — core merged; acceptance gaps remain]** Implemented explicit linear/tick-value sizing, timestamped runtime snapshot cache, source/conversion-path validation, account/instrument identity and account-state freshness checks, fail-closed venue-capability and netting/hedging-semantics gates, pending-order exposure/margin reserves, volume-step and min/max/min-notional controls, final modeled-loss checks, and append-only structured sizing audit with exact decimal strings plus the complete policy snapshot. Instrument-aware execution requires persistent audit storage; missing storage blocks new exposure. CI run 40445467228 passed Rust format/check/test/Clippy and frontend lint/typecheck/build. QOS-014/015 must provide real venue/account/quote/instrument data before any live use; no live mode is enabled.
- [ ] **QOS-014 [IN PROGRESS — safety foundation merged; live release gates remain]** PRs #14 and #18–#26 deliver the versioned protocol, durable order claims/checkpoints, account-bound readiness, one-shot dispatch, mTLS, watchdog, runtime coordinator and reconciliation persistence. Remaining gates are production daemon wiring, protected credential lifecycle, real terminal conformance, broker-specific mapping and end-to-end demo verification.
- [ ] **QOS-015 [DEFERRED — demo order execution is a later task]** Implement order submission/cancellation only after QOS-015A read-only reconciliation passes CI and a Windows demo smoke test. Live execution remains disabled.
- [ ] **QOS-014 [IN PROGRESS — protocol and safety foundation merged]** PR #26 merged at `3f7c3a213e7777ccae2f723aca01d496afac2ab6`. Remaining live release gates include production host daemon wiring, protected certificate provisioning/rotation, broker mapping, terminal-side conformance, and end-to-end demo verification.
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


- **QOS-014 heartbeat/session follow-up (PRs #25–#26):** PR #25 added heartbeat freshness checks; merged PR #26 adds autonomous watchdog scheduling, mTLS integration tests, runtime orchestration, dispatch persistence and reconciliation snapshot storage. A production daemon, protected certificate lifecycle, real MT5 client conformance and broker mapping remain open.


- **QOS-014 implementation follow-up (PR #26):** PR #26 merged on 2026-10-27. It adds end-to-end generated-certificate TLS tests, autonomous heartbeat watchdog, runtime orchestration, durable heartbeat checkpoints, one-shot dispatch/outcome persistence, typed reconciliation snapshots, append-only PostgreSQL snapshot storage, and restart tests. Live readiness still requires a production host daemon, protected credential lifecycle, real MT5 adapter conformance, broker mapping and end-to-end demo verification.


## QOS-015A implementation note — 2026-10-27

Branch: `feat/qos-015a-python-mt5-readonly`. The read-only Python worker and additive `reconciliation_snapshot` contract are under implementation. Rust session readiness now permits read-only reconciliation when order lookup and sequence tracking are available, while submit authorization independently requires idempotency and a supported order type. This is not order-execution readiness. CI and Windows demo-terminal smoke testing remain required before marking the task done.
