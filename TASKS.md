# Quant OS — Task Queue
Last updated: 2026-10-31. Follow `RULES.md`. One task at a time; only mark DONE with evidence.

## Status definitions
READY = scoped; IN PROGRESS = active; BLOCKED = prerequisite missing; DONE = acceptance passed; DEFERRED = intentionally postponed.

## Execution sequence (task order; product roadmap unchanged)

This sequence is the agreed implementation order. It does not replace product definitions or milestones in `PRD.md` and `ROADMAP.md`.

0. **Implementation baseline and task reconciliation** — verify repository sync, review authoritative rules and project status, reconcile current task states, and record evidence-based acceptance criteria.
1. **MT5 Connectivity Alpha** — complete read-only MT5 demo connectivity and data verification, first in the macOS development environment and then on the intended managed host. QOS-013, QOS-014, QOS-015A, and QOS-015B are dependencies. No order execution is included.
2. **Risk Router and execution core** — complete shared risk authorization, OMS, idempotency, execution outcomes, and reconciliation before exposure-changing workflows.
3. **Trader Dashboard and Journal** — consume real shared account, position, order, market, and risk data through backend contracts; no frontend execution authority.
4. **Shared connectivity platform** — operationalize terminal provisioning, account management, health, recovery, and credential lifecycle.
5. **Developer Platform** — implement tenant-scoped APIs, credentials, usage controls, streams/webhooks, and developer access.
6. **Integrate remaining EdgeNest products** — reuse shared services without duplicating connectivity, execution, or financial truth.
7. **Production readiness** — complete security, resilience, operations, backup/recovery, and controlled rollout gates.

The sequence does not authorize live trading or change the existing product roadmap.

## Milestone 0 — Baseline
- [x] **QOS-001 [DONE]** Audit source, branch/commit, workspace, tests, venue status, frontend status.
  - Evidence: [`BASELINE_AUDIT.md`](BASELINE_AUDIT.md), audited `main` at `14619a758ac9864edb52958195de57fb5ca4b685`; 13 Cargo workspace members, checked-in test inventory, simulated/prop-paper vs live-not-implemented status, frontend mock/API fallback and synthetic events recorded. Source audit only; executable checks remain QOS-002.
- [x] **QOS-002 [DONE]** Run `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and applicable Clippy.
  - Verified on GitHub Actions run 40425656667 (latest PR head): formatting, workspace check, workspace tests, and `cargo clippy --workspace --all-targets` passed. Frontend `npm ci`, lint, typecheck, and build also passed in the same run.
  - The initial formatting failure is retained in REV-004; successful rerun is recorded in REV-005. This is CI verification, not proof of production or live trading readiness.
- [x] **QOS-003 [DONE]** Inspect frontend scripts/dependencies/tokens before UI changes.
  - Evidence: `FRONTEND_AUDIT.md`; source inspection covered scripts/dependencies, environment/data modes, theme tokens, mock/synthetic data, and operational status presentation. CI run 40425954540 passed frontend lint, typecheck, and production build. No UI or runtime behavior changed.

## Current execution status — 2026-10-31

- **Phase 0 — baseline and task reconciliation [DONE].** Local `main` and `origin/main` are both `3e1259063d3194e5b26cf93766ba50306c13fec1`. Current-head workflow run 40858852429 passed all four CI jobs: Rust format/check/test/Clippy (with PostgreSQL), MT5 Python adapter contract tests, frontend lint/typecheck/build, and macOS portable preflight. The local worktree still has an uncommitted `Cargo.lock` modification; it is preserved and was not part of that CI run. See the 2026-10-31 entry in `PROGRESS.md` and the refresh in `BASELINE_AUDIT.md`.
- **Phase 1 — MT5 Connectivity Alpha [IN PROGRESS; QOS-013/014/015A/015B IN PROGRESS].** Source includes the Rust mTLS bridge and read-only daemon, durable sequence/reconciliation persistence, a Windows Python-terminal worker, and a native macOS MQL5 read-only provider. Contract/CI tests exist, but neither a real macOS nor managed-host demo conformance PASS is recorded. QOS-014 describes implemented protocol/transport foundations, not proof of terminal connectivity or order execution.
- **QOS-013 [IN PROGRESS].** Instrument-aware sizing and the persisted-snapshot consumer are present. Real broker data and configured conversion, costs, margin/exposure inputs, volume rounding, final modeled-loss audit, and fail-closed behavior still need target-environment verification.
- **QOS-014 [IN PROGRESS — implementation foundations present; integration evidence pending].** The typed protocol, durable idempotency/checkpoints, session gate, mTLS transport, watchdog, runtime coordinator, and read-only daemon exist. Cross-language terminal conformance, restart/reconciliation against real terminal state, and protected credential lifecycle evidence remain outstanding. Order dispatch is not exposed by the read-only daemon.
- **QOS-015A/015B [IN PROGRESS — real-terminal evidence pending].** Windows and macOS adapter/provider code, smoke/restart tooling, data validation, and persistence paths exist. Real demo identity/data, persistence, reconnect/restart, certificate lifecycle, and independent evidence review have not been verified.
- **Latest operator observation (2026-10-31):** A fresh isolated local PostgreSQL container was started without reusing or deleting existing stopped containers/volumes; all six SQLx migrations applied and the daemon began listening on `127.0.0.1:9443`. Timestamp-correlated MT5 diagnostics established that `TimeCurrent()` remains frozen at the last quote while `TimeTradeServer()` advances; host NTP offset was about 0.45 seconds. The EA now sends `TimeTradeServer()` as `server_time`; the adapter rejects a quote whose timestamp differs from that advancing clock, preserving the stale-data fail-closed behavior. The updated EA compiled with zero errors/warnings and its source/binary were installed in the active terminal's Experts folder with the previous pair backed up. The running chart still exhibits the old frozen-reference failure, so it must reload/re-attach the EA before validating the fix. Its configured EURUSD quote remains stale; no fresh market-data or reconciliation snapshots have been recorded, and the stream remains degraded. This diagnosis/fix is not a QOS-015 conformance PASS. The supervised services run from a separate checkout at `3e12590`, while the protected configuration pins `0097d4b`; exact-commit conformance remains unproven.

### Phase 1 acceptance gate

Use the real designated demo terminal and the exact repository commit. Require all of the following before marking connectivity acceptance complete:

1. Verify broker/server, account identity, and account currency from the terminal; reject mismatches.
2. Verify configured exact symbols, fresh bid/ask quotes, and broker instrument specifications from the terminal (not fixtures).
3. Persist identity-bound account/order/position reconciliation and market-data snapshots to PostgreSQL; verify they survive daemon/worker restart.
4. Verify sequence continuity, worker-only restart while the daemon remains available, and fresh post-restart market-data and reconciliation snapshots.
5. Demonstrate and report healthy, disconnected, stale, and degraded/reconciliation-required states; fail closed on stale data, identity mismatch, gaps, or failed reconciliation.
6. Review certificate trust/pinning/revocation, private-key permissions, rotation, rollback, and rejection of an untrusted/retired identity.
7. Capture redacted evidence tied to the exact commit and obtain independent review.

CI and portable tests are required but cannot substitute for real-terminal evidence. Keep venue order submission/cancellation disabled throughout this phase.

## Platform direction update — planned work, not implementation evidence
- **QOS-018 [READY — architecture/product definition]:** Define EdgeNest as a shared trading infrastructure platform with (1) Trader Dashboard and (2) Developer API as first-party clients of the same services. Build EdgeNest-owned MT5 connectivity and orchestration rather than treating a hosted API vendor as a permanent core dependency. Initial acceptance: documented API/tenant boundary, managed-terminal lifecycle, provider/adapter seam, target-host demo gates, and explicit distinction between current implementation and target design. This documentation change does not complete QOS-013/014/015 and does not authorize order execution.

## Milestone 1 — Product B
- [x] **QOS-010 [DONE]** Specify the existing `VenueAdapter` contract and gaps. Evidence: `VENUE_ADAPTER_CONTRACT.md` documents capabilities, health/readiness, instrument constraints, async events, idempotency, ambiguity, cancel confirmation, reconnect/reconciliation, and 18 test scenarios. Spec-only; no runtime behavior changed.
- [x] **QOS-011 [DONE — merged]** Enforce same-client-ID/same-payload idempotency and reject changed payloads in simulated and prop venues; close concurrent-submit race; require authoritative terminal cancellation confirmation in OMS bridge. Tests cover simulator replay/conflict and cancel-ack-with-open-order; existing ambiguous-submit test retained. PR #7 merged 2026-10-27 at `324d327f035ca5ccb573404cc8e446b816990ada`. See REV-009–REV-015 for implementation, CI history, and merge-status correction. Event-stream deduplication is not implemented because no event stream is exposed today.
- [x] **QOS-012 [DONE]** Specify per-account sizing inputs/outputs and loss models. Evidence: `INSTRUMENT_AWARE_SIZING.md` covers account-currency risk budget, tick-value and linear models, FX conversion, costs, exposure/margin/pending-order reserves, volume-step rounding, fail-closed behavior, and 26 acceptance scenarios. Specification only; no risk code changed.
- [ ] **QOS-013 [IN PROGRESS — core merged; acceptance gaps remain]** Implemented explicit linear/tick-value sizing, timestamped runtime snapshot cache, source/conversion-path validation, account/instrument identity and account-state freshness checks, fail-closed venue-capability and netting/hedging-semantics gates, pending-order exposure/margin reserves, volume-step and min/max/min-notional controls, final modeled-loss checks, and append-only structured sizing audit with exact decimal strings plus the complete policy snapshot. Instrument-aware execution requires persistent audit storage; missing storage blocks new exposure. CI run 40445467228 passed Rust format/check/test/Clippy and frontend lint/typecheck/build. QOS-014/015 must provide real venue/account/quote/instrument data before any live use; no live mode is enabled.
- [ ] **QOS-014 [IN PROGRESS — protocol and read-only daemon implemented; conformance pending]** The versioned protocol, durable order claims/checkpoints, account-bound readiness, one-shot dispatch foundation, mTLS transport, watchdog, runtime coordinator, reconciliation persistence, and read-only daemon exist. Remaining gates include real terminal conformance, broker-specific mapping, restart/reconciliation evidence, and protected credential lifecycle review.
- [ ] **QOS-015 [DEFERRED — demo order execution is a later task]** Consider order submission/cancellation only after read-only conformance passes first on the macOS development terminal and then on the intended managed host, all QOS-013/014/015 acceptance evidence is independently reviewed, and a separate demo-execution scope is approved. Live execution remains disabled.
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


- **QOS-014 heartbeat/session follow-up (PRs #25–#26):** PR #25 added heartbeat freshness checks; merged PR #26 adds autonomous watchdog scheduling, mTLS integration tests, runtime orchestration, dispatch persistence and reconciliation snapshot storage. The read-only daemon was added later; protected certificate lifecycle, real MT5 client conformance and broker mapping remain open.


- **QOS-014 implementation follow-up (PR #26):** PR #26 merged on 2026-10-27. It adds end-to-end generated-certificate TLS tests, autonomous heartbeat watchdog, runtime orchestration, durable heartbeat checkpoints, one-shot dispatch/outcome persistence, typed reconciliation snapshots, append-only PostgreSQL snapshot storage, and restart tests. Live readiness still requires a production host daemon, protected credential lifecycle, real MT5 adapter conformance, broker mapping and end-to-end demo verification.


**Status correction (2026-10-31):** The production read-only daemon is now present. Real terminal conformance, protected certificate lifecycle, broker mapping, and end-to-end demo verification remain open.

## QOS-015A implementation note — 2026-10-27

Branch: `feat/qos-015a-python-mt5-readonly`. The read-only Python worker and additive `reconciliation_snapshot` contract are under implementation. Rust session readiness now permits read-only reconciliation when order lookup and sequence tracking are available, while submit authorization independently requires idempotency and a supported order type. This is not order-execution readiness. CI and Windows demo-terminal smoke testing remain required before marking the task done.


## QOS-013 runtime integration follow-up — 2026-10-27

Branch `feat/qos-013-mt5-runtime-integration` composes persisted MT5 snapshots with a separately supplied trusted supplement provider. It selects ask for buys and bid for sells, passes the same broker entry price to risk evaluation and the submitted order request, validates terminal/server/account-currency/snapshot identity, and fails closed when venue capabilities or position semantics are not explicitly validated. Acceptance is pending exact-head CI and runtime integration tests. This does not satisfy the Windows MT5 demo smoke test or authorize live execution.


## Next acceptance slice — QOS-015A/015B Windows demo verification
- Branch `feat/qos-015a-windows-demo-smoke-harness` adds a PowerShell harness and a detailed runbook. It is designed to run on the actual Windows VPS after the daemon and Python worker are started. The script never submits, modifies, or cancels orders.
- Required evidence is a passing target-host smoke run, followed by worker restart/reconciliation recovery, and a protected certificate lifecycle/private-key ACL review. Do not mark QOS-015A/015B or QOS-013 done based on hosted CI alone.


## QOS-015 consolidated release gate — 2026-10-27

- **QOS-015A [IN PROGRESS — tooling merged; real Windows VPS evidence pending].** PR #32 smoke harness, PR #33 certificate/ACL operator kit, PR #34 worker restart verifier, and PR #35 restart-evidence validator are merged. Hosted CI is not a substitute for actual target-host conformance. See `docs/MT5_DEMO_SMOKE_TEST.md`.
- **QOS-015B [IN PROGRESS — hosted contract/persistence tests exist; target-host evidence pending].** Require verified target-host feed, fresh bid/ask and instrument specifications, PostgreSQL persistence, stale-sequence rejection, and exact account/terminal binding before accepting.
- **QOS-013 [IN PROGRESS — sizing implementation merged; deployment evidence pending].** Require verified account-currency conversion, configured cost/slippage inputs, margin/exposure inputs, broker volume rounding, final modeled loss, persistent sizing audit, and missing-data fail-closed behavior.
- **Consolidated evidence tooling [IN REVIEW — PR #36].** Adds a default-BLOCKED manifest and fail-closed validator requiring all preceding gates, certificate revocation/trust review, private-key/parent-directory ACL review, tested rotation/rollback, and independent reviewer sign-off.
- **QOS-015 execution [BLOCKED].** No venue order submission/cancellation work until all gates are PASS and independently reviewed. Keep venue execution disabled. Missing or inaccessible Windows host evidence is not waivable by green CI.


## QOS-015A native macOS provider follow-up — 2026-10-28
- Added a native Python provider that connects to a read-only MQL5 Expert Advisor over authenticated loopback TCP, with configured-symbol allowlisting, account/server identity validation, and read-only account/order/position/instrument/quote mappings.
- Added `adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5`; MetaEditor compilation and real-terminal execution are not available in repository CI and remain unverified.
- Added Python contract tests for provider factory validation, handshake, identity mismatch, protocol mapping, and fail-closed symbol/command handling. Exact-head GitHub Actions run 40653937982 passed Rust format/check/tests/Clippy, frontend lint/typecheck/build, macOS portable preflight, and MT5 Python adapter contract tests.
- Status remains IN PROGRESS. No real MT5 demo smoke, persisted snapshot/restart test, or certificate lifecycle evidence is claimed. Keep venue execution disabled.

## Cross-product architecture requirement — documentation only

All EdgeNest products must reuse the same connectivity, account registry, tenant/security model, risk authorization, OMS/execution, venue adapters, account/event services, persistence and reconciliation. Product definitions and existing milestone scope remain unchanged. Product-specific broker connectivity and execution bypasses are prohibited. The target UX must not require users to attach an EA to charts; EdgeNest operates the managed gateway fleet. Real MT5 demo lifecycle evidence and existing QOS-013/014/015 release gates remain prerequisites. This statement is architecture guidance only and does not mark any implementation DONE or authorize live trading.
