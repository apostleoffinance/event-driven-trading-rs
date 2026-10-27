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

