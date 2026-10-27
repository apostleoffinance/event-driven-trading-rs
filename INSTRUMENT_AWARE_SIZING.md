# Instrument-Aware Risk Sizing Specification (QOS-012)

Last updated: 2026-10-27. Specification only. This document describes target behavior and does not claim the current risk engine implements it. `RULES.md` is authoritative.

## 1. Purpose and current source baseline

Define deterministic, account-specific position sizing for FX, CFDs, crypto, and other supported instruments before multi-account allocation is implemented. Sizing must calculate a quantity from a monetary risk budget and instrument loss mechanics, then apply all account, portfolio, strategy, and venue constraints. Equity percentage alone or price-distance division alone is not a universal sizing formula.

Source inspected:
- `crates/risk-engine/src/sizing.rs` currently caps quantity using requested quantity/notional, equity-derived position/exposure/leverage limits, and a trade-risk cap based on `equity × risk_pct ÷ abs(entry - stop)`.
- `domain::RiskRequest` currently contains account state, policy, entry price, stop loss, requested quantity/notional, and current exposure, but no instrument spec, account-currency conversion, contract multiplier, tick/pip value, margin requirements, or market-data freshness fields.
- `domain::InstrumentSpec` currently includes tick size, lot size, minimum quantity, optional minimum notional, and price/quantity precision. It does not represent all FX/CFD contract and currency-conversion rules.
- The current risk API returns Approved/Resized/Rejected decisions with a quantity; this spec does not prescribe an immediate public API migration.

## 2. Non-negotiable safety invariants

1. **Per-account calculation:** size each account independently from that account's equity, currency, open exposure, existing risk, policy, margin, and venue constraints. Never copy a source account's lot size or approval to another account.
2. **Risk before OMS:** sizing and risk authorization complete before order submission. Strategies only emit intent; they cannot override sizing or authorization.
3. **Exact arithmetic:** all authoritative prices, quantities, money, conversion rates, and risk calculations use decimal types. Wire money/decimal values are strings. Never use binary floating point for authoritative calculations.
4. **Fail closed:** missing, malformed, stale, inconsistent, unsupported, or non-positive required data rejects new exposure with a typed reason. Never substitute a generic instrument default for an unknown live instrument.
5. **Stop required for risk-budget sizing:** when a policy requires bounded loss per trade, missing/invalid stop or unsupported loss model rejects. An equity/exposure cap does not replace stop-loss risk sizing.
6. **Side-aware stop validation:** a long/buy stop-loss must be below entry; a short/sell stop-loss must be above entry unless an explicitly reviewed product semantics says otherwise. Zero/negative prices and zero stop distance reject.
7. **Round down only:** round quantity down to the venue volume step; never round up to fit minimum size. If rounded quantity is below venue minimum or minimum notional, reject rather than exceed budget.
8. **Constraints intersect:** final quantity cannot exceed any independent cap (risk budget, position, total exposure, leverage, margin, concentration, drawdown/kill-switch policy, strategy/account limit, instrument max, venue max). Apply the most restrictive cap.
9. **Costs and gap risk:** where supported and configured, include estimated commissions, spread, slippage, financing/swap and other per-trade costs in the risk budget. Stop losses do not guarantee execution at stop price; document residual gap/slippage risk and optional safety buffer.
10. **Auditable decision:** persist or emit the policy version, input snapshot references/timestamps, conversion path, calculation model/version, unrounded quantity, rounded quantity, every limiting cap, rejection/resize reason, and final risk decision.
11. **No false guarantees:** do not promise exact realized loss, guaranteed stop execution, profitability, or loss-free execution.
12. **Stale state blocks exposure:** stale price, account, instrument spec, conversion rate, margin data, or open exposure invalidates a new-risk calculation according to configured age limits.

## 3. Required input contract

Every calculation must identify an exact account, venue, instrument, side, entry/mark/reference price, stop-loss, and calculation timestamp. Required inputs:

| Input group | Required fields | Validation / failure behavior |
|---|---|---|
| Identity | Account ID, venue ID, instrument ID, side | All must match the order and current account/venue configuration. |
| Account state | Equity, balance as needed, account currency, available funds/free margin, current margin, status, observation time | Reject non-positive equity, ineligible/disabled account, stale or inconsistent snapshot, or unavailable currency. |
| Risk budget | Policy version, max risk per trade, remaining daily loss/drawdown budget, strategy/account/portfolio caps | Validate non-negative budget and enforce kill switches and policy caps before sizing. |
| Trade geometry | Entry/reference price, stop-loss, order side, stop type/semantics | Positive prices, correct stop direction, positive distance, same instrument/price units. |
| Instrument mechanics | Instrument type, contract size/multiplier, quantity unit, tick size, tick value or loss-per-unit model, price precision, volume step, min/max quantity, minimum notional, quote/base/settlement currencies | Venue-specific, current, and complete for the selected model. Missing required fields reject. |
| Currency conversion | Currency pair/path, rate(s), direction, source timestamp, freshness bound, conversion method | Rate positive and fresh; conversion direction explicit. Missing or ambiguous path rejects. |
| Exposure and margin | Existing positions/orders exposure, current risk where available, leverage/margin schedule, concentration limits, pending order reservations | Consistent account-scoped snapshot; reserve exposure for open/pending orders to prevent over-allocation. |
| Costs and execution buffer | Fee schedule, spread/slippage assumptions, financing if relevant, configured safety buffer | Model and units explicit; if policy requires a cost input and it is unavailable, reject. |
| Venue constraints | Supported order type/TIF, volume step/min/max, price tick/precision, min notional, margin/close-out constraints | Must agree with fresh venue capabilities and instrument specification. |

Inputs must include timestamps and source identifiers sufficient to establish freshness and audit the decision. The precise freshness thresholds are configuration, not hardcoded global constants; unknown thresholds must not imply data is fresh.

## 4. Loss models and formulas

Choose a typed, explicit loss model per instrument. Do not infer model solely from symbol text.

### 4.1 Tick-value model (common FX/CFD/futures style)

For one lot/contract:
- `stop_ticks = abs(entry_price - stop_price) / tick_size`, only when the distance is an exact multiple or the provider supplies a conservative normalized tick count.
- `gross_loss_per_lot_in_quote = stop_ticks × tick_value_per_lot_in_quote`.
- Convert that loss to account currency using a fresh, directionally correct conversion rate.
- `loss_per_lot_account = gross_loss_account + estimated_cost_per_lot + configured_slippage/gap_buffer_per_lot`.
- `risk_budget = min(equity × max_risk_pct, remaining policy loss budget, explicitly configured absolute budget)`.
- `raw_lots = risk_budget / loss_per_lot_account`.

Tick value may already be denominated in account currency or may depend on current price, contract, settlement currency, and conversion path. Its currency and calculation semantics must be typed so conversion is applied exactly once. If tick value is dynamic, use the provider's valid current calculation or a documented conservative bound.

### 4.2 Linear quantity model (e.g. supported spot instruments)

For a validated linear instrument with quantity expressed in base units:
- Determine the account-currency loss for one quantity unit at the stop using the instrument's contract multiplier and a directionally correct quote/settlement-to-account conversion.
- Add applicable per-unit costs/buffer.
- `raw_quantity = risk_budget / loss_per_unit_account`.

The conversion model must match the instrument's payoff/settlement mechanics. A simplistic `abs(entry - stop)` division is valid only when one quantity unit's price P&L is expressed in the same currency as the risk budget and the unit multiplier is exactly one.

### 4.3 Unsupported or nonlinear models

Options, inverse contracts, quanto contracts, tiered margin, nonlinear payoffs, and instruments with unavailable/ambiguous conversion mechanics must use a dedicated validated model or be rejected. Do not route them through the linear formula by default.

## 5. Constraint and rounding pipeline

Recommended deterministic order:

1. Validate identity, account eligibility, policy/kill switches, freshness, units, and all required data.
2. Validate entry/stop geometry and select the explicit instrument loss model.
3. Compute the account-currency risk budget after daily/drawdown and reserved-risk constraints.
4. Compute conservative per-unit/per-lot loss including configured costs and buffer.
5. Derive raw quantity from risk budget.
6. Derive independent caps for per-position notional, remaining total exposure, leverage, margin/free funds, strategy/account/portfolio concentration, current open and pending orders, and venue max volume.
7. Intersect all caps with the raw risk-based quantity.
8. Round down to the exact venue volume step using decimal arithmetic; do not round to a display precision as a substitute for volume-step rounding.
9. Validate min/max quantity, minimum notional, precision, tick/price constraints, order type and venue capability. Reject if final quantity is below minimum or fails any constraint; never increase it to meet minimum.
10. Recompute worst-case modeled loss from the final rounded quantity and assert it is no greater than the effective budget, allowing only explicitly defined decimal rounding tolerance (default zero tolerance).
11. Return a typed decision and an audit record containing inputs, model, caps, rounded quantity, modeled loss, timestamps, and reasons. Any failed invariant rejects.

### Exposure and margin semantics

- Exposure/notional must be converted to a documented common currency before comparing with equity-based limits.
- Pending orders reserve worst-case exposure/risk until definitively rejected, cancelled, expired, or filled and represented in current position state.
- Leverage limits are not a substitute for stop-loss risk limits; both must apply.
- Margin is venue-specific and can change. Missing/stale margin rules or insufficient free margin reject.
- Existing position risk must be included in remaining-risk budgets where policy requires aggregate open-risk control.
- For hedged/netted accounts, model netting, gross exposure, and margin behavior explicitly. Do not assume offsetting positions eliminate risk.

## 6. Output contract

A sizing result should distinguish at least:
- Approved quantity;
- Resized quantity with one or more limiting reasons;
- Rejected with stable typed reason(s), including invalid geometry, missing/stale input, unsupported model, conversion unavailable, risk budget exhausted, exposure/leverage/margin limit, instrument constraint, or below-minimum volume.

For auditability, a result should carry a structured calculation record:
- Account/venue/instrument IDs and side;
- policy and calculation-model versions;
- snapshot IDs/timestamps/freshness outcomes;
- account currency and conversion path/rates;
- risk budget, estimated costs/buffer, per-unit loss, raw quantity;
- each independent cap and its source;
- rounded final quantity and recomputed modeled loss;
- final decision and stable reason codes.

Do not rely on a free-form reason string as the sole machine-readable output. Existing public `RiskDecision` may require a staged migration to support multiple reasons and structured audit data; that migration is outside this specification task.

## 7. Acceptance and test matrix

QOS-013 implementation must include deterministic unit and property/invariant tests. Use synthetic fixtures only; no live accounts or real money.

| ID | Scenario | Required assertion |
|---|---|---|
| SZ-01 | Same risk inputs repeated | Identical decision and quantity; no wall-clock/network dependence beyond injected snapshots. |
| SZ-02 | Missing stop when policy requires bounded risk | Reject; position/exposure cap alone cannot approve. |
| SZ-03 | Buy stop at/above entry | Reject invalid stop geometry. |
| SZ-04 | Sell stop at/below entry | Reject invalid stop geometry. |
| SZ-05 | Zero/negative entry, stop, equity, tick value, conversion rate | Reject with typed reason; no panic. |
| SZ-06 | Tick-value FX instrument | Quantity reproduces hand-calculated account-currency risk within exact decimal arithmetic. |
| SZ-07 | Quote currency differs from account currency | Correct directional conversion is applied exactly once. |
| SZ-08 | Conversion stale/missing/ambiguous | Reject; no implicit parity conversion. |
| SZ-09 | Contract multiplier changes | Quantity changes inversely with modeled loss per unit as expected. |
| SZ-10 | Nonlinear/unsupported instrument model | Reject unless dedicated model explicitly registered. |
| SZ-11 | Spread/commission/slippage buffer increases | Approved quantity never increases; recomputed modeled loss stays within budget. |
| SZ-12 | Round down to volume step | Quantity is a valid step multiple and is never rounded up. |
| SZ-13 | Raw quantity rounds below minimum | Reject; do not force minimum lot. |
| SZ-14 | Quantity exceeds venue maximum | Resize down to valid maximum/step while preserving risk budget. |
| SZ-15 | Minimum notional fails after rounding | Reject without increasing quantity. |
| SZ-16 | Exposure cap lower than risk-based size | Final size respects exposure cap. |
| SZ-17 | Margin/free-funds cap lower than risk-based size | Final size respects margin and free-funds constraints. |
| SZ-18 | Pending order reserves capacity | Second calculation sees reserved exposure/risk; no over-allocation. |
| SZ-19 | Two accounts have different currencies/specs/equity | Independent quantities and conversions; no copied approval/lot size. |
| SZ-20 | Stale account, instrument, quote, or margin data | Reject new exposure. |
| SZ-21 | Decimal boundary/rounding | No binary float; recomputed modeled loss does not exceed budget. |
| SZ-22 | Kill switch or remaining daily/drawdown budget exhausted | Reject before any sizing result is executable. |
| SZ-23 | Hedged/netted account semantics unknown | Reject or use explicit account-specific model; never assume risk offsets. |
| SZ-24 | Venue capabilities/order type unsupported | Reject before OMS submission. |
| SZ-25 | Missing audit inputs/model version | Do not mark decision as production-ready; persist a structured reason/failure. |
| SZ-26 | Randomized/property-generated valid and invalid specs | All outputs satisfy positivity, step, min/max, budget, and cap invariants. |

## 8. Dependencies and sequencing

1. Review this specification and the QOS-010 venue contract; the venue contract defines how fresh instrument specs, capabilities, and connection state are obtained.
2. Implement a typed instrument-loss model and required sizing inputs without silently changing existing behavior for unsupported instruments.
3. Add deterministic decimal helpers for step rounding, conversion, and worst-case loss verification.
4. Add unit/property tests for the matrix above, plus tests at the OMS/risk boundary proving rejected/unknown/stale decisions cannot submit.
5. Only after QOS-012 and tests are reviewed should QOS-013 allocator work proceed.
6. QOS-011 adapter implementation remains a separate workstream; sizing cannot depend on a specific vendor SDK.

Out of scope: implementing the sizing engine, changing current risk behavior, choosing live venues, connecting accounts, multi-account order fan-out, frontend changes, or money movement.

## 9. Exit criteria

- [ ] Existing sizing and domain input gaps accurately documented.
- [ ] Explicit per-account inputs and output/audit contract defined.
- [ ] FX/CFD tick-value, linear quantity, and unsupported/nonlinear models distinguished.
- [ ] Conversion, costs, exposure/margin caps, pending-order reserves, volume-step rounding, and fail-closed behavior specified.
- [ ] Test matrix is concrete and invariant-driven.
- [ ] `TASKS.md`, `PROGRESS.md`, and `REVIEWS.md` reflect verified status.
- [ ] No runtime/risk code changed and no live execution enabled.
