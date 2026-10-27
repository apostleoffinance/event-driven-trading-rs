# QOS-013 — Risk-aware sizing implementation plan

Last updated: 2026-10-27. This is a source audit and acceptance plan, not an implementation claim. Follow `RULES.md` and the full contract in `INSTRUMENT_AWARE_SIZING.md`.

## 1. Scope and status

**Status: CORE IMPLEMENTATION COMPLETE — CI verified on PR #12 merged at `148ee80c8dc9be7e9d45b5f5acd80b30685149b3` and PR #13 head `7beb7987bb520faf81b965d5342973d4b5198082` (run `40445467228`).** The venue-neutral runtime now obtains timestamped snapshots through a runtime-owned cache/provider boundary, validates source/conversion paths, account/instrument identity, account-state freshness, venue-capability validation and known netting/hedging semantics, applies pending-order exposure/margin reserves and instrument-aware sizing, and persists an append-only structured audit before OMS. Live MT5/venue data-source integration remains a QOS-014/015 release gate; this status does not imply production/live readiness.

Goal: make risk sizing an explicit, instrument-aware decision behind the existing risk authorization path. Strategies remain limited to emitting `TradeIntent`; they must not submit orders or bypass risk authorization.

Out of scope for this slice: live venue integration, MT5 bridge, frontend changes, multi-account order fan-out, persistence redesign, portfolio allocation/rebalancing, and unsupported nonlinear instrument models.

## 2. Source audit — observed current behavior

| Source | Verified current behavior / gap | Implementation implication |
|---|---|---|
| `crates/risk-engine/src/sizing.rs` | `size_order` validates positive entry/equity, chooses requested quantity/notional or policy position cap, then applies position, exposure, leverage, and optional trade-risk caps. Trade-risk quantity is computed as equity risk budget divided by absolute price-stop distance. It rounds with `round_dp(8)`. | This is not a generally valid FX/CFD sizing model; decimal display precision is not venue volume-step rounding. Do not claim instrument-aware sizing until explicit model inputs and final-loss checks exist. |
| `crates/domain/src/risk.rs` | `RiskRequest` contains intent/account/instrument IDs, side, requested size, entry/stop, account state, policy, current exposure, session flag, and request timestamp. It does not carry an instrument specification, quote/conversion snapshot, margin snapshot, freshness limits, pending-order reserves, or a structured sizing audit record. | Extend the request through typed domain structures rather than inferring contract semantics from a symbol string. Missing required data must reject, not default to parity or a generic formula. |
| `crates/domain/src/instrument.rs` | Existing instrument metadata does not provide the full tick-size/tick-value/currency/contract multiplier/volume-step/min-max/cost model required by QOS-012. | Model supported instruments explicitly; unsupported/incomplete specs must be non-executable. Avoid silently changing all existing instruments to a guessed model. |
| `crates/domain/src/account.rs` | Account state provides account metrics but not a complete, timestamped per-account risk/conversion/margin snapshot bundle for this contract. | Keep account state identity separate from venue connectivity; freshness and account-currency semantics need explicit inputs before relying on them. |
| `crates/risk-engine/src/evaluator.rs` | Risk evaluation composes rules and sizing into a `RiskDecision`; sizing is not an order-submission interface. | Preserve risk-before-OMS/execution. Add a boundary regression proving rejected/halted decisions cannot become executable order requests. |
| `crates/trading-runtime/src/pipeline.rs` | Runtime coordinates intent, risk and execution; current path is single-account. | Do not add multi-account fan-out as a side effect of sizing. Keep the sizing decision pure and deterministic. |
| `INSTRUMENT_AWARE_SIZING.md` | Defines tick-value and linear models, conversion, costs/buffers, independent caps, pending reserves, conservative step rounding, fail-closed outcomes, and scenarios SZ-01–SZ-26. | This document is the target contract. Implement in staged slices; do not mark the entire specification met by tests for only a subset. |

## 3. Implementation sequence

1. **Typed input/model foundation:** add explicit sizing inputs for the supported model(s), including instrument units and constraints, account currency, timestamped conversion inputs where needed, and configured freshness requirements. Do not add implicit FX parity or infer tick value from a symbol.
2. **Small supported-model slice:** implement one clearly bounded loss model end-to-end with exact `Decimal` arithmetic. If the repository cannot supply the required data, return a typed non-executable rejection rather than using the legacy distance formula for that model.
3. **Constraint pipeline:** apply risk budget and existing position/exposure/leverage constraints, then venue min/max/step constraints; round down to a valid step, reject below minimum, and recompute modeled loss after rounding.
4. **Auditability:** return stable typed rejection reasons. If the current public `RiskDecision` cannot carry the required structured audit without broad migration, stage that API change explicitly rather than hiding the decision in a free-form string.
5. **Boundary integration:** verify only approved/resized decisions can proceed to OMS construction; rejected/halted/stale/unsupported decisions never submit.
6. **Broader models only after prerequisites:** conversion direction, costs/buffers, margin, pending-order reserves, per-account independence, and nonlinear-model rejection must be individually tested before being described as supported.

## 4. Core implementation verification and remaining live-release gates

### A. Correctness and deterministic behavior
- [ ] Repeated identical request/snapshot inputs produce identical decision, quantity, reason code, and audit values; no network or wall-clock dependency inside the sizing calculation.
- [ ] Buy stop at or above entry and sell stop at or below entry reject when a stop is required; missing/zero/negative required inputs reject without panic.
- [ ] No float is used for authoritative financial calculations; quantity and modeled loss use exact decimal arithmetic.
- [ ] The selected loss model is explicit and typed. Unsupported or incomplete models reject; no model is inferred only from symbol text.
- [ ] Account-currency conversion is explicit, directionally correct, timestamped, and applied exactly once; missing/stale/ambiguous conversion rejects.
- [ ] Costs and configured slippage/gap buffers cannot cause approved quantity to increase.
- [ ] Final rounded quantity is a valid volume-step multiple and within venue min/max; rounding is downward only; below-minimum or minimum-notional failures reject rather than forcing a larger order.
- [ ] Recomputed worst-case modeled loss for the final quantity does not exceed the effective budget; all position/exposure/leverage/margin caps are independently applied where the necessary inputs exist.
- [ ] Kill switch, exhausted daily/drawdown budget, stale account/instrument/quote/margin data, and unknown netting semantics block new exposure when relevant to the supported slice.

### B. Isolation and execution safety
- [ ] Two accounts with different currency/equity/spec inputs are sized independently; no account's approval or quantity is copied to another account.
- [ ] Sizing code never submits orders or calls venue/network APIs.
- [ ] A rejected, halted, stale, unsupported, or invalid sizing decision cannot reach OMS/order submission; add a regression test at the nearest existing risk/execution boundary.
- [ ] Existing simulated/paper paths remain available; no live venue, live-mode flag, deployment, or money movement is enabled.

### C. Verification and reporting
- [x] Add targeted tests for the initial implemented slice: linear/tick-value calculations, conversion, stop geometry, stale data, tick alignment, minimum volume, costs/buffer, and evaluator gate order. CI result pending.
- [x] Added tests for exposure/margin/pending reserves, venue maximum, minimum notional, multiplier and account independence, deterministic generated sizing invariants, cache eligibility, and no-OMS behavior when audit storage is absent. Additional live-source contract tests belong to QOS-014/015.
- [x] Supported sizing behavior and fail-closed unknown venue/account-semantics cases have targeted coverage. QOS-014/015 must add real bridge/venue contract tests before any live-readiness claim.
- [x] Latest follow-up CI run `40445467228` passed `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, frontend typecheck and production build.
- [x] Updated `TASKS.md`, `PROGRESS.md`, and `REVIEWS.md` with evidence, corrections and live-release limitations.
- [ ] Do not claim production or live readiness from a green CI run alone.

## 5. First implementation slice now present

`crates/risk-engine/src/instrument_sizing.rs` adds `LossModel::Linear` and `LossModel::TickValue`, `InstrumentSizingContext`, typed rejection/limiting reasons, exact decimal calculations with checked operations, directionally supplied account-currency rates, explicit freshness status, side-aware stop validation, risk/position/exposure/leverage caps, volume-step round-down, min/max and minimum-notional validation, and post-rounding loss verification. `DefaultRiskEvaluator::evaluate_with_instrument_sizing` runs the existing risk gates before invoking this API. Eight focused tests were added. CI run 40435876274 caught an invalid sell-stop test fixture (entry 100, stop 101) that contradicted the rejection assertion; the fixture now uses equality (100/100), and REV-020 records the finding. Corrected-head CI run 40436418111 passed formatting, workspace check, tests, Clippy, frontend lint, typecheck, and production build.

The runtime now has a runtime-owned snapshot cache implementing `InstrumentSizingSnapshotProvider`; adapter/source implementations publish validated snapshots, and execution rejects missing/stale/future/out-of-order data, mismatched identity, unknown venue capabilities, unknown netting/hedging semantics, stale account state, or absent audit storage before OMS. The ordinary `execute_intent` compatibility path remains on legacy sizing and must not be used for live execution. The bridge and real venue-source implementation remain QOS-014/015; this is a verified venue-neutral core, not a live-readiness claim.

## 6. Remaining known risks

- Explicit risk-policy version IDs are not yet modeled; the audit stores the full policy contents and a sizing-model schema version instead. The current public `RiskDecision` still has a free-form resized reason, but the structured sizing audit records typed sizing limit/reject reasons. Real MT5 account/instrument/quote/conversion snapshots are not connected until QOS-014/015. QOS-011 still has no event-stream surface, so venue event deduplication/reconnect needs separate coverage. No live execution or production deployment is enabled.


## 7. Implementation slice status

Verified implementation: instrument-aware sizing, runtime-owned snapshot cache/provider, account/instrument identity and timestamp freshness checks, venue capability and position-semantics fail-closed gates, pending-order/margin constraints, structured persistent audit and no-store/no-OMS regression. Latest CI run `40445467228` passed Rust format/check/test/Clippy and frontend lint/typecheck/build. QOS-013 core is complete for the venue-neutral/paper runtime. QOS-014/015 remain mandatory before any real venue can publish production snapshots or be used live.
