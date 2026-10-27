# Quant OS — Correction and Feedback Register
Last updated: 2026-10-27. Every correction, real-use failure, test mismatch, or rule violation must be logged here. See `RULES.md`.

## Required fields
Date/context; finding and impact; evidence; failed/missing rule; rule update; artifacts/code/tests changed; regression or real-use check; verification result; remaining risk. Never invent a correction or claim a real-use test happened when it did not.

## REV-000 — Documentation consistency review
- **Date:** 2026-10-27
- **Context:** Creating the product roadmap and operating docs.
- **Finding:** The product blueprint includes future capabilities while repository docs describe some paths as paper-only, mock-first, deferred, or not implemented.
- **Impact:** Without status labels, a roadmap item could be mistaken for an existing feature.
- **Evidence:** Repository README, migration phases, venue docs, account docs, and frontend README were inspected through GitHub.
- **Rule strengthened:** `RULES.md` requires source inspection and evidence for claims; docs must distinguish current facts from target design.
- **Artifacts:** PRD, architecture, tasks, progress, roadmap.
- **Regression check:** QOS-001 compares docs to source and records exact test commands/results.
- **Verification:** Initial branch creation attempt returned HTTP 403. Repository access later became available and the working branch was created successfully; this historical entry is retained for audit.
- **Remaining risk:** Exact source implementation and test state still need verification.

## REV-001 — Source audit refined the implementation baseline
- **Date:** 2026-10-27
- **Context:** Initial source inspection for QOS-001.
- **Finding:** The existing `VenueAdapter` has basic request/snapshot methods; runtime intent execution is configured for one account. Inspected sizing does not model all instrument-specific tick/contract/conversion rules. The cancellation bridge marks cancelled after adapter success, requiring adapter-specific semantic verification.
- **Impact:** A proportional-size loop over the current pipeline would be unsafe and incomplete.
- **Evidence:** Read `crates/venue-connectors/src/adapter.rs`, `types.rs`, `bridge.rs`, `crates/trading-runtime/src/pipeline.rs`, `crates/risk-engine/src/sizing.rs`, account model/service, and `Cargo.toml`.
- **Rule that failed:** No existing implementation rule was proven violated; these are gaps between target requirements and implementation.
- **Rule update:** No weakening/change needed. Financial safety rules 12–15 already require independent account outcomes, instrument-aware sizing, and confirmed cancellation/close state.
- **Regression checks:** QOS-010 specifies capabilities/events/instrument specs/health; QOS-012/QOS-013 cover contract/tick values, conversions, volume increments, and per-account gating; cancellation tests distinguish request acknowledgement from terminal cancellation.
- **Verification:** Source inspection completed. Rust tests not run.
- **Remaining risk:** Complete source review and test execution required before code changes.

## REV-002 — Repository access restored and docs branch created
- **Date:** 2026-10-27
- **Context:** User confirmed repository access is now available.
- **Finding/correction:** Prior progress notes that said branch creation was blocked became stale after access was restored.
- **Rule that failed:** Progress accuracy / status freshness.
- **Rule update:** Existing workflow rules 6–7 require verified progress and accurate claims.
- **Artifacts changed:** Update this register and `PROGRESS.md`; add all docs on the feature branch.
- **Regression check:** Verify branch/file URLs and PR creation response.
- **Verification:** Branch `docs/quant-os-product-operating-system` created; all eight documentation files verified on branch; draft PR #1 opened at https://github.com/apostleoffinance/event-driven-trading-rs/pull/1.
- **Remaining risk:** PR is unmerged and awaiting review; Rust tests remain unrun.

## Entry template
### REV-XXX — Short title
- **Date:**
- **Context:**
- **Finding/correction:**
- **Impact:**
- **Evidence:**
- **Rule that failed:**
- **Rule update made in RULES.md:**
- **Code/docs/tests changed:**
- **Regression/real-use check added:**
- **Verification result:**
- **Remaining risk:**


## REV-003 — QOS-001 baseline audit completed after docs merge
- **Date:** 2026-10-27
- **Context:** User confirmed documentation PR #1 was merged and requested QOS-001 implementation.
- **Finding/correction:** REV-002's “PR unmerged and awaiting review” described the earlier state and is superseded. Current `main` is merge commit `14619a758ac9864edb52958195de57fb5ca4b685`. Source inspection confirms paper/simulated execution and a mock-first frontend; it does not establish live venue execution.
- **Impact:** Without a source-backed baseline, future implementation could confuse mock events/public market data with live execution, or overstate current multi-account and instrument sizing support.
- **Evidence:** `BASELINE_AUDIT.md`; GitHub tree and metadata; inspected Cargo workspace, venue adapters/implementations, runtime pipeline/config, sizing, account model/service, intent bridge, and frontend source. GitHub Actions API returned zero workflow runs.
- **Rule that failed:** No engineering safety rule was shown to have been violated. This is a status correction and implementation-gap record.
- **Rule update made in RULES.md:** None; existing rules already require source-backed claims, explicit mock/paper/live labels, fail-closed risk, and confirmed venue outcomes.
- **Code/docs/tests changed:** Added `BASELINE_AUDIT.md`; updated `TASKS.md` and `PROGRESS.md`. No trading code changed.
- **Regression/real-use check added:** QOS-002 retains executable formatting/check/test/clippy commands; QOS-003 retains frontend script/dependency/token checks. Venue/cancel semantics and multi-account failure injection remain explicit future test requirements.
- **Verification result:** QOS-001 source audit acceptance met. Local clone failed with `Could not resolve host: github.com`; Rust, Python and frontend checks were not run and are not claimed to pass.
- **Remaining risk:** Compilation, test behavior, frontend build/lint/typecheck, database migrations, and live venue readiness remain unverified. Do not enable live execution based on this audit.


## REV-004 — Initial CI run caught Rust formatting drift
- **Date:** 2026-10-27
- **Context:** QOS-002 executable baseline checks via PR #3, run 40425100977.
- **Finding/correction:** The new CI job successfully initialized PostgreSQL and Rust, then failed at `cargo fmt --all -- --check`. The exact diff identified formatting drift in `crates/domain/src/instrument.rs`, `crates/events/src/market_data_health.rs`, `crates/execution-engine/src/oms.rs`, `crates/strategy-runtime/src/worker.rs`, and `crates/venue-connectors/tests/ambiguous_unknown.rs`. The workspace check, tests, and Clippy were skipped because formatting failed first. Frontend lint, typecheck, and production build all passed in the same run.
- **Impact:** The initial CI workflow did not yet provide compilation/test evidence; source behavior remained unverified.
- **Evidence:** GitHub Actions run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40425100977; Rust job https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40425100977/job/121928766731.
- **Rule that failed:** No substantive coding/safety rule was shown to be violated; repository formatting was not compliant with the declared CI command.
- **Rule update made in RULES.md:** None. Existing rules require formatting/check/test/clippy evidence and minimal reversible changes.
- **Code/docs/tests changed:** Applied only the rustfmt-diff changes to the five files above. No behavior changes intended. CI workflow itself remains the same.
- **Regression/real-use check added:** Re-run CI so formatting, workspace compile, tests, and Clippy can execute sequentially.
- **Verification result:** Format-only edits were committed to branch `audit/qos-002-rust-checks`; post-fix CI result is pending.
- **Remaining risk:** Compilation, workspace tests, and Clippy have not yet run successfully or been verified. No live trading or money movement enabled.


## REV-005 — QOS-002 CI verification passed after format correction
- **Date:** 2026-10-27
- **Context:** Follow-up to REV-004, PR #3 latest-head CI run 40425656667.
- **Finding/correction:** Formatting-only corrections resolved the initial `cargo fmt --all -- --check` failure. The subsequent complete Rust and frontend CI jobs succeeded.
- **Impact:** First automated evidence now exists for the current PR head, including compilation, tests, and linting. This does not prove live-venue readiness or financial correctness beyond the covered checks.
- **Evidence:** GitHub Actions run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40425656667 completed with conclusion `success`. The Rust job reports success for `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets`. The frontend job reports success for `npm ci`, `npm run lint`, `npm run typecheck`, and `npm run build`.
- **Rule that failed:** No safety rule failed. The initial formatting mismatch was corrected under REV-004.
- **Rule update made in RULES.md:** None.
- **Code/docs/tests changed:** Format-only edits in the five Rust files listed in REV-004; task/progress/review records updated. CI workflow unchanged.
- **Regression/real-use check added:** CI workflow continues to run the Rust and frontend baseline commands on PRs to main and pushes to main.
- **Verification result:** Both CI jobs passed on the latest PR head `2786b355cd08c4d9fc61dc4653c583646d4f00ae`, run 40425656667. QOS-002 acceptance criteria met by GitHub Actions evidence.
- **Remaining risk:** CI passing is not proof of production safety, venue integrations, live execution, or money movement readiness. PR #3 has not been merged.


## REV-006 — QOS-003 frontend baseline audit
- **Date:** 2026-10-27
- **Context:** PR #3 merged; QOS-003 selected as the next task.
- **Finding/correction:** Source inspection confirmed mock repositories remain the only data source even when API mode is requested; the environment label can independently be set to LIVE; the top bar displays SYSTEM HEALTHY unconditionally; illustrative mock system telemetry contains old timestamps; and the current blue/dark token palette differs from the white-dominant navy/orange direction.
- **Impact:** A visual refresh without first defining truthful state semantics could make simulated data or disconnected services appear live/healthy.
- **Evidence:** `FRONTEND_AUDIT.md`; inspected frontend package/config, repositories, mock data, event/simulation providers, top bar, theme provider, CSS tokens, and dashboard.
- **Rule that failed:** No financial-safety rule was shown to have been violated by the audit itself. Existing frontend rules already prohibit presenting mock as live, fake status, and unsupported health claims; the inspected implementation exposes risk areas against those rules.
- **Rule update made in RULES.md:** None; current rules are adequate.
- **Code/docs/tests changed:** Added `FRONTEND_AUDIT.md`; updated `TASKS.md`, `PROGRESS.md`, and this register. No UI/runtime/dependency/trading code changed.
- **Regression/real-use check added:** Future UI work must test explicit mock/API mode, LIVE label disconnected from backend, stale/disconnected/unknown health, and ensure system status is never hardcoded as healthy.
- **Verification result:** QOS-003 source-inspection acceptance met. CI run 40425954540 completed successfully for Rust and frontend baseline commands.
- **Remaining risk:** No browser usability test, live API handshake, production deployment, or live venue integration was performed. Do not treat this audit or green CI as proof of live readiness.


## REV-007 — QOS-010 venue adapter contract specification
- **Date:** 2026-10-27
- **Context:** PR #4 merged; QOS-010 selected as the next scoped task.
- **Finding/correction:** The current adapter exposes basic submit/cancel and account/order/position queries, but no explicit capability discovery, instrument-spec lookup, health/readiness, async event stream, or complete reconciliation contract. Existing error types already distinguish ambiguous outcomes; the cancel bridge currently marks cancellation terminal after adapter call success, which is not a safe general venue contract.
- **Impact:** Implementing a real venue adapter before agreeing these semantics risks duplicate submits, false cancellation claims, stale-state trading, and unreconciled state after reconnect.
- **Evidence:** Inspected `crates/venue-connectors/src/adapter.rs`, `types.rs`, `error.rs`, `simulated.rs`, `crates/domain/src/instrument.rs`, `crates/events/src/lib.rs`, `crates/venue-connectors/src/bridge.rs`, and `ARCHITECTURE.md` on main at merge commit `e92e4e2c4fb9e5f3792c353397a9d1e536eec0a3`.
- **Rule that failed:** No rule violation was proven by this specification task. These are gaps between the current API and the required target contract.
- **Rule update made in RULES.md:** None; current financial-safety, idempotency, stale-data, and confirmation rules already cover the risks.
- **Code/docs/tests changed:** Added `VENUE_ADAPTER_CONTRACT.md`; updated `TASKS.md`, `PROGRESS.md`, and this register. No code or tests changed.
- **Regression/real-use check added:** Added contract-test scenarios VA-01 through VA-18 for capabilities, identity, duplicate/conflicting client IDs, pre/post-send timeout, reconciliation, cancellation confirmation, partial fills, duplicate/out-of-order events, stale specs, precision/volume validation, pagination, discrepancies, rate limits, and restart recovery.
- **Verification result:** Source inspection and documentation consistency completed. No executable tests were run because the change is documentation-only; PR CI must verify repository workflow before merge.
- **Remaining risk:** Contract and acceptance matrix need review before QOS-011 implementation. No provider-specific behavior, paper run, live adapter, live execution, or money movement was tested or enabled.


## REV-008 — QOS-012 instrument-aware sizing specification
- **Date:** 2026-10-27
- **Context:** PR #5 merged; the next planned task was QOS-012.
- **Finding/correction:** Existing `size_order` derives trade-risk quantity from equity risk budget divided by absolute price-stop distance, then caps by position/exposure/leverage. `RiskRequest` lacks instrument loss mechanics, account-currency conversion, current-data freshness, margin schedule, and pending-order reservations; `InstrumentSpec` lacks max volume and contract/tick/pip value metadata. This is insufficient to prove correct FX/CFD sizing across accounts.
- **Impact:** Applying the existing price-distance formula universally can materially mis-size risk when contract units, tick values, quote currency, or account currency differ; rounding up to venue minimum could exceed risk budget.
- **Evidence:** Inspected `crates/risk-engine/src/sizing.rs`, `crates/domain/src/risk.rs`, `crates/domain/src/instrument.rs`, `crates/risk-engine/src/lib.rs`, and merged `VENUE_ADAPTER_CONTRACT.md` on main at `f3f07c6387f1e124ccfa250b1961f3a3e42de032`.
- **Rule that failed:** No safety rule violation was proven by writing this specification. The source shows model/input gaps against existing instrument-aware sizing and fail-closed requirements.
- **Rule update made in RULES.md:** None; existing financial-safety rules already require instrument-aware risk budgets, exact decimals, stale-data blocking, and no unsupported guarantees.
- **Code/docs/tests changed:** Added `INSTRUMENT_AWARE_SIZING.md`; updated `TASKS.md`, `PROGRESS.md`, and this register. No implementation or tests changed.
- **Regression/real-use check added:** Defined SZ-01 through SZ-26 covering stop geometry, tick value, FX conversion, nonlinear models, costs, volume-step rounding, min/max volume, exposure/margin, pending orders, multi-account differences, stale state, decimal invariants, kill switches, and property tests.
- **Verification result:** Source-backed specification drafted and repository task records updated on branch `spec/qos-012-instrument-aware-sizing`. No executable tests run because the change is documentation-only; PR CI remains pending.
- **Remaining risk:** Specification must be reviewed before QOS-013 implementation. Existing sizing behavior remains unchanged; no live venue, live execution, or money movement was enabled.


## REV-009 — QOS-011 idempotency and cancellation confirmation hardening
- **Date:** 2026-10-27
- **Context:** QOS-011 started after QOS-010 adapter contract and QOS-012 sizing specification merged.
- **Finding/correction:** Both paper connectors previously returned the existing order for any repeated client order ID without checking the new payload; a concurrent duplicate submit could pass the initial lookup before either order was inserted. The cancel bridge also marked an order Cancelled after the cancel method returned success, without querying terminal venue state.
- **Impact:** Reusing a client ID for different order intent could silently report the wrong order outcome, concurrent requests could create duplicate simulated effects, and a cancel acknowledgement could be mistaken for confirmed cancellation.
- **Evidence:** Inspected `crates/venue-connectors/src/simulated.rs`, `prop.rs`, `types.rs`, `error.rs`, `bridge.rs`, `crates/domain/src/order.rs`, `crates/execution-engine/src/oms.rs`, and `VENUE_ADAPTER_CONTRACT.md` on main at `a39fd9d9a1bc0bb0b4fb8f7a09737a8b09aa1bf0`.
- **Rule that failed:** Existing financial-safety rules 7 (same client ID with changed payload is a conflict) and 15 (cancel requests are not success until external confirmation) were not consistently enforced by the connector/bridge implementation.
- **Rule update made in RULES.md:** None; existing rules are sufficient.
- **Code/docs/tests changed:** Added `ClientOrderIdConflict` and immutable payload comparison; updated simulated and prop idempotency paths with a lock-protected second check; hardened cancel bridge confirmation; added replay/conflict simulator tests and cancel-ack-but-open regression test; updated task/progress records.
- **Regression/real-use check added:** Unit/integration tests exercise same-payload replay, changed-payload conflict without extra position exposure, and a cancel acknowledgement where the venue still reports the order open. Existing ambiguous submit test continues to assert Unknown rather than Failed.
- **Verification result:** Code and tests committed to feature branch; CI pending. No local test execution claimed.
- **Remaining risk:** CI must verify formatting, compilation, tests, and Clippy. Adapter event-stream deduplication/reconnect remains unimplemented because no event-stream surface currently exists; live venue readiness and provider-specific cancellation semantics remain unverified.

## REV-010 — QOS-011 CI formatting failure and correction
- **Date:** 2026-10-27
- **Context:** PR #7 initial-head CI run 40432165184, Rust job 121952211529.
- **Finding/correction:** `cargo fmt --all -- --check` failed on formatting-only drift in the new replay-fill collection expressions and added tests in `crates/venue-connectors/src/prop.rs`, `crates/venue-connectors/src/simulated.rs`, and `crates/venue-connectors/tests/cancel_confirmation.rs`. The workspace check, tests, and Clippy were skipped because formatting is the first Rust CI gate. The frontend job passed.
- **Impact:** The first run did not compile or test the Rust changes, so it supplied no test evidence for QOS-011.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40432165184/job/121952211529; failed step: “Check formatting”.
- **Rule that failed:** Repository formatting did not satisfy the declared CI command; no financial-safety behavior rule was shown to have failed by this CI result.
- **Rule update made in RULES.md:** None; existing rules already require exact command/result reporting.
- **Code/docs/tests changed:** Applied rustfmt-equivalent formatting to the reported expressions/imports/assertions on `fix/qos-011-venue-contract`. No intended behavior change.
- **Regression/real-use check added:** Latest-head CI must rerun formatting, workspace check, workspace tests, and Clippy; the existing frontend check remains green on run 40432165184.
- **Verification result:** Formatting corrections pushed to the PR branch. New-head CI result pending; no Rust compile/test/Clippy success claimed.
- **Remaining risk:** The corrected head must pass the complete Rust job. Event-stream deduplication/reconnect remains unimplemented until `VenueAdapter` has an event-stream API; live venue behavior remains unverified.


## REV-011 — Latest-head CI found one remaining rustfmt diff
- **Date:** 2026-10-27
- **Context:** PR #7 Rust job in run 40433534897, job 121956683322.
- **Finding/correction:** The first formatting correction fixed all reported differences except the second position assertion in `crates/venue-connectors/src/simulated.rs`. The formatting check continued to fail, so Rust workspace check, tests, and Clippy were skipped.
- **Impact:** The implementation still had no compile/test evidence at that point. The frontend lint and typecheck passed, but production build also failed in `src/app/layout.tsx` inside Next.js `next/font` with `TypeError: Cannot read properties of null (reading '1')`; this is an external build-path issue to diagnose separately.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40433534897; Rust formatting log around `simulated.rs:505`; frontend build log reports Next.js 15.5.7 and failure in `next/font`.
- **Rule that failed:** The checked-in Rust formatting did not comply with `cargo fmt --all -- --check`; frontend production build did not complete.
- **Rule update made in RULES.md:** None; safety rules remain unchanged.
- **Code/docs/tests changed:** Corrected the remaining simulator-test formatting on the same feature branch. No behavioral code change intended.
- **Regression/real-use check added:** Await the newest-head Rust formatting/check/test/Clippy and frontend build result. Determine whether `next/font` failure is stable or runner/network-dependent before changing font loading.
- **Verification result:** Final rustfmt diff identified in this run was pushed as commit 93e919c5c55c2df8bc8aa8eaa724a96fc1ed609e. Latest-head CI pending.
- **Remaining risk:** Rust tests have not yet run successfully; production frontend build requires diagnosis. QOS-011 remains IN PROGRESS.

## REV-012 — QOS-011 compile failure: conflict error variant missing
- **Date:** 2026-10-27
- **Context:** PR #7 run 40433693778, Rust job 121957204050.
- **Finding/correction:** After formatting passed, `cargo check --workspace` failed because four references to `VenueError::ClientOrderIdConflict` could not resolve: the typed variant was absent in the branch's current `crates/venue-connectors/src/error.rs`. Restored the variant with an explanatory typed error message.
- **Impact:** Rust compilation failed before workspace tests and Clippy could run. The frontend build passed in the same CI run after an earlier intermittent `next/font` failure; no frontend code change was made.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40433693778; Rust check reported E0599 “no variant ... named `ClientOrderIdConflict`”.
- **Rule that failed:** The feature implementation referenced an error variant not present in the checked-in source state; existing workflow rule 2 (inspect source) and rule 7 (claim only verified state) apply.
- **Rule update made in RULES.md:** None; existing rules are sufficient.
- **Code/docs/tests changed:** Restored `ClientOrderIdConflict(String)` in `VenueError` on `fix/qos-011-venue-contract`.
- **Regression/real-use check added:** Latest-head CI must rerun workspace compile, tests, and Clippy after formatting.
- **Verification result:** Typed error variant update committed as `af8e29acc8f5127a8fd8b8ec6b6d2293ae4bbca7`; awaiting CI evidence.
- **Remaining risk:** Compile/tests/Clippy not yet verified on the corrected head. Adapter event-stream deduplication remains outside this slice; no live execution enabled.

## REV-013 — QOS-011 full CI verification passed
- **Date:** 2026-10-27
- **Context:** PR #7 code head `fc36d8bd598e93d231929cc2c10fe34663678de5` passed run 40433848539; latest documentation head `eb4c4d388e1160c25e6f757766731764ddb76809` also passed run 40434017979.
- **Finding/correction:** After the formatting and missing `ClientOrderIdConflict` corrections, the latest-head Rust and frontend jobs completed successfully.
- **Impact:** The PR now has CI evidence for formatting, compilation, workspace tests, Clippy, frontend lint, typecheck, and production build. This is not evidence of live-venue readiness or production financial correctness.
- **Evidence:** Code-head run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40433848539 and latest-head run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40434017979. Both run's Rust steps `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets` passed. Latest documentation-head run 40434017979 completed with conclusion `success`; frontend `npm ci`, `npm run lint`, `npm run typecheck`, and `npm run build` all passed there too.
- **Rule that failed:** No new rule failure in the successful run; earlier failures and corrections are recorded in REV-010–REV-012.
- **Rule update made in RULES.md:** None.
- **Code/docs/tests changed:** Updated task/progress/review records only. No additional runtime changes.
- **Regression/real-use check added:** No new test added in this verification step; existing replay/conflict and cancel-confirmation regressions are included in the passing workspace test suite.
- **Verification result:** Both required CI jobs completed with conclusion `success` for the code head and again on the latest docs-only head `eb4c4d388e1160c25e6f757766731764ddb76809` in run 40434017979.
- **Remaining risk:** PR #7 remains open and unmerged pending user review. Venue event-stream deduplication/reconnect remains unimplemented because the current adapter trait has no event-stream API; no live adapter, live execution, deployment, or money movement was enabled.

## REV-014 — Latest PR-head CI reconfirmed after task-summary correction
- **Date:** 2026-10-27
- **Context:** The task summary was reconciled in commit `8b413f5127e906650f8621b84543b5bb4ee0741d`; GitHub Actions run 40434342385 checks the resulting PR head.
- **Finding/correction:** Both required CI jobs completed successfully again on the actual latest PR head.
- **Impact:** The task-summary correction itself is covered by the successful CI run; the PR does not have a known failing check on its current head.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40434342385 completed with conclusion `success`. Rust job passed `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets`; frontend job passed `npm ci`, `npm run lint`, `npm run typecheck`, and `npm run build`.
- **Rule that failed:** None; this is final evidence reconciliation after the earlier documented failures.
- **Rule update made in RULES.md:** None.
- **Code/docs/tests changed:** Review register only.
- **Regression/real-use check added:** None; the full CI matrix reran on the latest PR head.
- **Verification result:** Latest PR head `8b413f5127e906650f8621b84543b5bb4ee0741d`, run 40434342385, all checks green.
- **Remaining risk:** PR #7 is open/unmerged. Venue event-stream deduplication/reconnect remains out of scope until the adapter exposes an event stream; no live execution or money movement enabled.


## REV-015 — QOS-011 merge status corrected; QOS-013 implementation plan audited
- **Date:** 2026-10-27
- **Context:** Follow-up after the user confirmed PR #7 had merged and requested the recommended next steps.
- **Finding/correction:** `TASKS.md` and `PROGRESS.md` still said PR #7 was open/unmerged despite the actual GitHub PR metadata reporting `merged: true` and merge commit `324d327f035ca5ccb573404cc8e446b816990ada`. Historical work-log entries remain as historical records, but the current summaries were stale. The existing sizing implementation also remains materially narrower than the QOS-012 specification: it uses absolute price-stop distance as a quantity cap and has no instrument tick-value model, currency conversion, freshness snapshots, venue volume-step/min/max validation, margin inputs, or pending-order reserves in `RiskRequest`.
- **Impact:** Stale task status misleads sequencing, and implementing a universal sizing formula from the current request fields could misstate risk across instrument/account currencies.
- **Evidence:** GitHub PR #7 metadata reports merged at `2026-10-27T01:48:38Z`, merge commit `324d327f035ca5ccb573404cc8e446b816990ada`. Source audit: `crates/risk-engine/src/sizing.rs`, `crates/risk-engine/src/evaluator.rs`, `crates/domain/src/risk.rs`, `crates/domain/src/instrument.rs`, `crates/domain/src/account.rs`, `crates/trading-runtime/src/pipeline.rs`, and `INSTRUMENT_AWARE_SIZING.md` on `main`.
- **Rule that failed:** Progress accuracy (RULES.md Workflow 6) was not maintained after merge; the financial model gap is a known implementation limitation against Financial Safety 13 and stale/missing-data fail-closed requirements.
- **Rule update made in RULES.md:** None; existing rules already cover verified status and instrument-aware sizing.
- **Code/docs/tests changed:** Corrected current QOS-011/QOS-013 status in `TASKS.md` and `PROGRESS.md`; added `QOS-013_IMPLEMENTATION_PLAN.md`; appended this review entry. No runtime code changed.
- **Regression/real-use check added:** QOS-013 plan defines deterministic unit/integration acceptance criteria, including stop geometry, unsupported/missing instrument inputs, decimal rounding, fail-closed data requirements, and the risk-to-OMS boundary. These tests are planned, not yet implemented or run.
- **Verification result:** PR #7 merge status verified through GitHub PR metadata; source inspection completed. No tests/checks were run because this PR is documentation-only. This is not a claim that the QOS-013 implementation is complete or safe for live execution.
- **Remaining risk:** Current sizing implementation still lacks the required typed instrument, conversion, freshness, margin, and venue constraints. QOS-013 implementation must remain paper/simulation-only and must not mark unsupported instrument models executable.


## REV-016 — QOS-013 first instrument-aware sizing implementation slice
- **Date:** 2026-10-27
- **Context:** User requested implementation of QOS-013 after the QOS-011 merge-status correction and sizing-plan review.
- **Finding:** Existing sizing relied on absolute entry/stop price distance and did not encode explicit instrument loss mechanics. A new opt-in API is needed until authoritative sizing snapshots are wired into the runtime without unsafe defaults.
- **Impact:** A price-distance-only calculation is not sufficient for instruments with contract multipliers, tick values, or differing account currency. Missing/stale sizing inputs must not yield an executable size through the new API.
- **Evidence:** Current source audit in `QOS-013_IMPLEMENTATION_PLAN.md`; implementation in `crates/risk-engine/src/instrument_sizing.rs` and `crates/risk-engine/src/evaluator.rs`.
- **Rule that failed:** No new rule violation is asserted; this work addresses gaps against Financial Safety rules 4, 5, and 13 and Rust/testing rules.
- **Rule update made in RULES.md:** None; existing rules are adequate.
- **Code/docs/tests changed:** Added explicit linear and tick-value loss models, conversion rates, freshness state, conservative step rounding, risk/position/exposure/leverage caps, minimum/maximum/minimum-notional checks, post-rounding modeled-loss verification, and an evaluator entry point that runs risk gates before sizing. Added eight focused tests and updated task/progress/plan docs.
- **Regression/real-use check added:** Tests cover linear sizing and risk-budget resize, tick-value conversion, buy/sell stop geometry, stale-input rejection, off-tick stops, below-minimum quantity, costs/buffer effects, and kill-switch precedence. CI pending at time of entry; no real venue test was run.
- **Verification result:** Files were written to the feature branch through GitHub. Compilation and test success are not yet claimed; CI must verify the current head.
- **Remaining risk:** The new context is opt-in. Existing `RiskEvaluator::evaluate` and runtime do not yet source authoritative venue specs/conversion/freshness by default. Pending-order reserves, margin/free-funds caps, account-specific freshness thresholds, structured persisted audit, property tests, and full SZ-01–SZ-26 coverage remain incomplete. No live execution enabled.

## REV-017 — QOS-013 initial CI formatting failure and correction
- **Date:** 2026-10-27
- **Context:** First QOS-013 PR CI run 40435500152 on the initial implementation head.
- **Finding/correction:** `cargo fmt --all -- --check` failed on formatting-only differences in `crates/risk-engine/src/instrument_sizing.rs` and module ordering in `crates/risk-engine/src/lib.rs`. Workspace check, tests, and Clippy were skipped by the sequential workflow.
- **Impact:** No compile/test evidence was available from that run.
- **Evidence:** GitHub Actions run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40435500152; Rust job 121963077119 reported failure at formatting step 6. Logs showed the exact rustfmt diffs.
- **Rule that failed:** The code did not yet comply with the repository's required `cargo fmt --all -- --check` command.
- **Rule update made in RULES.md:** None; existing rules already require formatting and workspace verification.
- **Code/docs/tests changed:** Applied only the rustfmt-indicated line wrapping and module order corrections. No intended behavior change.
- **Regression/real-use check added:** Latest-head CI run must rerun formatting, workspace check, workspace tests, and Clippy.
- **Verification result:** Formatting correction pushed to the feature branch; rerun status pending. No compile/test success claimed from the failed run.
- **Remaining risk:** Compilation, unit/integration test results, and Clippy remain unverified until the latest-head CI run completes.

## REV-018 — QOS-013 follow-up CI formatting failure and correction
- **Date:** 2026-10-27
- **Context:** Follow-up QOS-013 CI run 40435630512 on commit `90f7af86202a79e34910c78f56c77d519cb62fd6`.
- **Finding/correction:** The first formatting correction resolved the earlier reported diffs, but rustfmt identified one remaining match-arm wrapping difference in `InstrumentSizingOutcome::risk_decision`. Workspace check, tests, and Clippy were skipped again.
- **Impact:** Compilation/test evidence remained unavailable from that run.
- **Evidence:** GitHub Actions run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40435630512; Rust job 121963502803 failed at formatting step 6 and showed a single diff at `instrument_sizing.rs:106`.
- **Rule that failed:** Required rustfmt check was still not satisfied.
- **Rule update made in RULES.md:** None; current rules are sufficient.
- **Code/docs/tests changed:** Applied the final rustfmt-indicated match-arm wrapping. No intended behavior change.
- **Regression/real-use check added:** Run the complete CI matrix on the latest branch head.
- **Verification result:** Correction committed as `3dcce8660a26982f30b6a2b085574436e9a525ca`; latest-head CI pending.
- **Remaining risk:** Compilation, tests, and Clippy remain unverified until the current run finishes.

## REV-019 — QOS-013 compile failure in decimal step rounding and correction
- **Date:** 2026-10-27
- **Context:** QOS-013 CI run 40435757549 on commit `3b8a59fbf87eeae3fe329beb9835a2c8f11cb266`.
- **Finding/correction:** Formatting passed, but `cargo check --workspace` failed with E0631 because `Decimal::trunc` takes `&Decimal`, while `Option::map` expected a function taking `Decimal`. Changed the call to a closure `|steps| steps.trunc()`.
- **Impact:** Workspace tests and Clippy were skipped because compilation failed.
- **Evidence:** GitHub Actions run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40435757549; Rust job 121963920153 reported E0631 at `instrument_sizing.rs:414`.
- **Rule that failed:** Compile/test verification rule; implementation used an incompatible method function signature.
- **Rule update made in RULES.md:** None; current rules already require compiling before claiming correctness.
- **Code/docs/tests changed:** Corrected step rounding to use a closure; no intended behavior change.
- **Regression/real-use check added:** Full latest-head CI rerun.
- **Verification result:** Correction committed as `603282cecc1c46e2788de194f3eed17b9b9f7388`; rerun pending. No test success claimed.
- **Remaining risk:** Additional compiler/test/Clippy findings may remain until the full CI suite passes.


## REV-020 — QOS-013 sell-stop geometry regression test failure and correction
- **Date:** 2026-10-27
- **Context:** QOS-013 CI run 40435876274 on commit `6180e7a636eae391e1f79d840c22495ffa29bf69`.
- **Finding:** `cargo fmt`, workspace check, and the majority of workspace tests ran successfully, but `risk_engine::instrument_sizing::tests::sell_stop_at_or_below_entry_rejects` failed. Its fixture used entry 100 and stop 101, which is a valid protective stop for a sell order (sell stop must be strictly above entry), while its name and expected rejection described an invalid stop at or below entry.
- **Correction:** Changed the fixture stop from 101 to 100 so the test actually exercises equality, which must reject per `RULES.md` and `INSTRUMENT_AWARE_SIZING.md`. The production geometry check already correctly requires sell stop > entry; no production behavior was weakened.
- **Evidence:** GitHub Actions run https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40435876274, Rust job 121964300309. Failure reported at `crates/risk-engine/src/instrument_sizing.rs:517`: actual Approved versus expected InvalidStopGeometry. Frontend job 121964300550 passed.
- **Rule update:** None. The existing stop-geometry rule is clear; the regression fixture was inconsistent with it.
- **Verification:** Fix pushed to the QOS-013 branch; new CI run must pass before the task can be treated as verified.
- **Remaining risk:** Full SZ-01–SZ-26 implementation and production runtime snapshot wiring are still open; this correction addresses only the failed test fixture.

- **Verification result update (2026-10-27):** Corrected-head CI run 40436418111 completed with conclusion `success`. Rust format, workspace check, workspace test, and all-target Clippy steps passed; frontend lint, typecheck, and production build also passed. This verifies this implementation slice on the cited commit only, not full QOS-013 completion or production/live readiness.


## REV-021 — Explicit instrument-aware runtime entry point (CI pending)
- **Date:** 2026-10-27
- **Context:** Follow-up implementation after PR #9 merged the first instrument-aware sizing slice.
- **Finding:** The sizing API existed behind `DefaultRiskEvaluator`, but the production runtime had no explicit entry point to use it. The only runtime path invoked `RiskEvaluator::evaluate`, which retained legacy price-distance sizing.
- **Change:** Added `TradingRuntime::execute_intent_with_instrument_sizing`, which accepts a `RuntimeInstrumentSizingContext` that binds `InstrumentSizingContext` to account and instrument IDs, validates those IDs against runtime/intent identity, and invokes `evaluate_with_instrument_sizing` only on a match. The method routes through the same risk → decision persistence/events → OMS pipeline and does not fall back to legacy sizing when the explicit entry point is selected. The original `execute_intent` remains unchanged at its public interface for compatibility.
- **Regression test:** Added `instrument_aware_runtime_rejects_stale_sizing_before_oms` in `crates/trading-runtime/tests/continuous_pipeline.rs`; it expects `DependencyUnavailable`, no order status, and no OMS open orders for stale context and mismatched instrument identity.
- **Verification:** Changes pushed to `feat/qos-013-runtime-sizing-boundary`; CI has not yet completed. No tests are claimed as passing for this branch until CI evidence is available.
- **Remaining risk:** Runtime-owned authoritative timestamped snapshot retrieval and freshness calculation are not implemented. The context remains caller-supplied and is not yet a timestamped authoritative snapshot; margin, pending-order reserves, structured persistent sizing audit, property tests, and full SZ-01–SZ-26 coverage remain open. No live execution, deployment, or money movement enabled.


## REV-022 — QOS-013 runtime regression formatting check failed and corrected
- **Date:** 2026-10-27
- **Context:** PR #10 CI run 40438016915, Rust job 121971313717.
- **Finding:** `cargo fmt --all -- --check` failed on formatting-only differences in the newly added `crates/trading-runtime/tests/continuous_pipeline.rs` regression: constructor wrapping and indentation of nested `InstrumentSizingContext` fields. Workspace check, tests, and Clippy were not reached by the sequential Rust workflow.
- **Correction:** Applied the formatting shown by rustfmt to those test expressions; no intended behavior change.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40438016915/job/121971313717.
- **Verification:** Correction pushed to `feat/qos-013-runtime-sizing-boundary` in commit `f0f4a470310e4b42a6842199f7db68d298a06c88`. A new latest-head CI run is required; no compilation/test/Clippy success is claimed until it completes.
- **Remaining risk:** The runtime sizing entry point is caller-fed and not yet wired to authoritative timestamped data sources. No live execution or money movement enabled.


## REV-023 — QOS-013 runtime sizing boundary full CI passed after rustfmt correction
- **Date:** 2026-10-27
- **Context:** Corrected PR #10 head `070e822d1afb2636c289d12ab79de042496d8400`; GitHub Actions run 40438207539.
- **Finding:** After REV-022's formatting correction, the latest-head run completed successfully. Rust job passed `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets`. Frontend job passed lint, typecheck, and production build.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40438207539. Run conclusion: success; Rust job 121971942780 and frontend job 121971942194 both concluded success.
- **Code/docs/tests changed:** No additional code change in this verification record; status summaries updated with exact CI evidence.
- **Verification result:** The PR head is verified by the full CI workflow. PR #10 remains open and unmerged. This does not establish live or production readiness and does not complete QOS-013.
- **Remaining risk:** Runtime context remains caller-supplied rather than sourced from authoritative timestamped snapshots; margin, pending-order reserves, structured persistent sizing audit, property tests, and full SZ-01–SZ-26 coverage remain open. No live execution or money movement enabled.


## REV-024 — Frontend production build failed in Next.js Google font loader; workaround applied to PR #10
- **Date:** 2026-10-27
- **Context:** PR #10 head `3d457365e78f8a0710cdb1281d134c52a639a68d`; GitHub Actions run 40438370475, frontend job 121972469846.
- **Finding:** `npm ci`, `npm run lint`, and `npm run typecheck` passed, but `npm run build` failed in `frontend/src/app/layout.tsx` within `next/font/google/loader.js` with `TypeError: Cannot read properties of null (reading '1')`. The root layout imported IBM Plex fonts via `next/font/google`, making the production build depend on an external Google Fonts fetch/response parse.
- **Correction:** Removed `next/font/google` imports; changed the global body and chart text to deterministic system font stacks. The theme switching script and PAPER metadata remain unchanged. This fix is committed directly to the PR #10 branch to rerun the original failing workflow.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40438370475/job/121972469846.
- **Rule that failed:** CI/release rule 2 (report exact commands/results) and workflow rule 7 (claims must match evidence); prior head had a failed production build.
- **Rule update made in RULES.md:** None; existing rules already require a working production build and accurate test claims.
- **Code/docs/tests changed:** `frontend/src/app/layout.tsx`, `frontend/src/styles/globals.css`, and `frontend/src/components/charts/EquityChart.tsx`.
- **Regression check:** Latest-head CI must pass `npm ci`, `npm run lint`, `npm run typecheck`, `npm run build`, and the Rust job.
- **Verification result:** Frontend fix is pushed to `feat/qos-013-runtime-sizing-boundary`; CI result for the combined head is pending. Do not claim build success until the rerun completes.
- **Remaining risk:** System stacks avoid remote font access but don't bundle IBM Plex, so rendering may vary by platform. No live execution or money movement enabled.



## REV-025 — QOS-013 combined Rust and frontend workflow passed after Google font fix
- **Date:** 2026-10-27
- **Context:** PR #10 runtime-sizing head after removal of `next/font/google`; GitHub Actions run 40439372393.
- **Finding:** The prior frontend build failure was eliminated by removing the build-time Google font loader dependency. The workflow completed with conclusion `success`.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40439372393. Rust job 121975721468 passed formatting, workspace check, tests, and Clippy. Frontend job 121975721940 passed lint, typecheck, and production build.
- **Code/docs/tests changed:** No additional behavior change for this verification entry; verification status is recorded here.
- **Verification result:** Combined implementation head tested by run 40439372393 passed both jobs. PR #10 remains open and unmerged; no live/production readiness is implied.
- **Remaining risk:** Runtime sizing still requires caller-supplied context rather than authoritative timestamped snapshots; pending-order reserves, margin, persistent structured sizing audit, property tests, and full SZ-01–SZ-26 coverage remain open. Font styling now uses system fallbacks, not bundled IBM Plex. No live execution or money movement enabled.


## REV-026 — QOS-013 expanded sizing implementation formatting failures and corrections
- **Date:** 2026-10-27
- **Context:** Draft PR #12, initial CI run 40443283430 on intermediate head `597587ef422720207f231aa0a11001e11a3b97a0`.
- **Finding:** Rust formatting gate found rustfmt differences in the evaluator fixture, instrument sizing branches/invariant test, runtime imports/test module, and runtime integration fixture. Sequential CI skipped check/test/Clippy for that run; frontend checks passed.
- **Correction:** Applied the rustfmt changes reported by CI and corrected the margin-limit conditional syntax during follow-up edits. Subsequent latest-head CI remains required; no compile/test/Clippy success is claimed from the failed run.
- **Rule update:** None; existing `RULES.md` already requires formatting and accurate verification claims.
- **Remaining risk:** QOS-013 is not complete until latest-head Rust format/check/test/Clippy and frontend checks pass, and a concrete authoritative snapshot provider plus remaining acceptance coverage are implemented or explicitly scoped by review. No live execution enabled.


## REV-027 — Margin-limit reason classification corrected
- **Date:** 2026-10-27
- **Context:** CI run `40443646563` failed the `available_margin_caps_quantity_independently` regression because a different lower-level cap was reported as the binding reason even when margin was the tighter constraint.
- **Correction:** Changed limiting-reason selection to compare the margin cap against the risk, position, exposure, leverage and venue-maximum caps before selecting `MarginLimit`.
- **Verification:** Corrected-head run `40444515528` passed Rust format/check/test/Clippy and frontend lint/typecheck/build.
- **Remaining risk:** No live venue is enabled; margin data must come from a fresh trusted snapshot.

## REV-028 — Venue-maximum limit reason classification corrected
- **Date:** 2026-10-27
- **Context:** CI run `40444389467` failed `venue_maximum_caps_quantity_without_rounding_up`; the code selected exposure/leverage as the reason even when venue maximum quantity was the binding cap.
- **Correction:** Prioritized the venue-maximum reason when the maximum quantity is no greater than the other effective caps.
- **Verification:** Corrected-head run `40444515528` passed the full Rust and frontend workflow.

## REV-029 — Final rustfmt blank-line correction
- **Date:** 2026-10-27
- **Context:** CI run `40444154016` failed `cargo fmt --all -- --check` on a trailing blank line in `instrument_sizing.rs`.
- **Correction:** Removed the trailing blank line. Corrected-head run `40444515528` passed formatting and all remaining Rust/frontend checks.

## REV-030 — Generated sizing invariant test formatting correction
- **Date:** 2026-10-27
- **Context:** CI run `40445162003` failed formatting in `generated_valid_specs_preserve_quantity_and_loss_invariants`.
- **Correction:** Applied the exact rustfmt loop formatting and removed the extra final blank line.
- **Verification:** Corrected-head run `40445289729` passed the full Rust and frontend workflow.

## REV-031 — Persistent sizing audit required for executable instrument-aware decisions
- **Date:** 2026-10-27
- **Context:** Review of the QOS-013 structured audit path found that the runtime allowed an executable instrument-aware decision to proceed when no persistent store was configured, which meant no sizing audit could be written.
- **Correction:** Instrument-aware execution now rejects with `DependencyUnavailable` when persistent audit storage is absent; added an integration regression proving fresh inputs still create no OMS order without the store.
- **Verification:** PR #13 head `7beb7987bb520faf81b965d5342973d4b5198082`, CI run `40445467228`, passed formatting, workspace check, tests, Clippy, frontend typecheck and production build.
- **Remaining risk:** Explicit risk-policy version IDs are not yet modeled; the audit stores the complete policy snapshot and sizing model version. No live execution enabled.
## REV-032 — QOS-014 initial file was written to main and immediately reverted
- **Date:** 2026-10-27
- **Context:** Starting QOS-014 MT5 bridge v1 protocol work.
- **Finding/correction:** The initial `create_file` call omitted the optional `branch` argument, so the protocol source file was committed directly to `main`. This violated the feature-branch workflow.
- **Impact:** Unreviewed protocol code briefly appeared on the default branch.
- **Evidence:** GitHub contents API showed the file commit on `main`; the file was then deleted in revert commit `3c995c10a75cc0e76672decf803489d04bbd9b76`. The file was recreated on `feat/qos-014-mt5-bridge-v1` at `de94075db38348db88cd299d0c758749f8b73c2b`.
- **Rule that failed:** Workflow rule 8 (prefer feature branches and reviewable PRs; do not bypass protections).
- **Rule update made in RULES.md:** None; rule is already explicit. Correction was immediate and no financial execution was involved.
- **Code/docs/tests changed:** Removed the direct-to-main file; recreated it on the feature branch; continued schema, protocol and tests only on the feature branch.
- **Regression/real-use check added:** Verify every subsequent file write explicitly supplies `branch: feat/qos-014-mt5-bridge-v1`; verify branch refs before PR creation.
- **Verification result:** Revert commit and feature-branch file commit are confirmed by GitHub responses. CI has not yet verified the protocol code.
- **Remaining risk:** Full Rust/frontend CI, schema conformance and actual terminal transport remain pending. No live execution enabled.


## REV-033 — QOS-014 first CI run caught formatting-only differences
- **Date:** 2026-10-27
- **Context:** Draft PR #14, run 40446398790 on head `b92ff3bf8568ca255ac9b72abf4f37edf0f70479`.
- **Finding/correction:** Rust formatting gate reported line wrapping and match-arm formatting in `crates/intent-bridge/src/mt5_v1.rs`; compilation, tests and Clippy were skipped by the workflow. The first correction missed one zero-quantity assertion; rerun 40446528312 caught that remaining line. It was corrected in commit `a400cbe7bbcfef3391e54e678438fb306124dee4`. Frontend checks continued independently.
- **Impact:** No Rust compile/test evidence was produced by that run.
- **Evidence:** GitHub Actions runs https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40446398790 and https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40446528312; Rust jobs `121998613711` and `121999036260`.
- **Rule that failed:** Formatting gate compliance; no financial-safety behavior was implicated.
- **Rule update made in RULES.md:** None. Existing testing rules already require format/check/test/Clippy and accurate reporting.
- **Code/docs/tests changed:** Applied the reported rustfmt line wrapping and match-arm formatting to `mt5_v1.rs`; then corrected the remaining assertion wrap after the second run.
- **Regression/real-use check added:** Rerun full CI on the corrected latest branch head.
- **Verification result:** Formatting corrections committed as `59dbf9dea5ebbf61b160230703e4e6dfd7f2a44d` and `a400cbe7bbcfef3391e54e678438fb306124dee4`; latest-head rerun pending.
- **Remaining risk:** Rust compilation, protocol tests, Clippy and latest-head frontend CI still need successful evidence. No live execution enabled.


## REV-034 — QOS-014 bounded incremental NDJSON framing
- **Date:** 2026-10-27
- **Context:** Continued QOS-014 on `feat/qos-014-mt5-bridge-v1` after the initial protocol types and schema were present.
- **Finding:** The original helper validated a single JSON string and relied on an external caller to frame the byte stream. A future transport could otherwise buffer an unbounded partial line or accidentally act on an incomplete frame.
- **Change:** Added `NdjsonFrameDecoder` with a 64 KiB line limit enforced while receiving chunks, newline-delimited validation, CRLF handling, multiple frames per chunk, and fail-closed poisoning after malformed/oversized/blank/non-UTF-8 input. EOF with a partial frame rejects. Added deterministic tests and a canonical JSON submit fixture.
- **Evidence:** `crates/intent-bridge/src/mt5_v1.rs`, `docs/contracts/fixtures/mt5_v1_submit_order.json`, and `docs/MT5_BRIDGE_V1.md` on the feature branch.
- **Rule update:** None; this implements existing API/bridge and security rules. It does not add transport/authentication or claim venue connectivity.
- **Verification:** Code and documentation updates were committed to the feature branch. Full latest-head CI has not yet been verified; no tests are claimed as passing until CI completes.
- **Remaining risk:** Authenticated connection binding, durable client-order idempotency, reconnect/sequence-gap reconciliation, and cross-language conformance against an MT5-side implementation remain open. No live execution, deployment, or money movement enabled.


## REV-035 — QOS-014 Rust formatting CI failure and correction
- **Date:** 2026-10-27
- **Context:** GitHub Actions run 40448579920, Rust job 122005653340, on the QOS-014 PR merge ref.
- **Finding:** `cargo fmt --all -- --check` failed only on line wrapping in three NDJSON decoder tests in `crates/intent-bridge/src/mt5_v1.rs`. The Rust job stopped at the formatting gate, so workspace check, tests, and Clippy did not run in that attempt. The frontend job passed.
- **Correction:** Applied the exact rustfmt line wrapping to the feature branch in commit `4265d240582aae6722a049b09a54337f01fbcfca`; no runtime behavior change intended.
- **Evidence:** https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/40448579920/job/122005653340.
- **Rule update:** None; existing rules require formatting and accurate verification claims.
- **Verification:** Fix is committed on `feat/qos-014-mt5-bridge-v1`. A new full latest-head CI result is still required; this failure run does not verify compilation or tests.
- **Remaining risk:** Authenticated transport, durable idempotency, reconnect/reconciliation, and cross-language MT5 conformance remain open. No live execution, deployment, or money movement enabled.


## REV-036 — QOS-014 second formatting correction completed
- **Date:** 2026-10-27
- **Context:** Continued inspection of CI run 40448579920 after REV-030.
- **Finding:** The first formatting correction fixed the decoder line wrap and the partial-frame assertion, but the workflow diff also showed a remaining wrapped assertion for invalid UTF-8 input.
- **Correction:** Applied the remaining rustfmt line wrapping in commit `e83897e3b99dcbecbe1114a6473fbca0e9aa0e0f`.
- **Verification:** A fresh full CI run is in progress for the QOS-014 head; do not infer success from a commit being pushed. No runtime behavior change intended.
- **Remaining risk:** Compilation, tests and Clippy still require a completed green CI run. No live execution enabled.


## REV-037 — QOS-014 repeated rustfmt diff and correction
- **Date:** 2026-10-27
- **Context:** Re-examined Rust format failure from run 40449083166/job 122007273488, following the initial formatting fix.
- **Finding:** The new CI log still reported two issues: an extra blank line before the first NDJSON decoder test and an unwrapped invalid-UTF-8 assertion. The initial branch update had fixed the assertion but left the extra blank line.
- **Correction:** Removed the extra blank line in commit `6b7fa8b8ff2fea86e1df23d6fbf4d6362ed94944`. The assertion wrap was already present in the previous branch head. The changes are formatting-only.
- **Verification:** Full CI is still required on the current branch head. Earlier runs were rejected at the formatting gate, so workspace compilation, tests and Clippy have not yet been verified.
- **Remaining risk:** Do not merge until a current-head CI run passes. No live execution or money movement enabled.


## REV-038 — Review-register revision IDs reconciled after parallel QOS histories merged
- **Date:** 2026-10-27
- **Context:** The QOS-013 capability-gate history and QOS-014 MT5 bridge history were both retained during merge-conflict resolution, but each branch had independently assigned REV-027 through REV-031.
- **Finding/correction:** Duplicate revision identifiers made references ambiguous. Preserved every historical entry and its original content, keeping QOS-013 entries REV-027 through REV-031 and renumbering the six QOS-014 entries sequentially as REV-032 through REV-037.
- **Impact:** Review records now have unique, stable identifiers without dropping either workstream's audit trail.
- **Evidence:** Compared the merged `REVIEWS.md` headings on `main`; five IDs were duplicated before this correction.
- **Rule that failed:** Documentation consistency and auditability; no runtime or financial-safety behavior was implicated.
- **Rule update made in RULES.md:** None. Existing rules require correction history and consistent, evidence-backed documentation.
- **Code/docs/tests changed:** `REVIEWS.md` only; no application code or tests changed.
- **Regression/real-use check added:** Check that every `REV-NNN` heading is unique after future history merges.
- **Verification result:** Programmatic heading scan confirmed the five duplicate IDs before correction; post-update scan and PR CI should confirm uniqueness and document-only diff.
- **Remaining risk:** This documentation-only correction does not change or verify runtime behavior, venue connectivity, or live readiness.


## REV-039 — QOS-017 account-isolated multi-account orchestration
- **Date:** 2026-10-27
- **Context:** QOS-016's initial failure-injection suite was merged in PR #15; the next queued task is QOS-017, multi-account orchestration above the single-account runtime path.
- **Finding:** The runtime currently executes one configured account at a time, and its prior client-order ID helper was based only on the trade-intent ID. Reusing that identifier across accounts could collide at venue boundaries. Falling back to legacy sizing in a multi-account path would also bypass the instrument-aware fail-closed contract.
- **Impact:** Naive fan-out could couple accounts through duplicate idempotency keys, allow one account's error to abort later account attempts, or execute without account-specific instrument sizing.
- **Evidence:** Inspected `crates/trading-runtime/src/pipeline.rs`, `runtime.rs`, `config.rs`, `ids.rs`, `crates/venue-connectors/src/adapter.rs`, `bridge.rs`, `crates/domain/src/ids.rs`, and QOS-016's merged failure-injection test file.
- **Rule that failed:** No production behavior violation is claimed. These are identified design gaps before introducing fan-out.
- **Rule update made in RULES.md:** None; existing rules 5–8 and 12–15 already require fail-closed risk, stable idempotency, explicit partial outcomes, and no promise of atomic multi-account execution.
- **Code/docs/tests changed:** Added `crates/trading-runtime/src/multi_account.rs`; exported coordinator types; added length-prefixed account-scoped client order IDs and wired the single-account pipeline to use them; updated `TASKS.md` and `PROGRESS.md`.
- **Regression/real-use check added:** Tests cover empty-target rejection, duplicate account IDs rejected before execution, venue binding mismatch rejected before execution, continuation after an account-local failure, and stable/distinct account-scoped IDs.
- **Verification result:** Source changes are committed to the feature branch. Full latest-head Rust format/check/test/Clippy and frontend CI have not yet been observed; no test pass is claimed.
- **Remaining risk:** Sequential fan-out is non-atomic and does not guarantee simultaneous fills. Real authoritative per-account snapshots, shared-database persistence semantics, durable cross-process idempotency, cancellation/reconciliation integration, and live venue readiness remain separate work. No live execution, deployment, or money movement enabled.


## REV-040 — QOS-014 durable MT5 idempotency and sequence checkpoints
- **Date:** 2026-10-27
- **Context:** User requested the next MT5 bridge implementation after QOS-017 merged.
- **Finding:** The merged MT5 v1 code validates protocol frames but did not persist client-order claims or sequence continuity. A retry after process restart could otherwise mistake a pre-send/pre-response crash as permission to submit again, and a reconnect could be treated as healthy without a durable checkpoint.
- **Impact:** Duplicate orders and execution from a stale or discontinuous bridge stream are possible unless persistence gates are part of the contract.
- **Evidence:** Inspected `crates/intent-bridge/src/mt5_v1.rs`, `crates/persistence/src/repos/mod.rs`, `crates/persistence/src/repos/orders.rs`, `crates/persistence/src/pool.rs`, and `docs/MT5_BRIDGE_V1.md` on `main`.
- **Rule that failed:** No production incident or live behavior violation is claimed; this is a missing safety boundary identified before transport integration.
- **Rule update made in RULES.md:** None. Existing financial-safety rules require stable idempotency, ambiguous outcomes to remain unknown, and stale/discontinuous state to block new exposure.
- **Code/docs/tests changed:** Added `migrations/20261027000002_mt5_bridge_idempotency.sql`, `crates/persistence/src/repos/mt5_bridge.rs`, public TradingStore APIs, bridge helpers in `mt5_v1.rs`, persistence error propagation, and MT5 contract/progress updates.
- **Regression/real-use check added:** Durable claims distinguish a new key from any existing key; JSONB payload mismatch rejects; accepted/rejected state cannot be rewritten by a conflicting late outcome; new/gapped/out-of-order sequence streams require explicit reconciliation before ready.
- **Verification result:** Changes are committed on `feat/qos-014-mt5-idempotency-v1`. CI and DB-backed concurrency/restart tests have not yet been observed; do not claim acceptance or live readiness.
- **Remaining risk:** The bridge helper boundary is not yet connected to a real MT5 transport; a process must use the claim result correctly and reconcile existing claims before retry. No live execution, deployment, or money movement enabled.


## REV-041 — QOS-014 formatting gate corrections and successful rerun
- **Date:** 2026-10-27
- **Context:** PR #18, durable MT5 idempotency and sequence checkpoint implementation.
- **Finding:** Initial CI attempts failed at `cargo fmt --all -- --check` on formatting-only differences in the MT5 bridge helper, persistence exports/facade, and durable repository. Workspace check/tests/Clippy were skipped on those attempts; frontend checks passed.
- **Correction:** Applied the exact rustfmt layout from CI, including simplifying nested tuple-field access by destructuring the decoded JSON row and formatting the final stream-degradation call.
- **Evidence:** Failed run `40452335204` job `122017703073`; successful code-head run `40452429799` on `5912da7143b3d5d1ee9cf703d8f474791c28207c`.
- **Rule that failed:** Rust format gate; no financial-safety behavior was implicated.
- **Rule update made in RULES.md:** None. Existing testing rules already require formatting, workspace check/test, Clippy, frontend checks and accurate claims.
- **Code/docs/tests changed:** Formatting-only corrections; added database-backed idempotency and sequence checkpoint tests; progress and bridge contract now reflect test coverage.
- **Regression/real-use check added:** Rerun all Rust and frontend CI steps after the corrections.
- **Verification result:** Run `40452429799` passed Rust formatting, workspace check, workspace tests, all-target Clippy, frontend lint, typecheck and production build on the recorded code head. Documentation follow-up requires the current PR-head workflow to complete before this PR is considered verified.
- **Remaining risk:** Actual authenticated MT5 transport and wiring to real send/reconnect operations remain unimplemented. No live execution, deployment, or money movement enabled.


## REV-042 — MT5 session identity and readiness gate before transport
- **Date:** 2026-10-27
- **Context:** User confirmed PR #18 merged and requested the next implementation completed before a real MT5 connection.
- **Finding:** Durable order claims and sequence checkpoints were present, but no per-connection state object enforced a single allow-listed account/terminal/server identity or required explicit reconciliation before dispatch. Protocol-level capabilities alone could be mistaken for session readiness.
- **Impact:** A future transport could accept account switching or treat a reconnect/heartbeat as permission to resume exposure before reconciliation.
- **Evidence:** Reviewed RULES.md, TASKS.md, docs/MT5_BRIDGE_V1.md, crates/intent-bridge/src/mt5_v1.rs, and merged PR #18 metadata.
- **Rule that failed:** No live incident or live execution is claimed; this was a missing pre-transport guard.
- **Rule update made in RULES.md:** None. Existing rules already require account identity binding, explicit capability checks, and reconciliation after ambiguity or stale/discontinuous state.
- **Code/docs/tests changed:** Added Mt5SessionPolicy and Mt5SessionGate with allow-list checks, immutable account/terminal/server binding, safe capability checks, submit/cancel gates, and a reconciliation latch; updated TASKS.md, PROGRESS.md, and docs/MT5_BRIDGE_V1.md.
- **Regression/real-use check added:** Unit tests cover fresh/gapped streams, disconnect/reconnect, account switching, non-allow-listed identities, missing safe capabilities, and supported submit/cancel checks.
- **Verification result:** Changes are on branch feat/qos-014-mt5-session-gating; CI has not yet run on this branch. No test pass is claimed before workflow evidence.
- **Remaining risk:** This is not authentication and does not implement sockets, a terminal Expert Advisor, real send/reconnect orchestration, broker mapping, or cross-language conformance. The future transport must authenticate the peer before creating a gate and must still enforce risk/OMS and durable idempotency. No live execution, deployment, or money movement enabled.


## REV-043 — Session gate initial CI formatting failure
- **Date:** 2026-10-27
- **Context:** PR #19 first CI run `40453162829`.
- **Finding/correction:** `cargo fmt --all -- --check` failed on import ordering and line wrapping in the new MT5 session gate and bridge module exports on runs `40453162829` and `40453293621`. The second run showed remaining module ordering and three nested `.unwrap()` chain layout differences. Workspace check, tests and Clippy were skipped by the format gate.
- **Impact:** The implementation was not verified at that head; no behavior failure was established by this run.
- **Evidence:** GitHub Actions runs `40453162829` (Rust job `122020325341`) and `40453293621` (Rust job `122020734888`), formatting step 6.
- **Rule that failed:** Rust formatting gate.
- **Rule update made in RULES.md:** None; the existing testing/release rule already requires formatting checks and reruns after correction.
- **Code/docs/tests changed:** Applied rustfmt's exact suggested import ordering, module ordering and wrapping changes in `crates/intent-bridge/src/lib.rs` and `crates/intent-bridge/src/mt5_session.rs` after both format failures.
- **Regression/real-use check added:** Re-run full Rust and frontend CI on the latest PR head.
- **Verification result:** Corrections for both format failures committed. Full CI run `40453466375` passed on head `36c233fcf3274191dce9bcb15d5b9bd97a078541`: Rust formatting, workspace check, workspace tests, all-target Clippy, frontend lint, TypeScript typecheck, and production build. This establishes CI success only, not production or live readiness.
- **Remaining risk:** Compilation, tests and Clippy are not verified until a successful latest-head run. No live execution, deployment, or money movement enabled.


## REV-044 — MT5 durable dispatch orchestration gap
- **Date:** 2026-10-27
- **Context:** User confirmed PR #19 merged and asked to implement the next milestone before a real MT5 connection.
- **Finding:** The protocol, durable order ledger and session gate existed as separate helpers, but no single coordinator enforced the order of operations or bound a response to the exact authorized submit.
- **Impact:** Future transport code could accidentally send before claiming, retry an existing prepared/unknown claim, or persist a mismatched response; an unrecorded outcome after possible venue execution must block further exposure.
- **Evidence:** Inspected merged main `crates/intent-bridge/src/mt5_session.rs`, `mt5_v1.rs`, `crates/persistence/src/repos/mt5_bridge.rs`, `docs/MT5_BRIDGE_V1.md`, and `TASKS.md`.
- **Rule that failed:** No live incident or live execution is claimed; this was a missing orchestration boundary.
- **Rule update made in RULES.md:** None; existing rules already require durable idempotency, fail-closed ambiguity handling, and no blind retries.
- **Code/docs/tests changed:** Added `Mt5DispatchDecision`, private-field `Mt5DispatchPermit`, `prepare_submit`, outcome binding/persistence and ambiguous transport failure handling in `crates/intent-bridge/src/mt5_dispatch.rs`; exported the API; updated MT5 contract, task and progress docs.
- **Regression/real-use check added:** Unit tests reject account/correlation/client-order mismatches and verify accepted/rejected/unknown state mapping.
- **Verification result:** GitHub Actions run `40454050187` passed on implementation head `80e74f7acf2786065800b5fa075a0216a8b965c9`: Rust formatting, workspace check/tests, all-target Clippy, frontend lint, TypeScript typecheck and production build. A final rerun is required after these verification-document updates.
- **Remaining risk:** Actual transport, send/reconnect wiring, authenticated peer identity, broker symbol mapping, cross-language conformance and end-to-end recovery remain open. No live execution, deployment, or money movement enabled.


## REV-045 — MT5 transport security contract before concrete transport

- **Date:** 2026-10-27
- **Context:** QOS-014 follow-up after PR #20 merged the durable dispatch coordinator.
- **Finding:** The merged session gate explicitly assumes its peer has already been authenticated. Implementing a socket/EA transport without a documented trust boundary could lead to trusting terminal-asserted account identity, accepting replayed handshakes, leaking credentials, or restoring readiness on reconnect alone.
- **Impact:** An unauthenticated or weakly bound peer could inject venue state or request messages into the bridge; reconnection could be incorrectly treated as readiness.
- **Evidence:** Inspected `RULES.md`, `crates/intent-bridge/src/mt5_session.rs`, `crates/intent-bridge/src/mt5_dispatch.rs`, and `docs/MT5_BRIDGE_V1.md`. The session gate documents that authentication must precede construction.
- **Rule:** Existing security rules require least privilege, secret redaction, untrusted-boundary validation, and no implicit live deployment; no rule was weakened.
- **Artifacts:** Added `docs/MT5_TRANSPORT_SECURITY.md`; reconciled QOS-014 status in `TASKS.md`; appended this work log to `PROGRESS.md`.
- **Regression/verification:** Documentation consistency review only. No executable tests were run for this docs-only slice. Transport implementation must test certificate/principal binding, unauthenticated-session rejection, replay, credential failure, reconnect latching, bounded input, and secret redaction.
- **Verification result:** Pending PR review and repository CI checks where configured; no code/test success is claimed.
- **Remaining risk:** No authenticated transport, terminal EA, broker mapping, or live execution exists.


## REV-046 — Authenticated MT5 transport boundary

- **Date:** 2026-10-27
- **Context:** QOS-014 follow-up after PR #21 merged the transport security contract.
- **Finding:** The existing session gate assumes the transport authenticated the peer; without a concrete boundary, account/terminal/server fields from a heartbeat could be mistaken for authenticated identity.
- **Impact:** An unauthenticated or unbound peer could inject bridge messages. A transport write could also be misinterpreted as venue acceptance if the contract were not explicit.
- **Evidence:** Inspected `RULES.md`, `docs/MT5_TRANSPORT_SECURITY.md`, `crates/intent-bridge/src/mt5_session.rs`, `crates/intent-bridge/src/mt5_dispatch.rs`, and `crates/intent-bridge/src/mt5_v1.rs`.
- **Rule:** Transport identity must be authenticated before session creation; untrusted boundary data must be validated; secrets must not be logged; ambiguous order outcomes must not be retried blindly. No safety rule was weakened.
- **Artifacts:** Added `crates/intent-bridge/src/mt5_transport.rs`; added Rust TLS/hash dependencies; exported transport APIs; updated `docs/MT5_TRANSPORT_SECURITY.md`, `TASKS.md`, and `PROGRESS.md`.
- **Regression/verification:** Added tests for principal configuration validation and certificate fingerprint hashing. CI is required for compilation, formatting, workspace tests and Clippy. End-to-end certificate/handshake, wrong-certificate, identity-mismatch, timeout and reconnect tests remain required.
- **Verification result:** Pending latest-head CI at time of entry.
- **Remaining risk:** No MT5 EA/client, broker mapping, protected runtime credential rotation, listener timeouts/resource limits, durable dispatch wiring, reconciliation integration, or live trading.


## REV-047 — Rust formatting check caught transport style drift

- **Date:** 2026-10-27
- **Context:** CI verification for QOS-014 authenticated transport PR #22.
- **Finding:** The first CI run on the transport implementation failed at `cargo fmt --all -- --check`; the new transport module had formatting differences.
- **Impact:** The patch could not pass the repository's mandatory format gate.
- **Evidence:** GitHub Actions run `40455353695`, Rust job `122027289328`, reported diffs in `crates/intent-bridge/src/mt5_transport.rs`.
- **Rule:** All code changes must pass repository formatting before being treated as verified.
- **Artifacts:** Applied the formatting corrections in `crates/intent-bridge/src/mt5_transport.rs` on branch `feat/qos-014-authenticated-transport`.
- **Regression/verification:** A new CI run `40455460153` is queued for head `037c678b9a4a22047233b5757b7621e7df543025`; success is not yet confirmed.
- **Verification result:** Formatting correction committed; latest-head CI pending.
- **Remaining risk:** Compile/API compatibility and tests still need to pass on the latest head.


## REV-048 — QOS-014 transport resource bounds and heartbeat freshness

- **Date:** 2026-10-27
- **Context:** Follow-up to merged PR #22; user requested QOS-014 continue toward full implementation.
- **Finding:** The authenticated TLS boundary had a fixed NDJSON frame cap but no explicit handshake/read/write deadlines, connection concurrency cap, or heartbeat-age check. A silent peer or stalled socket could otherwise retain runtime resources or leave stale transport state unnoticed.
- **Correction:** Added configurable positive handshake/I/O/heartbeat timeout values and maximum concurrent connections to `Mt5TransportLimits`. Defaults are 5s handshake, 15s I/O, 30s heartbeat and 64 concurrent connections. TLS handshakes and I/O are timed; connection capacity is guarded by an owned semaphore permit. Added a fail-closed heartbeat freshness method and invalid-limit tests.
- **Documentation:** Updated `docs/MT5_TRANSPORT_SECURITY.md`, `TASKS.md` and `PROGRESS.md` to describe implemented bounds and remaining integration gates.
- **Verification:** Current branch CI has not yet completed. No successful test/Clippy claim is made until CI reports a green result on the final PR head.
- **Remaining risk:** Heartbeat expiry is a callable watchdog API, not an autonomous background task. The host runtime must invoke it and connect failure to session degradation/reconciliation. Certificate fixture integration tests, dispatch/reconciliation wiring, MT5-side EA/client conformance, protected credential lifecycle, and end-to-end restart tests remain outstanding. No live trading, deployment or money movement enabled.


## REV-049 — QOS-014 transport-hardening formatting correction

- **Date:** 2026-10-27
- **Context:** PR #23 first CI run `40456634638`, Rust job `122031382040`.
- **Finding:** `cargo fmt --all -- --check` failed on line wrapping in connection semaphore acquisition, transport read errors and the frame identity validator signature. Workspace check/tests/Clippy were skipped by the format gate.
- **Correction:** Applied the exact formatting changes reported by rustfmt in `crates/intent-bridge/src/mt5_transport.rs`.
- **Verification:** A new full CI run must pass on the updated PR head before merge. No compile/test/Clippy success is claimed from the failed run.
- **Remaining risk:** This correction is formatting-only; runtime correctness remains unverified until the next full CI run and integration gaps listed in REV-048 remain open.


## REV-050 — QOS-014 transport export formatting correction

- **Date:** 2026-10-27
- **Context:** Second PR #23 CI run `40456746993`, Rust job `122031744793`.
- **Finding:** The transport module formatting was corrected, but rustfmt still rejected the long public re-export line in `crates/intent-bridge/src/lib.rs`. Workspace check/tests/Clippy remained skipped.
- **Correction:** Wrapped the public transport exports in rustfmt's expected multi-line form.
- **Verification:** Third full CI run is required on the new branch head. No Rust compilation or test pass is claimed from this run.
- **Remaining risk:** The previous run reached only formatting. The next run must pass formatting, workspace check/tests, Clippy and frontend checks before the slice is considered verified.


## REV-051 — Correct MT5 outbound NDJSON line framing

- **Date:** 2026-10-27
- **Context:** Post-merge code review following PR #23 (merge commit `7b47069623bb55d2994298d1e8eb9573f964bdb4`). The PR's CI passed on its final head and the merge-commit CI run `40457087734` passed, but those checks did not detect a protocol framing defect.
- **Finding:** The outbound transport writer was writing the two literal bytes backslash and `n` rather than a single LF delimiter. This could prevent a peer's NDJSON decoder from recognizing the frame boundary.
- **Correction:** Centralized serialization in `encode_frame_line`, enforces the existing frame size limit, appends exactly one LF byte, and writes the complete encoded line. Added a regression test that asserts one LF terminator, rejects a literal backslash-`n` suffix, and round-trips the JSON payload. Added deterministic heartbeat freshness boundary tests and poisons the connection on incomplete EOF.
- **Verification:** Follow-up PR CI is pending. The previous green CI is not evidence that this new correction has passed.
- **Severity / release gate:** Protocol correctness issue; do not connect to an MT5 client until this fix passes CI and the end-to-end transport conformance tests are implemented. No live trading enabled.


## REV-052 — QOS-014 conformance PR formatting correction

- **Date:** 2026-10-27
- **Context:** PR #24 CI run `40457381942`, Rust job `122033762951`.
- **Finding:** `cargo fmt --all -- --check` rejected the multi-line signature of the short `heartbeat_is_fresh` helper. Workspace compilation, tests and Clippy were skipped by the formatting gate.
- **Correction:** Applied the exact rustfmt signature layout.
- **Verification:** A new full CI run is required on the updated PR head; no compilation or test success is claimed from the failed run.


## REV-053 — QOS-014 conformance PR Clippy failure

- **Date:** 2026-10-27
- **Context:** PR #24 CI run `40457521240`, Rust job `122034210752`; formatting, workspace check and workspace tests passed, but all-target Clippy failed.
- **Finding:** The new outbound NDJSON regression test used two `unwrap()` calls, which are denied by the `intent-bridge` crate's `clippy::unwrap_used` policy.
- **Correction:** Replaced both unwraps with explicit error-reporting branches in the test, preserving coverage for exact LF framing and JSON round-trip.
- **Verification:** A fresh full CI run is required on the updated PR head. This Clippy failure must not be treated as a code or test pass.
- **Remaining risk:** MT5 transport integration gates documented in REV-051 and the PR description remain open; no live trading enabled.


## REV-054 — QOS-014 test lint correction follow-up

- **Date:** 2026-10-27
- **Context:** PR #24 rerun `40457817820`, Rust job `122035147781`. Formatting, workspace check and workspace tests passed, but Clippy rejected the first correction because test-only `panic!` branches violated the crate-wide `clippy::panic` deny rule.
- **Correction:** Changed the framing regression test to return `BridgeResult<()> `, use `?` for serialization/deserialization errors, and return an explicit validation error if the LF suffix is absent. This preserves assertions without `unwrap`, `expect`, or `panic!`.
- **Verification:** A fresh CI run on this latest head must pass before PR #24 is considered verified.
- **Remaining risk:** MT5 transport integration gates from REV-051 remain open; no live trading enabled.


## REV-055 — QOS-014 heartbeat expiry must degrade session state

- **Date:** 2026-10-27
- **Context:** Follow-up after PR #24 merged and merge-commit CI passed.
- **Finding:** The transport exposed heartbeat freshness validation, but expiry only poisoned the connection object. Unless the caller separately degraded the associated `Mt5SessionGate`, a host integration could leave session readiness state stale.
- **Correction:** Added `check_heartbeat_fresh_with_session`, which fails closed by poisoning the connection and degrading the supplied session gate on stale/missing/future heartbeat time. Added a regression test for expiry and a healthy-heartbeat control case.
- **Verification:** Pending PR-head CI; no pass is claimed until the complete Rust and frontend workflows succeed.
- **Remaining risk:** This is an explicit watchdog callback, not an autonomous background scheduler. TLS integration tests, durable dispatch/reconciliation orchestration, restart recovery and MT5-side conformance remain open. No live trading enabled.


## REV-056 — QOS-014 heartbeat watchdog formatting correction

- **Date:** 2026-10-27
- **Context:** PR #25 CI run `40458446603`, Rust job `122037144816`.
- **Finding:** `cargo fmt --all -- --check` rejected three formatting differences in the new heartbeat freshness helper and regression test; workspace check, tests and Clippy were skipped.
- **Correction:** Applied the exact rustfmt layout to the helper expression and test assertions.
- **Verification:** A fresh full CI run is required on the updated PR head; no Rust test or Clippy success is claimed from the formatting-failed run.
- **Remaining risk:** Automatic watchdog scheduling, end-to-end TLS tests, durable dispatch/reconciliation wiring, restart recovery and MT5 EA conformance remain open. No live trading enabled.


## REV-057 — QOS-014 authenticated transport/runtime/reconciliation integration

- **Date:** 2026-10-27
- **Context:** PR #26 extends the merged mTLS boundary after PR #25 added heartbeat-to-session degradation.
- **Findings addressed:** The prior transport helpers were not orchestrated into a runtime, incoming heartbeat sequences were not checkpointed at the frame/session boundary, durable dispatch permits were not consumed by the authenticated transport path, and restart reconciliation did not persist a complete external-state snapshot.
- **Changes:** Added generated-certificate TLS handshake tests; a Tokio heartbeat watchdog that poisons the connection, degrades the session and durable stream; `read_frame_with_session_checkpoint`; `Mt5BridgeRuntime`; a one-shot dispatch method that persists ambiguous results as `Unknown`; typed account/open-order/position reconciliation snapshots; PostgreSQL snapshot persistence with atomic checkpoint readiness; restart and end-to-end local TLS/DB tests.
- **Security detail:** Heartbeat identity is validated against the session allow-list/binding before creating/updating durable stream state. Existing order keys do not produce a send permit. Reconciliation rejects stale/future snapshots, duplicate external IDs, invalid decimal values, and identity mismatch. Snapshot persistence is append-only and serializes concurrent reconciliation attempts per account/terminal.
- **Verification:** Intermediate CI runs have failed at the Rust formatting gate while files were still being assembled; exact rustfmt corrections are being applied. The latest final-head Rust format/check/test/Clippy and frontend lint/typecheck/build run must pass before the PR is considered verified. No success is claimed from an earlier head.
- **Remaining release gates:** This PR does not supply a real MT5 EA/client, production daemon deployment, protected certificate provisioning/rotation, broker-specific symbol/volume/price mapping, or a live authoritative snapshot query. Integration tests use local generated certificates and controlled test snapshots. No live trading enabled.


## REV-058 — Prevent readiness restoration without durable external-state evidence

- **Date:** 2026-10-27
- **Context:** Follow-up review while wiring reconciliation snapshots into the runtime.
- **Finding:** The older `mark_stream_reconciled` persistence API could restore `ready` based only on a sequence number, bypassing the newly added external account/order/position snapshot. Concurrent reconciliation/degradation operations could also race with checkpoint updates.
- **Correction:** The legacy readiness API now requires a persisted snapshot at or beyond the requested sequence, uses an account/terminal transaction advisory lock, and rejects checkpoint rollback. Snapshot persistence and stream degradation use the same lock, and snapshot persistence updates the snapshot record plus ready checkpoint atomically.
- **Regression coverage:** The persistence test asserts that readiness restoration fails without a snapshot, then persists account state/open orders/positions and verifies restart recovery, stale sequence rejection, and subsequent sequence continuity.
- **Verification:** Final-head Rust formatting, workspace check/tests, all-target Clippy, and frontend CI are still required. Intermediate failures to date have been formatting-gate failures; do not claim final verification until a latest-head run passes.
- **Remaining risk:** Snapshot records are only as authoritative as the source supplied by the host. The real MT5 EA/client and its broker-specific external-state query adapter remain unimplemented; no live trading enabled.


## REV-021 — QOS-013 MT5 sizing-data adapter
- **Date:** 2026-10-27
- **Context:** PR #29 merged; next scoped task connects persisted MT5 market data to instrument-aware sizing.
- **Finding/correction:** The bridge stored broker instrument metadata and quotes, but the risk engine had no strict adapter to turn the persisted snapshot into sizing inputs.
- **Impact:** Without an explicit adapter, a caller could accidentally use stale prices, wrong account currency, non-broker volume steps, or missing FX conversion data.
- **Evidence:** Added pure adapter crates/risk-engine/src/mt5_market_data.rs and a persistence reader scoped to exact account/terminal keys.
- **Rule that failed:** No rule violation asserted; this is a completion gap.
- **Rule update made in RULES.md:** None; existing fail-closed freshness, exact-decimal, account identity, and risk-before-execution rules apply.
- **Code/docs/tests changed:** Added adapter, latest-snapshot reader, persistence tests, and QOS-013 progress notes. Costs, margin per quantity, pending exposure, and pending margin remain explicit trusted inputs because the market-data frame does not contain those values.
- **Regression/real-use check added:** Unit tests cover ask/bid selection, tick value, broker volume step, FX conversion, stale snapshots, account-currency mismatch, and invalid margin inputs; persistence tests cover exact account/terminal retrieval.
- **Verification result:** Implementation committed to feature branch; CI not yet run at time of this entry.
- **Remaining risk:** CI may identify compile/test issues; Windows MT5 demo-terminal conformance is still pending. No order submission or live mode enabled.


## REV-022 — QOS-013 persisted MT5 runtime consumer (CI pending)
- **Date:** 2026-10-27
- **Context:** PR #30 merged the persistence reader and pure MT5 sizing adapter. The runtime still needed to compose those inputs with the explicit instrument-aware risk path.
- **Finding:** Without side-aware provider loading, broker ask/bid could be used inconsistently; without propagating the selected broker entry price, sizing could use one price while the order request retained the intent's stale price.
- **Change:** Added `Mt5PersistedSizingProvider`, exact instrument-to-broker-symbol mapping, server/currency/sequence/capture identity checks, a dynamic trusted supplement provider for margin/cost/exposure inputs, side-aware snapshot loading, and entry-price propagation into both risk evaluation and the order request. Capability and account position semantics must be explicitly validated.
- **Safety:** No legacy fallback is added to the explicit provider path. No execution feature was added. Windows MT5 demo conformance remains a release gate.
- **Verification:** Exact-head CI and runtime integration tests are pending; do not mark QOS-013 done until they pass.
- **Remaining risk:** The caller must configure trusted supplement and capability/position-semantics providers correctly. No live trading readiness is claimed.
