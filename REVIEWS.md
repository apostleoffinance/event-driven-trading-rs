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
