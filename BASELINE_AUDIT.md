# Quant OS — Baseline Audit (QOS-001)

**Audit date:** 2026-10-27  
**Repository:** `apostleoffinance/event-driven-trading-rs`  
**Audited ref:** `main` at `14619a758ac9864edb52958195de57fb5ca4b685`  
**Commit:** merge commit for PR #1, “docs: establish Quant OS product roadmap and engineering rules”  
**Method:** GitHub source/tree inspection. No local source checkout or executable test run was available during this audit.

## Executive result

QOS-001 source audit is complete. The repository is a 13-member Rust workspace with Rust integration tests, inline unit tests, and a Python strategy test. The executable Rust order path currently supports simulated/paper execution and a prop-style paper simulator. The prop connector's live mode is explicitly not implemented. The frontend is mock-first; requesting API data mode currently falls back to mock repositories, and the simulation UI can emit synthetic trade lifecycle events. Therefore neither the frontend's “LIVE” label option nor mock trade events prove live execution capability.

**Not established by this audit:** that the workspace compiles, tests pass, lint/build passes, database migrations run, or any venue integration works end-to-end. Those require executable checks under QOS-002 and integration-specific evidence.

## 1. Repository and branch baseline

- Default branch: `main`.
- Audited `main` commit: `14619a758ac9864edb52958195de57fb5ca4b685` (merge of PR #1).
- The merged commit includes the product/engineering documentation baseline.
- At audit time, GitHub listed `main` and the former documentation branch `docs/quant-os-product-operating-system`.
- A local clone was attempted, but `git clone --depth 1 https://github.com/apostleoffinance/event-driven-trading-rs.git /tmp/event-driven-trading-rs` failed with `Could not resolve host: github.com`. Consequently, no local Git status, Rust toolchain version, Node/npm version, or executable command result could be verified here.
- GitHub's repository tree contains no `.github/workflows` files, and the Actions API returned `total_count: 0` workflow runs. There is no CI result available to use as a green baseline.

## 2. Rust workspace and test inventory

The root `Cargo.toml` declares **13 workspace members**:

1. `crates/domain`
2. `crates/events`
3. `crates/strategy-runtime`
4. `crates/risk-engine`
5. `crates/account-engine`
6. `crates/execution-engine`
7. `crates/venue-connectors`
8. `crates/persistence`
9. `crates/trading-runtime`
10. `crates/reconciliation`
11. `crates/intent-bridge`
12. `crates/event-trading` (preserved legacy paper engine)
13. `strategies/mean-reversion`

Python strategy code and tests live under `strategies/python` and are not a Cargo workspace member.

### Checked-in integration test files

- `crates/account-engine/tests/with_risk.rs`
- `crates/domain/tests/phase1_architecture.rs`
- `crates/events/tests/market_data_to_trade_intent.rs`
- `crates/execution-engine/tests/invariants.rs`
- `crates/intent-bridge/tests/python_intent_ingest.rs`
- `crates/persistence/tests/survive_restart.rs`
- `crates/reconciliation/tests/venue_vs_internal.rs`
- `crates/risk-engine/tests/intent_to_decision.rs`
- `crates/trading-runtime/tests/continuous_pipeline.rs`
- `crates/venue-connectors/tests/ambiguous_unknown.rs`
- `crates/venue-connectors/tests/e2e_pipeline.rs`
- `crates/venue-connectors/tests/prop_pipeline.rs`
- `strategies/mean-reversion/tests/runtime_flow.rs`

The tree also contains inline Rust unit-test modules and `strategies/python/tests/test_mean_reversion.py`. This is an inventory of checked-in tests, **not** evidence that they pass.

### Commands still required under QOS-002

Run from the repository root and record each exact exit code/output:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace`
- applicable clippy (at minimum `cargo clippy --workspace --all-targets`, with warnings/errors preserved)

Also inspect/run Python tests and frontend checks only after confirming their local dependencies/tooling. Do not substitute documentation claims or old audit results for current command output.

## 3. Execution, venue and risk status

### Current execution boundary

- `crates/venue-connectors/src/adapter.rs` defines `VenueAdapter` methods for venue name, account state, submit/cancel, open orders, positions and order lookup.
- The trait does **not** currently expose an explicit capability model, instrument-spec lookup, health/readiness, or an async venue-event stream.
- `crates/venue-connectors/src/simulated.rs` implements a paper/simulated venue.
- `crates/venue-connectors/src/prop.rs` implements a prop-style **paper** venue. Its `PropVenueMode::Live` is documented as gated until a concrete firm's REST/WebSocket API is selected; live adapter methods return `VenueError::NotImplemented`.
- `crates/intent-bridge/src/main.rs` accepts Python TradeIntent NDJSON and selects the simulated or prop adapter. The CLI has no live-mode flag; the prop path bootstraps the paper adapter.
- The legacy `crates/event-trading` contains Binance and Bybit public market-data fetchers. This is not evidence of authenticated order execution on either exchange.
- No MT5 or cTrader execution adapter was found in the workspace tree.
- The bridge in `crates/venue-connectors/src/bridge.rs` maps ambiguous submit errors to `OrderStatus::Unknown`, which is a useful safety behavior. However, it marks an order cancelled after `VenueAdapter::cancel_order` returns success; provider-specific confirmation semantics and the case where an order remains open after a cancel acknowledgement still need contract/failure-injection tests.
- `crates/trading-runtime/src/pipeline.rs` executes one intent against `RuntimeConfig.account_id` and `venue_id`. Multi-account fan-out is not implemented in this path.
- `crates/account-engine/src/service.rs` uses an in-memory account registry and rejects `AccountType::Vault`; PostgreSQL persistence exists in a separate crate.
- `crates/risk-engine/src/sizing.rs` caps by position, exposure, leverage and stop-distance. The inspected stop-distance cap does not account for contract/tick value, pip/tick value, quote-currency conversion or venue volume steps. Do not treat it as complete FX/CFD/derivatives sizing.

### Supported status classification

| Capability | Baseline classification | Evidence / boundary |
|---|---|---|
| Simulated venue orders/fills | Implemented in code; tests exist | `SimulatedVenue`, connector integration tests; execution not run in this audit |
| Prop-style paper orders/fills | Implemented in code; tests exist | `PropVenue::paper`, `prop_pipeline.rs`; execution not run in this audit |
| Prop live order execution | Not implemented | `PropVenueMode::Live` is explicitly gated and returns `NotImplemented` |
| Binance/Bybit public market data | Present in legacy engine | Fetcher modules; no implication of live order execution |
| MT5/cTrader live execution | Not found | No adapter in workspace tree |
| Multi-account order fan-out | Not implemented in runtime path | Single account/venue in `RuntimeConfig` and `execute_intent` |
| Vault accounts/product flow | Reserved / rejected | Domain comment and account-engine validation |
| Venue async event stream/capabilities/health/spec lookup | Contract gaps | Missing from current `VenueAdapter` trait |

## 4. Frontend status

- `frontend/package.json` identifies Next.js 15.5.7, React 19 and TypeScript. Scripts are `dev`, `build`, `lint` (`next lint`) and `typecheck` (`tsc --noEmit`).
- `frontend/README.md` explicitly describes the console as **PAPER**, mock repositories and mock event stream; it does not execute trades, evaluate risk, or hold secrets.
- `frontend/src/lib/repositories/index.ts` states that API mode is not wired and falls back to mock repositories even when API mode is requested.
- `frontend/src/features/simulation/SimulationProvider.tsx` emits synthetic signal/intent/risk/order/fill/position events. These are simulation events, not backend execution receipts.
- `frontend/src/config/frontend.ts` permits a public environment variable to select the displayed environment label (`paper`, `shadow`, `live`), but that label does not activate a live backend. This mismatch is a presentation/configuration risk: never interpret the label alone as proof of live connectivity.
- Theme source defaults to dark mode; light mode is available. Current CSS uses blue accents and does not yet match the product design direction of white-dominant layouts with navy/orange accents. This is a UI follow-up, not changed in QOS-001.
- Frontend lint, typecheck and build were **not run**.

## 5. Risks and next steps

1. Run QOS-002 with a working local checkout/toolchain; capture exact outcomes before calling the codebase green.
2. Keep QOS-003 as the dedicated frontend scripts/dependency/token audit before UI changes.
3. Complete QOS-010/QOS-012 specifications before adapter or allocator implementation.
4. Treat live venue integration as unimplemented until a specific provider, credentials/access model, execution API, reconciliation semantics, and paper evidence are validated.
5. For multi-account execution, add independent account authorization, stable per-account order identity, partial-failure outcomes and reconciliation; do not loop the current single-account path without those controls.
6. Do not enable live trading, production deployment, or money movement as part of this audit.

## Evidence scope

Source files and repository metadata inspected through GitHub include `Cargo.toml`, `README.md`, `RULES.md`, `TASKS.md`, `PROGRESS.md`, `docs/migration/AUDIT.md`, venue adapter/types/bridge/prop/simulated modules, runtime config/pipeline, risk sizing, account service/domain, intent-bridge CLI, frontend package/README/config/repository factory/mock repository/simulation provider/theme provider/CSS, and the recursive repository tree. No source files were modified for this audit; the task/progress/review records are updated alongside this report.
