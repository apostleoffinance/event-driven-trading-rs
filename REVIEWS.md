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
- **Change:** Added `TradingRuntime::execute_intent_with_instrument_sizing`, which accepts a caller-supplied `InstrumentSizingContext` and invokes `evaluate_with_instrument_sizing`. The method routes through the same risk → decision persistence/events → OMS pipeline and does not fall back to legacy sizing when the explicit entry point is selected. The original `execute_intent` remains unchanged at its public interface for compatibility.
- **Regression test:** Added `instrument_aware_runtime_rejects_stale_sizing_before_oms` in `crates/trading-runtime/tests/continuous_pipeline.rs`; it expects `DependencyUnavailable`, no order status, and no OMS open orders when context freshness is false.
- **Verification:** Changes pushed to `feat/qos-013-runtime-sizing-boundary`; CI has not yet completed. No tests are claimed as passing for this branch until CI evidence is available.
- **Remaining risk:** Runtime-owned authoritative timestamped snapshot retrieval and freshness calculation are not implemented. The context is caller-supplied; margin, pending-order reserves, structured persistent sizing audit, property tests, and full SZ-01–SZ-26 coverage remain open. No live execution, deployment, or money movement enabled.
