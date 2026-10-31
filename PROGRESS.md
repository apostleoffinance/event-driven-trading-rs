# Quant OS — Progress

Last updated: 2026-10-31. Only verified facts belong here; see `RULES.md`.

## Current status — 2026-10-31

- **Repository:** at the Phase 0 baseline, local `main` and `origin/main` were both `aa0cc3da3258067f46da4485c54f6f0427f67013` and the worktree was clean. The current v2 source and evidence updates are uncommitted changes on top of that baseline.
- **Code CI:** GitHub Actions run 41159296295 completed successfully on this exact commit. Rust format/check/test/Clippy (with PostgreSQL), MT5 Python adapter contract tests, frontend lint/typecheck/build, and macOS portable preflight all passed. This does not prove real MT5 terminal compatibility or release readiness.
- **Phase 0 — implementation baseline and task reconciliation:** complete for repository sync, source/status review, current acceptance criteria, and CI evidence capture. Historical audit content below remains historical and is not the current implementation snapshot.
- **Phase 1 — MT5 Connectivity Alpha:** in progress. QOS-013, QOS-014, QOS-015A, and QOS-015B have substantial code and contract tests, but real-terminal demo conformance, persisted data/restart evidence, and independent credential-lifecycle review remain outstanding.
- **QOS-013:** instrument-aware sizing and the persisted-snapshot consumer are implemented. Real broker input validation for conversion, costs/slippage, margin/exposure, volume rounding, final modeled loss, and audit/fail-closed behavior remains part of target-environment acceptance.
- **QOS-014:** typed protocol, durable idempotency/checkpoints, session gate, mTLS transport, watchdog/runtime coordinator, and read-only daemon are present. Real terminal/cross-language conformance, recovery/reconciliation against broker state, and protected credential lifecycle evidence remain outstanding. The read-only daemon exposes no order execution.
- **QOS-015A/015B:** Windows worker and native macOS read-only provider/EA, validation, persistence paths, and smoke/restart tooling exist. A dedicated `release/mt5-bridge-v2-rc1` branch now holds the staged candidate from base `aa0cc3da3258067f46da4485c54f6f0427f67013`. Review corrected a Rust quote-freshness check to use the trusted validation clock; 31 intent-bridge library tests and 67 Python adapter tests pass. No commit has been made; the daemon must be rebuilt and the compatibility preflight rerun after commit approval. Active terminal and managed-host demo conformance are not recorded.
- **QOS-015A-RC terminal/build findings:** The active Wine terminal is PID 27335, launched from `/Applications/MetaTrader 5.app`, and its current MQL5 logs are written under `drive_c/Program Files/MetaTrader 5`, indicating that install tree is the active data root; the File → Open Data Folder path was not independently confirmed. The EA log records the EA loaded at 03:25 and continued chart requests. Installed source reports version 1.00 (SHA-256 `eed359d586fe54e395b640497f3efd17236a7fbfcef8d1dbf6793b61850953cd`); installed EX5 SHA-256 is `0eed63bf6f27ea81be6401f7f1b4b6f8d118bad11e2c45170c7ebac86ee94296`. The exact chart-loaded binary/build is not proven because no runtime 2.00 marker appears and the EX5 is not held open. The staged EX5 log reports generated code and zero errors/warnings, but its Wine launcher exit code 1 remains unexplained; artifact acceptance is provisional.
- **Latest operator observation (2026-10-31):** The active chart has now reloaded the fixed EA. Its timestamp diagnostics establish that `current_epoch` is `TimeCurrent()` and `server_epoch` is `TimeTradeServer()`: at local 03:27:00, the quote/current epoch was fixed at 1793401185 while UTC (1793413620) and server (1793420820) epochs advanced on later samples. The configured quote was 12,435 seconds old relative to UTC and 19,635 seconds behind the advancing server clock; the worker correctly rejects it under strict stale/mismatch checks.

  The EA reports receipt of PING, ACCOUNT, SELECT_SYMBOL, SYMBOL_INFO, and TICK requests. The Python provider parses and validates account and symbol responses, then parses the TICK response and rejects the time mismatch. This is not merely inferred from request logs: the worker reaches the specific post-TICK timestamp rejection. The EA log alone does not confirm response transmission. Because market-data construction fails before completion, no market-data frame or subsequent orders/positions reconciliation request is emitted for these attempts.

  Read-only database inspection found one prior market-data snapshot at sequence 5329 (created 02:15:42Z; quote tick time 02:14:45Z, 57 seconds old at capture) and 17 reconciliation snapshots, latest sequence 5329. The stream is now degraded at sequence 5330 (updated 02:32:42Z); no market-data or reconciliation snapshots were created after 02:15:42Z. Earlier fresh persistence is demonstrated, but the latest retry cycle does not persist data and stable recovery is not demonstrated.

  Rust stdout has no 02:27Z entry. A later repeated cycle is correlated: Rust logs authenticated connection at 02:30:05.494Z and transport-read failure at 02:30:05.928Z, aligned with the terminal TICK diagnostic at 03:30:05.911 local. The active worker stderr lacks per-line timestamps because it runs from a separate legacy checkout, but reports the 19,xxx-second quote/server mismatch. Daemon stderr contains `PoolTimedOut` and connection-reset messages, but the file has no event timestamps and was last modified at 02:37 local, before this screenshot, so those errors are not attributable to this reproduction. In code, the Python worker closes its TLS context when snapshot construction raises; on retry its provider `connect()` calls `shutdown()` and closes/replaces the EA socket. The daemon logs the resulting transport EOF and is not evidenced as the initiator. Runtime worker/daemon HEAD is `3e12590` with local modifications; repository `main` is `aa0cc3d`, and the protected configuration pins `0097d4b`, so exact-commit conformance remains outstanding.
- **Execution remains disabled.** No live order submission/cancellation, production deployment, or money movement is authorized by this status update.

Full audit: [`BASELINE_AUDIT.md`](BASELINE_AUDIT.md).

## Work log — MT5 stale-data continuity (2026-10-31)

- Implemented the coordinated v2 freshness contract across the Python worker, Rust bridge/persistence, and sizing consumer. Quotes are explicitly `fresh`, `stale`, `unavailable`, or `invalid`; actual stale values and timestamps remain visible, while missing/invalid quotes have null price/time fields. Strict 60-second UTC-age validation remains. The prior large server-to-tick gap is no longer misclassified as a timezone failure.
- Stale/missing/invalid quote outcomes do not tear down the authenticated worker session. The worker continues account, orders, positions, and reconciliation requests. Protocol errors, EOF/I/O, and TLS authentication failures remain distinct and fail closed.
- Rust persists freshness status and reconciliation, keeps readiness degraded until a fresh market-data frame plus successful reconciliation, and preserves any prior actual quote separately as explicitly stale `last_known_quotes`. Sizing rejects any non-fresh aggregate or selected quote.
- Verification: Python MT5 adapter suite **63 passed**. Affected Rust package suites (`intent-bridge`, `persistence`, `risk-engine`, `trading-runtime`) passed, including mTLS stale-session continuity/fresh recovery and persistence coverage. `cargo fmt --all -- --check`, v2 schema JSON parsing, and `git diff --check` passed. Ordinary affected-package Clippy completed with warnings; strict Clippy with `-D warnings` remains blocked by lint warnings (`map_or`, too-many-arguments, large-enum, drop-non-drop, question-mark, items-after-test-module). Touched `clone_on_copy` warnings were removed.
- Quality/deployment review: reviewed the complete 24-file worktree diff; no unrelated code surfaces changed. `cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace` passed (51 suites, 201 tests); Python adapter suite passed (63 tests); frontend lint/typecheck/build and macOS portable local checks passed. Full workspace Clippy passed with pre-existing warnings; strict workspace Clippy fails on pre-existing `module_inception` warnings in `event-trading`, while strict Clippy for `risk-engine` passes. Newly introduced `clone_on_copy` warnings in changed Rust code/tests were removed. `npm ci` reported 20 audit advisories (14 moderate, 5 high, 1 critical); no unrelated upgrades were applied. Schema JSON and diff checks passed.
- MetaEditor deployment preparation: a copy of the updated EA compiled in a unique temporary directory; compiler log reports generated code, 0 errors, 0 warnings, X64 Regular, 2197 ms. The Wine process returned exit code 1, so that launcher status remains an unresolved caveat. The terminal installation is `/Applications/MetaTrader 5.app` using the `net.metaquotes.wine.metatrader5` Wine prefix. Located EA artifacts are under `drive_c/Program Files/MetaTrader 5/MQL5/Experts`; their source and binary hashes differ from current source/build, and the exact file loaded by the active chart could not be proven. No terminal files were replaced.
- Active services: Python worker and Rust daemon run from `~/rust-trading-engine/event-trading` at `3e1259063d3194e5b26cf93766ba50306c13fec1`; their source contracts remain v1. The checkout has unrelated local changes to `Cargo.lock`, the macOS launch wrapper, and `docker-compose.yml`, all preserved. It is not compatible with the v2 source here; no live v2 runtime scenario was attempted. A controlled test matrix is prepared in `docs/QOS-015_MACOS_CONFORMANCE.md`.
- Limits: actual stale-market continuity, fresh-quote acceptance, current-cycle persistence/reconciliation, worker/daemon/terminal restarts, actual socket recovery, and authentication/version mismatch remain unverified against the live demo terminal. A matching staged v2 pair, confirmed active EA path, and genuine fresh broker tick are prerequisites. Phase 1 remains **IN PROGRESS** and order execution remains disabled.

## Work log — QOS-015A protocol v2 staging readiness (2026-10-31)

- Created an isolated detached Git worktree based on `aa0cc3da3258067f46da4485c54f6f0427f67013` and overlaid the current v2 source snapshot. Source-only fingerprint: `7a5924f59c0cede1980ba9ea925c051850ee297ad95494ca77d06657d2d70761` (excludes docs, status trackers, build outputs, Python caches, `.git`, and the manifest itself). There is no commit yet containing this implementation; deployment must wait for an approved/reviewed commit and a rebuild from it.
- Built `target/release/mt5-bridge-daemon` from the staged tree with release profile and debug info stripped. SHA-256: `81b002f054aef7ea6734c160d57f54d9b644877f52eef2c9a164b6fce48a51ac`. The staged worker and native provider hashes are respectively `3f0e71348ab8919bc72f439273eab5eb1e3dcfc2d459542d23d2a586c4c3db97` and `fd9dfd87babe68ae66d59c27462f0626ad464331a7c5bf45a9d98bc20ae5477f`. The Python worker is a source-run module; no separate native package build is used.
- Added fail-closed `mt5_protocol_preflight.py`: validates Python/Rust MT5 Bridge schema v2, local Python↔EA provider protocol v1, EA/Python build version 2.00, daemon under staging root, and worker/provider/daemon/EA-source/EA-binary hashes against expected build-manifest values. Python worker rejects stale/mixed EA build or provider protocol on PING. Added mismatch tests.
- Built isolated EA artifact from the staged source. MQL5 source SHA-256: `f43bbf5c4f834f2763689066e6d57c15af789c533c14397f75f1122b0a8e002c`; EX5 SHA-256: `60c66148efef6acd1b6b8bdf0adc496aa19b440491495a56d927d24f9a41aa45`; output mtime `2026-10-31T04:43:18+01:00`. MetaEditor log: code generated, 0 errors, 0 warnings, X64 Regular, 2118 ms. Wine process exit code remained 1; a pre-existing MetaEditor GUI process and Wine `fixme` diagnostics make single-instance handoff plausible but do not prove the cause. The EX5 remains in session-local staging storage and was not installed.
- Verification: Python adapter suite **67 passed**; isolated release build passed; Rust formatting and staged `git diff --check` passed; compatibility preflight passed. An initial release build ran out of disk while debug outputs filled the isolated staging area. Only those generated staging target directories were removed, then a stripped release build completed. No unrelated data was removed.
- Terminal inspection found the installed EA source and binary under `drive_c/Program Files/MetaTrader 5/MQL5/Experts`, with hashes `eed359d586fe54e395b640497f3efd17236a7fbfcef8d1dbf6793b61850953cd` and `0eed63bf6f27ea81be6401f7f1b4b6f8d118bad11e2c45170c7ebac86ee94296`. Active terminal logs are open under the install tree. The roaming terminal profile `D0E8209F77C8CF37AD8BF550E51FF075` lacks MQL5/Experts, so install-tree data mode is likely but the authoritative “Open Data Folder” path and chart-loaded EX5 identity remain unconfirmed.
- Active v1 processes (daemon PID 45101; worker PID 64095) remained running from `~/rust-trading-engine/event-trading` at `3e1259063d3194e5b26cf93766ba50306c13fec1`. No terminal/service restarts, v2 launch, file installation, backup overwrite, or changes to the three active-checkout modifications occurred. Dedicated staging ports are planned (`19443` daemon, `18765` provider); they are not in use.
- Runtime plan documents stale/weekend account/orders/positions reconciliation, fresh tick recovery, worker/daemon/terminal restarts, socket loss, authentication/version mismatch, and persistence/sequence continuity. All live scenarios remain **NOT RUN**. Order execution remains disabled; Phase 1 remains **IN PROGRESS**.

## Work log — MT5 quote-clock fix (2026-10-31)

- Diagnosed the false future/non-timezone-like quote timestamps: MQL5 `TimeCurrent()` is the last quote time and freezes when no quote arrives, while `TimeTradeServer()` advances with the terminal clock. Host NTP offset was approximately 0.45 seconds, so host clock drift did not explain the observed offset.
- Changed the native EA's `server_time` field to `TimeTradeServer()` and added/retained worker regression coverage that rejects frozen quotes against an advancing server clock without relaxing freshness limits.
- Compiled the EA in the isolated diagnostics directory (MetaEditor log: zero errors and warnings), backed up the prior files, installed the source and `.ex5`, and confirmed the operator reattached it. The active terminal diagnostics now show `TimeTradeServer()` advancing while `TimeCurrent()`/quote time is frozen; the stale quote is rejected.
- Focused tests passed: `test_worker.py` (17) and `test_macos_socket_provider.py` (8); full adapter suite passed (59). A read-only database check found one earlier fresh market-data snapshot and 17 reconciliation snapshots through sequence 5329, but none after 02:15:42Z; stream sequence 5330 is degraded. The current stale quote is not persisted. Exact-commit, managed-host, certificate lifecycle, sustained reconnect/recovery, and independent-review gates remain open. No order execution is enabled.

## Historical baseline evidence — 2026-10-27

### Repository and workspace
- Default branch `main` at audit time: `14619a758ac9864edb52958195de57fb5ca4b685`.
- Cargo workspace declares 13 members: domain, events, strategy-runtime, risk-engine, account-engine, execution-engine, venue-connectors, persistence, trading-runtime, reconciliation, intent-bridge, event-trading, and strategies/mean-reversion.
- Python strategy implementation/tests are outside the Cargo workspace.
- Recursive tree inspection found 13 checked-in Rust integration-test files, inline unit-test modules, and `strategies/python/tests/test_mean_reversion.py`. Inventory does not establish test success.
- GitHub Actions returned zero workflow runs and the repository tree contained no `.github/workflows` files; no CI result is available.
- A local clone attempt failed with `Could not resolve host: github.com`. The command failed before toolchain versions or tests could be checked.

### Execution and venue
- `VenueAdapter` has basic account-state, submit/cancel, open-orders, positions and order-lookup methods. It lacks explicit capabilities, instrument-spec lookup, health/readiness and async venue-event stream methods.
- `SimulatedVenue` and `PropVenue::paper` provide simulated/paper execution paths; checked-in integration tests cover both, but tests were not executed in this audit.
- Prop live mode is explicitly gated and returns `VenueError::NotImplemented`; the intent-bridge CLI chooses simulated or prop but has no live-mode flag.
- Binance/Bybit fetchers exist in the legacy engine for public market data; this is not proof of authenticated order execution.
- No MT5/cTrader execution adapter was found in the workspace tree.
- Runtime `execute_intent` uses one configured account and venue; multi-account fan-out is not implemented in this path.
- Account registry is in-memory; a separate PostgreSQL persistence crate exists. Vault account registration is explicitly rejected/reserved.
- Sizing caps by position, exposure, leverage and stop-distance, but the inspected trade-risk cap does not model contract/tick value, quote-currency conversion, or venue lot/volume steps.
- Submit ambiguity is represented as `Unknown`. Cancellation currently transitions to cancelled after adapter cancel success; provider-specific terminal-confirmation semantics still require testing.

### Frontend
- Next.js 15.5.7 / React 19 / TypeScript project. Scripts: dev, build, lint (`next lint`) and typecheck (`tsc --noEmit`).
- Frontend README declares PAPER mode, mock repositories and mock event stream; it does not execute trades or evaluate risk.
- API mode is currently a stub that falls back to mock repositories. Simulation controls emit synthetic intent/risk/order/fill/position events.
- Environment label can be configured as LIVE independently of backend connectivity; label alone is not evidence of a live backend. Default label is PAPER.
- Theme provider supports dark/light and defaults to dark; current CSS uses blue accents rather than the white-dominant navy/orange design direction. No UI changes were made.
- Frontend lint, typecheck, and production build passed in CI run 40425954540 on PR #3's latest head before it was merged. See `FRONTEND_AUDIT.md` for the source audit.
- Initial Rust CI run 40425100977: `cargo fmt --all -- --check` failed on five formatting-only diffs; compile/tests/Clippy were skipped. See REV-004.
- Successful rerun on latest PR head: GitHub Actions run 40425656667 completed with conclusion `success`. Rust steps passed: `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets`. Frontend steps passed: `npm ci`, `npm run lint`, `npm run typecheck`, and `npm run build`. See REV-005.

## Work log — 2026-10-27
- Verified PR #1 merged to `main`; merge commit `14619a758ac9864edb52958195de57fb5ca4b685`.
- Re-read `RULES.md` before QOS-001.
- Inspected repository tree, Cargo workspace, checked-in test inventory, venue adapter/types/bridge/simulated/prop implementation, runtime configuration/pipeline, risk sizing, account service/domain, intent-bridge CLI, migration audit, frontend package/README/config/repository factory/mock provider/simulation provider/theme/CSS.
- Added `BASELINE_AUDIT.md` and updated `TASKS.md`, `PROGRESS.md`, and `REVIEWS.md` on branch `audit/qos-001-baseline`.
- No trading code changed; no live mode, deployment, or money movement was enabled.
- No executable tests/checks were run. Local clone attempt failed due to DNS resolution for github.com.

## Work log — QOS-003 frontend baseline (2026-10-27)
- Read `RULES.md` and inspected the current frontend source on `main` before proposing UI changes.
- Added `FRONTEND_AUDIT.md`; recorded script/dependency alignment risk, mock/API fallback behavior, environment-label semantics, hardcoded system-health presentation, stale mock telemetry, and theme-token mismatch.
- Confirmed CI run 40425954540 passed frontend lint, typecheck, production build, and Rust checks on PR #3's latest head before it was merged.
- No UI, runtime, dependency, or trading behavior changed. Next work is contract specification, not live integration.

## Work log — QOS-002 CI verification (2026-10-27)
- Added CI workflow in PR #3 to execute baseline checks on GitHub-hosted runners because local clone DNS resolution failed.
- Corrected five rustfmt-only differences after the first run. No behavior changes were intended.
- Verified latest-head CI run 40425656667 passed all Rust and frontend steps. QOS-002 acceptance criteria are met by remote CI evidence; no claims are made about live-venue readiness.

## Roadmap status

| Order | Workstream | Status | Evidence to exit |
|---|---|---|---|
| 0 | Baseline source audit | DONE | `BASELINE_AUDIT.md`; executable checks are tracked separately in QOS-002 |
| 0b | Rust formatting/check/test/clippy | DONE (QOS-002) | GitHub Actions run 40425656667: formatting, check, test and Clippy passed |
| 0c | Frontend scripts/dependencies/tokens | DONE (QOS-003) | `FRONTEND_AUDIT.md`; CI run 40425954540 passed frontend checks |
| 1 | Product B multi-account copier | Planned | QOS-010 adapter contract and QOS-012 sizing specification before implementation |
| 2 | First traditional adapter | Planned; venue choice pending | provider tests, paper evidence, recovery/reconciliation |
| 3 | Product A risk co-pilot | Planned | enforcement capability matrix and confirmed-action states |
| 4 | Product C hosted strategies | Planned | isolation, versioning, reproducible performance evidence |
| 5 | EdgeNest Vault Protocol | Deferred | threat model, accounting invariants, audit, legal review |

## Blockers and risks
- GitHub-hosted CI now verifies the baseline Rust and frontend commands. Local development still benefits from a working checkout and installed toolchains.
- The first live venue and deployment environment must be confirmed before selecting a bridge.
- Multi-account execution is non-atomic; partial outcomes must be first-class.
- Risk enforcement depends on venue permissions and connection state.
- Hosted strategies and investor-facing vaults require security, operational and legal review.

## Work log — QOS-010 venue adapter contract (2026-10-27)
- Re-read `RULES.md` and inspected the current `VenueAdapter`, request/ack/error types, `InstrumentSpec`, event crate, simulated adapter, cancel bridge, and architecture docs.
- Added `VENUE_ADAPTER_CONTRACT.md` with explicit capability, readiness, instrument, event-stream, idempotency, ambiguity, cancel-confirmation, reconnect, and reconciliation requirements plus 18 contract-test scenarios.
- Updated task status to QOS-010 DONE for specification only; QOS-011 remains blocked pending review of the contract and test matrix.
- No runtime, trading, frontend, dependency, or live-mode behavior changed. No tests were run because this change is documentation-only.

## Work log — QOS-012 instrument-aware sizing specification (2026-10-27)
- Re-read `RULES.md`; verified PR #5 merged at `f3f07c6387f1e124ccfa250b1961f3a3e42de032` and inspected current risk sizing, `RiskRequest`/`RiskPolicy`/`RiskDecision`, `InstrumentSpec`, and venue adapter contract.
- Added `INSTRUMENT_AWARE_SIZING.md` defining required per-account inputs, tick-value and linear loss models, currency conversion, costs/buffers, exposure/margin/pending-order constraints, conservative volume-step rounding, audit outputs, and 26 deterministic acceptance scenarios.
- Updated task/progress/review records. No risk/runtime/frontend code changed. No executable tests run because the change is documentation-only.

## Status reconciliation
PR #7 is merged, not open: GitHub PR metadata reports merged on 2026-10-27 with merge commit `324d327f035ca5ccb573404cc8e446b816990ada`. The old open/unmerged wording below in the historical QOS-011 work log is retained as a time-of-writing record and superseded by this correction. Current next task is QOS-013; its source audit and acceptance plan are in `QOS-013_IMPLEMENTATION_PLAN.md`. No QOS-013 runtime code or live execution was enabled.


## Work log — QOS-011 venue adapter hardening (2026-10-27)
- Re-read `RULES.md` and inspected the current venue adapter, simulated and prop connectors, order lifecycle, OMS cancel methods, bridge, ambiguous-submit test, and `VENUE_ADAPTER_CONTRACT.md` on main at `a39fd9d9a1bc0bb0b4fb8f7a09737a8b09aa1bf0`.
- On branch `fix/qos-011-venue-contract`, added a typed `ClientOrderIdConflict` error and immutable submit-payload comparison. Both simulated and prop venues reject reuse of a client order ID with changed account/order payload and recheck under the mutation lock to close a concurrent duplicate-submit race.
- Updated the OMS/venue bridge so a cancel request acknowledgement is not treated as terminal success: it queries the venue order and only marks OMS Cancelled when identity matches and venue status is Cancelled; unconfirmed/mismatched state returns Ambiguous for reconciliation.
- Added simulated venue tests for same-payload replay and changed-payload conflict, plus a bridge regression test where cancel is acknowledged but the order remains open. Existing ambiguous-submit regression remains in place.
- Event-stream deduplication remains out of scope for this code slice because the current `VenueAdapter` exposes no event-stream API; this limitation is explicit and no event behavior is claimed.
- No live adapter, deployment, frontend, sizing logic, or money movement enabled. Initial CI runs found formatting drift and a missing typed `ClientOrderIdConflict` variant (REV-010–REV-012). After correction, latest-head run 40433848539 passed all Rust and frontend jobs (REV-013). PR remains unmerged; no live execution is enabled.


## Work log — QOS-011 merge reconciliation and QOS-013 kickoff (2026-10-27)
- Re-read `RULES.md` before this task; verified PR #7 metadata reports `merged: true`, merged at `2026-10-27T01:48:38Z`, merge commit `324d327f035ca5ccb573404cc8e446b816990ada`.
- Corrected current task/progress summaries that still described QOS-011 as open/unmerged. Historical work-log entries retain their original time-of-writing status and are explicitly superseded by the correction.
- Re-inspected `crates/risk-engine/src/sizing.rs`, `crates/risk-engine/src/evaluator.rs`, `crates/domain/src/risk.rs`, `crates/domain/src/instrument.rs`, `crates/domain/src/account.rs`, `crates/trading-runtime/src/pipeline.rs`, and `INSTRUMENT_AWARE_SIZING.md`.
- Added `QOS-013_IMPLEMENTATION_PLAN.md` to record observed source gaps, scope boundaries, and testable acceptance criteria before implementation.
- No runtime code or tests changed in this kickoff; no executable checks were run for this documentation-only change. No live mode, deployment, or money movement was enabled.


## Work log — QOS-013 instrument-aware sizing implementation slice (2026-10-27)
- Re-read `RULES.md` and the QOS-013 implementation plan before code changes.
- Added `crates/risk-engine/src/instrument_sizing.rs` with explicit linear and tick-value loss models, caller-supplied account-currency conversion rates, freshness fail-closed behavior, side-aware stop geometry, checked decimal arithmetic, risk/position/exposure/leverage caps, volume-step round-down, min/max and minimum-notional checks, and post-rounding modeled-loss verification.
- Exported the sizing API and added `DefaultRiskEvaluator::evaluate_with_instrument_sizing`, which runs the existing kill-switch/instrument/session/drawdown/daily-loss/position gates before sizing. The legacy `evaluate` path remains for compatibility and does not yet require the new context by default.
- Added seven focused sizing unit tests and one evaluator gate-order regression test. Expected values were checked against the formulas; CI is still required to validate compile/format/test/clippy.
- No venue I/O, live-mode changes, deployment, or money movement. QOS-013 remains IN PROGRESS because production snapshot wiring and several specification scenarios are not yet implemented.


## Work log — QOS-013 CI formatting correction (2026-10-27)
- Initial CI run 40435500152 failed at `cargo fmt --all -- --check`; the workspace check, tests, and Clippy were skipped.
- Applied the exact formatting differences reported by the Rust job and logged the failure/correction in REV-017.
- The latest-head full CI rerun is required before claiming verification.


## Work log — QOS-013 final formatting correction (2026-10-27)
- Follow-up CI run 40435630512 still failed `cargo fmt --all -- --check` on one match-arm wrapping difference; compile/tests/Clippy were skipped.
- Applied the remaining exact rustfmt diff and recorded it in REV-018. Awaiting latest-head CI.


## Work log — QOS-013 compile correction (2026-10-27)
- CI run 40435757549 passed formatting but failed `cargo check --workspace` with E0631 on the `Decimal::trunc` function pointer.
- Changed the call to a closure and recorded the failure/correction in REV-019. Full CI rerun pending.


## Work log — QOS-013 sell-stop test fixture correction (2026-10-27)
- Reviewed failed Rust job 121964300309 in CI run 40435876274. Formatting/check and 25 of 26 risk-engine unit tests passed; the failing test supplied a valid sell protective stop (entry 100, stop 101) but expected invalid geometry.
- Corrected the regression fixture to use equality (entry 100, stop 100), consistent with the test name and sizing contract. Production validation remains unchanged and still requires sell stop strictly above entry.
- Recorded the failure/correction as REV-020. Frontend job 121964300550 passed. Corrected-head CI run 40436418111 passed all Rust and frontend steps. QOS-013 remains in progress because production runtime snapshot wiring and other contract items remain open.


## Work log — QOS-013 explicit runtime sizing boundary (2026-10-27)
- Added `TradingRuntime::execute_intent_with_instrument_sizing`, which requires an explicit `RuntimeInstrumentSizingContext` containing account ID, instrument ID, and `InstrumentSizingContext` and calls `DefaultRiskEvaluator::evaluate_with_instrument_sizing`; it does not silently fall back to legacy sizing when this entry point is chosen.
- Added `instrument_aware_runtime_rejects_stale_sizing_before_oms` integration coverage, checking the decision is rejected and no OMS open order is created for stale context.
- The existing `execute_intent` path remains backward-compatible and still uses legacy sizing. This slice does not claim runtime-owned authoritative snapshot sourcing; context trust, timestamps/freshness policy, margin, pending-order reserves, and persisted structured sizing audit remain open.
- Code and integration test pushed to `feat/qos-013-runtime-sizing-boundary`. Initial run 40438016915 failed the formatting gate only; rustfmt corrections were applied and logged in REV-022. Corrected-head CI run 40438207539 passed Rust format, workspace check/test/Clippy and frontend lint/typecheck/build. No live mode, deployment, or money movement enabled.


## Work log — frontend Google font build failure correction on QOS-013 branch (2026-10-27)
- Inspected failed frontend job 121972469846 in run 40438370475. `npm ci`, lint, and typecheck passed; build failed in `next/font/google/loader.js` with `TypeError: Cannot read properties of null (reading '1')` from `frontend/src/app/layout.tsx`.
- Removed the build-time `next/font/google` dependency and switched global/chart typography to stable system font stacks. Theme switching and PAPER metadata are unchanged.
- Applied the three frontend changes directly to `feat/qos-013-runtime-sizing-boundary` so PR #10 will rerun against the combined changes. Correction logged in REV-024.
- CI for the combined latest head is pending. No build/test success is claimed until that run completes. No live mode, deployment, or money movement enabled.


## Work log — QOS-013 snapshot freshness, reserves, and audit persistence (2026-10-27)
- Re-read `RULES.md` and inspected the merged QOS-013 runtime path, risk sizing implementation, persistence façade, and SQLx migration layout.
- Added a runtime snapshot-provider contract. Each sizing bundle carries a non-empty source ID, capture timestamp, and explicit positive max-age; future-dated, expired, unknown-source, identity-mismatched, or stale bundles reject before OMS. Provider errors propagate and do not fall back to legacy sizing.
- Extended instrument-aware sizing to reserve pending-order notional exposure and pending-order margin, apply an optional available-margin-per-unit cap, round down, and recheck modeled loss. Added tests for pending exposure, margin caps, pending margin reserves, malformed margin inputs, and deterministic quantity/loss invariants.
- Added a detailed evaluator outcome and an append-only `sizing_decision_audits` JSONB table/repository method. When a store is configured, the runtime records exact decimal strings for request, model, constraints, sizing result, and decision before OMS submission. Persistence failure therefore prevents the order from proceeding.
- Branch/PR: `feat/qos-013-complete-sizing-snapshots`, draft PR #12. CI is still running/being corrected; QOS-013 remains IN PROGRESS. The provider is an interface, not yet a concrete authoritative venue/account/quote source. No live adapter, deployment, or money movement enabled.


## Work log — QOS-013 completion and capability fail-closed gates (2026-10-27)
- PR #12 merged at `148ee80c8dc9be7e9d45b5f5acd80b30685149b3`. Its full CI run `40444671655` passed Rust format/check/test/Clippy and frontend lint/typecheck/build.
- Follow-up PR #13 adds explicit `venue_capabilities_validated` and `account_position_semantics_known` requirements to runtime snapshots. The runtime-owned cache refuses unknown capabilities, unknown netting/hedging semantics, stale/future snapshots, empty source/conversion paths, out-of-order updates, and conflicting equal-timestamp snapshots.
- Instrument-aware execution now rejects when persistent storage is unavailable, so an executable sizing decision cannot reach OMS without the structured sizing audit. The audit stores exact decimal strings, source/conversion paths, snapshot times, model version, full policy contents, account state, constraints, decision and sizing result.
- Added deterministic generated-spec invariant tests, venue maximum/minimum notional/multiplier/account-independence tests, cache eligibility tests, and a regression that fresh instrument-aware inputs still cannot create an order without audit storage.
- PR #13 CI run `40445467228` passed Rust format, workspace check, workspace tests, all-target Clippy, frontend typecheck and production build. PR #13 remains open/draft for review. QOS-013 is complete for the venue-neutral runtime core; real venue snapshot production and live execution remain out of scope until QOS-014/015. No live mode, deployment or money movement enabled.
## Work log — QOS-014 MT5 bridge v1 protocol boundary (2026-10-27)
- Re-read `RULES.md` and inspected the existing Python-to-Rust `TradeIntentWire`, NDJSON ingestion, CLI, contract schema, venue adapter contract and workspace dependency setup.
- Added `crates/intent-bridge/src/mt5_v1.rs` with versioned typed frames and payloads for heartbeat/capabilities, order submit/outcome, cancel/outcome, order snapshot and reconciliation request. Validation is clock-injected and checks schema version, IDs, future timestamp skew, order-type requirements, positive quantities/prices and canonical base-10 decimal strings.
- Added a bounded incremental NDJSON decoder with a 64 KiB per-frame limit, arbitrary chunk handling, multiple frames per chunk, CRLF support, strict newline framing, and fail-closed behavior for blank, malformed, non-UTF-8, oversized, and incomplete-EOF frames. Added tests for framing, poisoned-session behavior, round-trip, unsupported version, future timestamps, malformed decimal lexemes, zero quantity, unknown submit outcomes, and cancellation acknowledgement semantics.
- Added `docs/contracts/mt5_bridge_v1.schema.json`, `docs/contracts/fixtures/mt5_v1_submit_order.json`, and `docs/MT5_BRIDGE_V1.md`. The protocol explicitly distinguishes account from terminal identity, accepted from unknown submit outcome, and cancel request from terminal cancellation.
- A first file creation was accidentally committed directly to `main`; I immediately removed that file in a separate revert commit before continuing on `feat/qos-014-mt5-bridge-v1`. The working branch now contains the implementation. This correction is recorded in REV-027. No direct-to-main implementation remains from that mistake.
- Current status: QOS-014 IN PROGRESS. This is protocol/type validation only, not an MT5 Expert Advisor, socket transport, authenticated handshake, persistent idempotency store, broker mapping or live execution connector. Latest-head CI is required before claiming tests pass. No live execution, deployment or money movement enabled.


## Work log — QOS-017 multi-account orchestration (2026-10-27)
- Re-read `RULES.md` and inspected the current single-account `TradingRuntime::execute_intent` path, runtime config, snapshot-provider contract, OMS client-ID generation, venue adapter boundary, and QOS-016 failure-injection tests.
- Added `crates/trading-runtime/src/multi_account.rs` with sequential non-atomic fan-out across separately configured account runtimes. It preflights non-empty target lists, duplicate account IDs, and runtime-to-venue bindings before any account is attempted.
- Account-local errors are returned as per-account results and do not stop later targets. Missing sizing providers fail closed; multi-account fan-out does not fall back to legacy sizing. Instrument-aware snapshots are loaded separately per account and passed to the existing fail-closed runtime path.
- Added deterministic account-scoped client order IDs with length-prefixed account and intent components; retries for one account/intent are stable while separate accounts receive distinct keys.
- Added tests for empty target lists, duplicate-account preflight, venue-binding mismatch, per-account failure isolation, and account-scoped idempotency-key stability/distinctness.
- Current status: implementation is on `feat/qos-017-multi-account-orchestration-v2`; full CI and source-level test verification are pending. This is sequential orchestration only, not atomic multi-account execution, live venue integration, deployment, or permission to move money.


## Work log — QOS-014 durable idempotency and sequence checkpoint slice (2026-10-27)
- Re-read `RULES.md`, the MT5 v1 protocol, PostgreSQL migration runner, and persistence repository patterns.
- Added migration `20261027000002_mt5_bridge_idempotency.sql` with an account-scoped client-order key ledger and per-account/terminal stream checkpoint table.
- Added transactional `TradingStore` APIs to claim IDs before sending, compare canonical JSONB payloads, persist accepted/rejected/unknown outcomes without rewriting terminal states, observe sequence continuity, and restore readiness only after explicit reconciliation.
- Added MT5 protocol helpers to claim submit orders before transport send, persist outcomes, observe connected heartbeat sequences, and mark a stream reconciled. Existing claims are never treated as permission to resend; even `prepared` is ambiguous after process restart.
- Updated the MT5 bridge contract and task/progress records. CI is not yet verified for this branch; no tests are claimed to pass.
- Remaining: transport must call the helpers around actual send/response paths; DB-backed concurrency/restart/reconnect tests and latest-head CI remain required. No MT5 terminal connection, live venue execution, deployment, or money movement is enabled.


## Work log — QOS-014 first full CI pass for durable persistence slice (2026-10-27)
- Latest implementation head `5912da7143b3d5d1ee9cf703d8f474791c28207c` passed GitHub Actions run `40452429799`: `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, frontend lint, typecheck and production build all succeeded.
- DB-backed tests passed for idempotency claim persistence across a fresh store handle, payload conflict rejection, unknown outcome recovery, terminal outcome immutability, sequence gap degradation, explicit reconciliation, and sequence range validation.
- Initial attempts failed the formatting gate only; applied the exact rustfmt changes and reran. No compile, test, Clippy or frontend failures remained on the verified code head.
- This log/status update changes documentation only; the follow-up CI run on the updated PR head is the final verification gate.


## Current implementation slice — MT5 session gate

- Added `Mt5SessionPolicy` and `Mt5SessionGate` in `crates/intent-bridge/src/mt5_session.rs`.
- Session is bound to one allow-listed account, terminal ID, and broker server; identity switching closes the session.
- Dispatch is blocked unless safe capabilities are declared, the connection is connected, durable stream readiness is ready, and explicit reconciliation has been confirmed.
- Submit/cancel gates check account binding and declared order/cancel capabilities; they do not replace risk authorization or OMS lifecycle checks.
- Unit tests cover fresh/gapped streams, disconnect/reconnect, account switching, non-allow-listed identities, missing capabilities, and allowed submit/cancel frames.
- CI run `40453466375` passed on head `36c233fcf3274191dce9bcb15d5b9bd97a078541`: Rust format/check/test/Clippy and frontend lint/typecheck/build.
- This module expects the future transport to authenticate the peer before constructing a session. It does not implement authentication, sockets, an MT5 Expert Advisor, broker mapping, or live execution.


## Current implementation slice — durable MT5 dispatch coordinator

- Added `crates/intent-bridge/src/mt5_dispatch.rs` and public exports.
- `prepare_submit` authorizes against the merged session gate, validates the request, then commits the durable account-scoped idempotency claim before returning any send permit.
- Only a new durable claim returns `Mt5DispatchPermit`; every existing claim returns `ReconcileRequired` and cannot authorize a blind resend.
- The permit's outcome method validates account, correlation ID, client-order ID and timestamps before persisting accepted/rejected/unknown state. Ambiguous transport failure is persisted as unknown, consumes the permit, and degrades the session. Failure to persist a possible venue outcome also degrades the session.
- Added unit tests for outcome identity binding and state mapping. GitHub Actions run `40454050187` passed on implementation head `80e74f7acf2786065800b5fa075a0216a8b965c9`: Rust formatting, workspace check/tests, all-target Clippy, frontend lint, TypeScript typecheck and production build. A final run is required after verification-document updates.
- This is not an actual MT5 transport, and no socket, EA, broker mapping, live order submission, or deployment is enabled.


## Work log — QOS-014 authenticated transport security contract (2026-10-27)

- Verified PR #20 is merged at merge commit `3de8c7abd10cfe72b987d8f567ae5ce10864a4a1`.
- Added `docs/MT5_TRANSPORT_SECURITY.md` to define the security requirements the concrete transport must satisfy before a session gate can be created: authenticated peer identity, confidentiality/integrity, host-managed principal-to-terminal/account mapping, bounded framing/resources, replay resistance, secret redaction, credential rotation/revocation, and fail-closed reconnect/reconciliation.
- This is a documentation-only contract slice. It does not claim authenticated transport, socket/EA implementation, broker compatibility, or live execution. No executable tests are applicable to the document itself; the next transport code change must add the listed deterministic authentication and reconnect tests and pass full CI.


## Work log — QOS-014 mTLS transport boundary (2026-10-27)

- Confirmed PR #21 merged at `770bf46918313f7bfca0265efabd07dfb735b7da`.
- Added `crates/intent-bridge/src/mt5_transport.rs`: TLS 1.3-only server configuration, required client certificates validated against a configured CA, exact leaf-certificate SHA-256 fingerprint allow-list, explicit terminal/server/account principal mapping, and authenticated NDJSON read/write boundary.
- The first application frame must be a heartbeat matching the authenticated principal. Identity/order violations fail closed. Successful writes are not interpreted as venue acceptance or fills.
- Added direct dependencies for rustls, tokio-rustls, rustls-pemfile and sha2; exported the transport types from `intent-bridge`.
- Updated the transport security contract and task queue. This code is a transport boundary, not end-to-end MT5 dispatch. It has no EA/broker integration, protected runtime config/rotation flow, connection timeouts/resource limits, durable reconciliation orchestration or live execution.
- Verification: GitHub Actions is triggered for the PR branch; results must be checked against the final head before claiming success. No local test execution is claimed.


## Work log — QOS-014 transport hardening (2026-10-27)

- Added `Mt5TransportLimits` with positive handshake, I/O and heartbeat timeout settings plus a maximum concurrent authenticated connection count (defaults: 5s, 15s, 30s and 64 connections).
- TLS handshakes and socket accept are bounded; reads/writes time out and poison the connection on failure. A held semaphore permit bounds authenticated concurrent connections and is released when the connection is dropped.
- Added `check_heartbeat_fresh` to fail closed when the configured heartbeat age is exceeded. This is a watchdog API, not a background timer: the host runtime must call it regularly and must degrade the associated session/reconciliation state when it fails.
- Added unit tests for zero/invalid timeout and connection-capacity settings. Existing NDJSON frame size limit and exact certificate fingerprint allow-list remain in force.
- Verification is pending the current branch CI run. No local execution is claimed. End-to-end TLS certificate fixtures/handshake tests, client identity mismatch tests, automatic watchdog orchestration, dispatch/reconciliation wiring, MT5 EA conformance and restart integration remain open; no live trading is enabled.


## Work log — QOS-014 transport conformance follow-up (2026-10-27)

- PR #23 is merged at `7b47069623bb55d2994298d1e8eb9573f964bdb4`; merge-commit CI run `40457087734` passed both Rust and frontend jobs.
- Code review after merge found the outbound writer encoded a literal backslash-plus-n sequence instead of an actual LF byte, despite inbound framing being NDJSON. This follow-up centralizes frame encoding in `encode_frame_line`, appends exactly one LF delimiter, and tests the exact delimiter plus JSON round-trip.
- Added deterministic heartbeat freshness tests for the timeout boundary, expired heartbeat and future timestamps. A truncated/incomplete EOF now poisons the connection before returning an error.
- Verification for this follow-up is pending CI on the current PR head. No local test execution is claimed.
- Still not complete: test-certificate TLS integration tests for trusted/untrusted/mismatched peers; runtime heartbeat watchdog scheduling and session degradation; durable dispatch and sequence/reconciliation orchestration; actual MT5 EA/client conformance; protected credential loading/rotation; end-to-end restart recovery. No live broker is connected and no live trading is enabled.


## Work log — QOS-014 heartbeat expiry degrades session (2026-10-27)

- Added `check_heartbeat_fresh_with_session` to the authenticated MT5 connection. When heartbeat freshness fails, it poisons the connection and degrades the supplied `Mt5SessionGate`, so stale transport health cannot leave the session gate apparently ready.
- Added a deterministic test for expired heartbeat causing session degradation and a fresh heartbeat leaving the session state unchanged.
- The API is invoked by a host-managed watchdog; it is not an autonomous timer. Runtime scheduling, reconnect reconciliation, certificate handshake integration tests, durable dispatch wiring, and MT5 EA conformance remain open. No live trading enabled.

## Work log — QOS-014 runtime, transport conformance and recovery integration (2026-10-27)

- PR #26 merged on 2026-10-27 at `3f7c3a213e7777ccae2f723aca01d496afac2ab6`; no live trading is enabled.
- Added generated-certificate end-to-end mTLS tests for a trusted peer, an untrusted client certificate, a trusted certificate with a mismatched fingerprint, and heartbeat identity mismatches for terminal/server/account.
- Added `spawn_heartbeat_watchdog`, which runs on a Tokio interval, poisons the transport on missing/expired/future heartbeat, degrades the session, and marks the known durable stream degraded. Added an integration test with a short heartbeat deadline.
- Added `read_frame_with_session_checkpoint`: heartbeat identity is checked before durable writes; then the sequence is persisted and the session gate consumes the durable observation/readiness result. Transport/runtime errors degrade the session and durable stream.
- Added `Mt5BridgeRuntime` to coordinate the authenticated connection, session mutex, supervised watchdog, checkpointed reads, authoritative-state reconciliation, and durable one-shot submit dispatch.
- Added `Mt5DispatchPermit::execute_over_transport`: it writes through the authenticated transport once, correlates and persists the response, and consumes the permit. Transport ambiguity is persisted as `Unknown`, degrades the session, and never authorizes a retry.
- Added typed `Mt5ReconciliationSnapshot` models for account state, open orders and positions; validation rejects stale/future snapshots, invalid decimal strings, non-positive quantities, duplicate venue IDs, and session identity mismatch. PostgreSQL migration `20261027000003_mt5_reconciliation_snapshots.sql` stores the snapshot append-only and atomically advances durable stream readiness/checkpoint under a transaction lock.
- Added database-backed tests for snapshot completeness, persistence across a new store handle/process-restart simulation, stale checkpoint rejection, session readiness restoration, and identity mismatch fail-closed behavior. Added a full local TLS + PostgreSQL dispatch test that sends one order and verifies the accepted outcome survives replay as an existing durable key.
- CI has encountered formatting-only failures while the branch was being assembled; those are being corrected and CI must pass on the final head before this slice is verified. Do not treat earlier green frontend jobs or intermediate runs as final verification.
- **Still not complete for live use:** the repository has no production host daemon invoking this coordinator, no MT5 Expert Advisor/client conformance implementation, no broker-specific symbol/volume/price mapping, and no authoritative snapshot adapter querying actual live account/order/position state. Tests use generated local certificates and controlled snapshot fixtures. Protected credential provisioning/rotation and deployment review also remain open.


## QOS-015A — read-only Python MT5 worker (2026-10-27)

- Started branch `feat/qos-015a-python-mt5-readonly` from merged main `3f7c3a213e7777ccae2f723aca01d496afac2ab6`.
- Added a Windows-only optional MetaTrader5 dependency, environment-driven configuration, TLS 1.3 mutual-authenticated client transport, persisted stream sequence, account identity checks, and read-only account/open-order/position snapshot generation.
- Added an additive `reconciliation_snapshot` wire payload and Rust mapping into the existing durable reconciliation snapshot path. The session gate permits read-only reconciliation while keeping order submission/cancellation disabled by advertised capabilities.
- Added Python unit tests and CI job plus Rust serialization/session regressions. Verification is pending on the PR head; no Windows terminal smoke test has run in this environment. No live or demo orders are submitted by this worker.


## Work log — QOS-015A daemon host wiring (2026-10-27)
- Follow-up branch `feat/qos-015a-mt5-bridge-daemon` created from current `main` after PR #27 was confirmed merged.
- Added `mt5-bridge-daemon`, which loads mTLS certificate files and a single explicit certificate-fingerprint/terminal/server/account principal, defaults to loopback binding, requires an explicit override for non-loopback binds, connects to PostgreSQL, creates the existing `Mt5BridgeRuntime`, persists read-only reconciliation snapshots, and discards failed connections so the worker must reconnect and reconcile.
- Added daemon environment/setup documentation. The daemon intentionally exposes no order-submit/cancel command and does not run migrations automatically.
- GitHub Actions run `40499291725` passed Rust format, workspace check, workspace tests, Clippy, Python adapter contract tests, and frontend lint/typecheck/build on code commit `bac103c8986408356fec37a4a35f69984c97ab99`. The Windows demo-terminal smoke test, protected certificate lifecycle, and instrument metadata/fresh quote integration remain release gates.


## Work log — QOS-015B market data feed (2026-10-27)
- Started after PR #28 was confirmed merged. Branch: `feat/qos-015b-mt5-market-data`.
- Added a typed `market_data_snapshot` protocol payload for broker instrument specifications and paired bid/ask ticks, with strict decimal, identity, symbol-match, volume-constraint, and 60-second quote-freshness validation.
- Updated the Windows worker to require `MT5_SYMBOLS` (1–64 exact symbols), query symbol metadata and current ticks from the official MetaTrader5 API, and emit the market-data frame alongside heartbeat/reconciliation frames. The worker remains read-only.
- Added PostgreSQL migration `20261027000004_mt5_market_data_snapshots.sql`, persistence API, and runtime handling that stores market-data snapshots separately from reconciliation readiness.
- Added schema, Python contract tests, Rust validation tests, and deployment/protocol docs. CI is pending; Windows demo verification and downstream QOS-013 sizing integration remain outstanding.


## Work log — QOS-013 persisted MT5 runtime consumer (2026-10-27)
- Started `feat/qos-013-mt5-runtime-integration` after PR #30 merged.
- Added `Mt5PersistedSizingProvider` to load the exact account/terminal snapshot, verify indexed server/currency/sequence/capture metadata, map Quant OS instrument IDs to exact broker symbols, and require a dynamic trusted supplement provider for costs, margin and pending-order reserves.
- Added side-aware provider loading; buy intents use ask and sell intents use bid. The resulting entry price is now used both by the risk evaluator and the downstream order request, avoiding sizing at one price and submitting at another.
- Provider fails closed when capability and account-position-semantics gates are not explicitly validated. No order submission/cancellation capability was added; the existing explicit runtime sizing path remains opt-in.
- Added SQLx-backed runtime integration tests for side-specific ask/bid selection and stale trusted-supplement rejection. Exact-head CI is pending. Windows MT5 demo-terminal smoke test and certificate lifecycle review remain outstanding. No live mode enabled.


## Work log — QOS-015A/015B Windows demo smoke harness (2026-10-27)
- Confirmed PR #31 is merged; its sizing runtime consumer is in `main`, but the actual Windows MT5 terminal has not been demonstrated by hosted CI.
- Added `adapters/mt5-python/scripts/mt5-demo-smoke.ps1`: verifies required configuration/cert files, worker leaf certificate DER SHA-256 pin, server/client cert-key pairing, actual MT5 demo login/server/currency, exact configured symbols, fresh bid/ask and broker sizing metadata, daemon listener, PostgreSQL migration presence, and fresh persisted market-data plus reconciliation rows. It requires at least two distinct market-data sequences.
- Added `docs/MT5_DEMO_SMOKE_TEST.md` and linked it from the adapter README. The runbook covers target-host setup, evidence retention, worker restart/reconciliation, and private-key ACL/certificate rotation/revocation review.
- Safety boundary: this is read-only and contains no order submission/modification/cancellation. It cannot prove terminal conformance until the user runs it on the actual Windows VPS. PowerShell execution is not available in this authoring environment; exact-head GitHub CI and target-host execution are pending.


## QOS-015 release evidence gate — 2026-10-27

- PR #35 merged the restart-evidence JSON validator. The merge-triggered main CI run must be checked independently; this record does not assume a passing conclusion.
- PR #36 adds a consolidated release manifest validator and a template that remains BLOCKED by default. It requires attested evidence for actual Windows demo smoke/restart, certificate trust/revocation, key and parent-directory ACL review, rotation/rollback, QOS-015B persistence, QOS-013 sizing, and independent review by someone other than the operator.
- The new validator is a structural gate over operator-provided evidence references. It cannot authenticate referenced files or execute Windows/MT5/certificate lifecycle tests.
- **Not verified here:** access to the target Windows VPS, real demo terminal/account, revocation enforcement, ACL effective access, rotation/rollback, QOS-015B runtime persistence on that host, and QOS-013 trusted sizing inputs/audit in deployment.
- **Current release decision: BLOCKED.** No venue execution is authorized or enabled. Only change this status after the referenced evidence is inspected and the exact commit and manifest digest are independently reviewed.


## Work log — QOS-015A native macOS provider — 2026-10-28
- Read `RULES.md`, the existing MT5 worker/provider seam, QOS-015 macOS conformance requirements, README, tests, and task status before implementation.
- Added `mt5_adapter.macos_socket_provider`, a loopback-only authenticated TCP client for the MQL5 EA, account/server identity checks, configured-symbol allowlisting, and mappings for account state, open orders, positions, instrument metadata, and ticks. The native provider does not require a broker password because login remains in the terminal UI.
- Added `mql5/QuantOSReadOnlyProvider.mq5` with a restricted read-only command surface; no order submission/modification/cancellation calls are included.
- Added Python tests and updated macOS preflight/docs. Initial exact-head CI run 40653817712 passed the standalone MT5 Python contract-test job but failed the macOS preflight because the mismatched-login socket fixture attempted to connect before its listener was ready. Added retry synchronization on the test fixture. Exact-head run 40653937982 then passed Rust format/check/workspace tests/Clippy, frontend lint/typecheck/build, macOS portable preflight, and MT5 Python adapter contract tests. The local clone still cannot resolve GitHub, so no local test run is claimed.
- Verification boundary: EA compilation in MetaEditor, real demo account connectivity, PostgreSQL persistence, worker restart recovery, and certificate lifecycle review remain outstanding. This work is not a QOS-015A conformance PASS. Venue execution remains disabled.


## Work log — Phase 0 baseline and task reconciliation (2026-10-31)

- Re-read `RULES.md`; reviewed `TASKS.md`, `PROGRESS.md`, `ROADMAP.md`, the MT5 protocol/conformance documents, and the current Rust/Python MT5 implementation surfaces.
- Confirmed local `main` and `origin/main` both point to `3e1259063d3194e5b26cf93766ba50306c13fec1`. Preserved the pre-existing uncommitted `Cargo.lock` change; did not include it in the baseline CI claim.
- Verified GitHub Actions run `40858852429` on the exact commit completed successfully across Rust format/check/test/Clippy with PostgreSQL, MT5 Python contract tests, frontend lint/typecheck/build, and macOS portable preflight.
- Reconciled current status: QOS-013/014/015A/015B have implementation foundations, while real terminal identity/data conformance, persisted snapshots and restart recovery, broker-input sizing evidence, protected credential lifecycle, and independent review remain open.
- Agreed implementation sequence is captured in `TASKS.md`; it is execution order only and does not replace product scope or the roadmap in `ROADMAP.md`.
- Updated the current baseline/status sections and corrected stale statements about whether the MT5 authenticated transport, read-only daemon, and macOS provider exist in the task queue, adapter README, and MT5 protocol/security/conformance docs. Historical work-log entries remain as historical records.
- No trading/runtime code changed. No actual MT5 terminal test or host security review was performed. No live execution was enabled.
