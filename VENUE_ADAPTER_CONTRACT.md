# Venue Adapter Contract Specification (QOS-010)

Last updated: 2026-10-27. Specification only; this document does not claim that the current adapter implements these guarantees. `RULES.md` is authoritative.

## 1. Purpose and scope

Define a venue-neutral boundary for simulated, paper, and future live venues. The adapter translates venue-specific transport/API behavior into typed domain facts and explicit capabilities. It must not make account risk decisions, bypass risk authorization, or expose credentials to strategies or the frontend.

Current source baseline: `VenueAdapter` in `crates/venue-connectors/src/adapter.rs` provides `venue_name`, `account_state`, `submit_order`, `cancel_order`, `open_orders`, `positions`, and `get_order`. `OrderAck` has client/internal/venue order IDs, an accepted flag, immediate fills, and an optional message. `VenueError::Ambiguous` exists. The current trait does not expose capabilities, instrument lookup, health/readiness, event streaming, or a reconciliation snapshot contract. The current cancel bridge marks an order cancelled after adapter call success; that behavior must be corrected/tested before a live adapter relies on it.

## 2. Boundary invariants

1. **Risk before execution:** the adapter accepts only OMS-authorized order requests. The adapter is not a risk engine and must never authorize an intent.
2. **Account scoping:** every order, cancellation, event, snapshot, and reconciliation query must be attributable to the configured venue and exact account. Reject mismatched account/venue identity.
3. **Stable identity:** every submit carries a stable `ClientOrderId`. Repeating the same ID with the same canonical payload must return/recover the original outcome, not create a second order. Same ID with a different payload is a conflict.
4. **Ambiguity is first-class:** timeout, disconnect, or lost response after a request may have reached the venue yields `Unknown`/ambiguous, not a definite failure. Never blindly retry an ambiguous submit.
5. **Acknowledgement is not terminal truth:** submit acknowledgement, cancel-request acknowledgement, and terminal order state are distinct. A cancellation request is not `Cancelled` until an authoritative venue event or query confirms terminal cancellation.
6. **External facts are reconciled:** venue events and snapshots are evidence to validate and append to the internal record. Do not silently overwrite conflicting OMS state.
7. **Financial precision:** quantities, prices, fees, balances, and P&L use exact decimal domain values. Do not parse authoritative values through binary floating point.
8. **Freshness/fail-closed:** missing, stale, malformed, inconsistent, or unavailable account/instrument/health state blocks new exposure. Read-only recovery/reconciliation remains possible.
9. **No hidden live mode:** the adapter reports observed capabilities and connection state; configuration/eligibility checks outside the adapter still gate live execution.

## 3. Proposed adapter surface

These are logical requirements, not finalized Rust signatures. Prefer small typed structures over booleans or vendor-specific JSON at the domain boundary.

| Surface | Required behavior |
|---|---|
| Identity | Stable venue identifier; account IDs must be explicit in all account-scoped operations. |
| Capabilities | Return a typed snapshot of supported order types, time-in-force values, client-ID idempotency semantics, cancel support, event-stream support, position/order snapshots, instrument-spec availability, and any venue-enforced protection/close capabilities. Unknown capability is unsupported for safety-sensitive use, not assumed true. |
| Health/readiness | Return connection state (connected/degraded/disconnected/unknown), observation timestamp, last successful request/event, optional latency/rate-limit/backoff metadata, and a separate readiness decision/reasons. “Connected” alone does not mean trading-ready. |
| Instrument specification | Fetch a typed `Instrument`/`InstrumentSpec` or equivalent for a venue+instrument. Include tick size, lot/volume step, min/max quantity, precision, minimum notional where applicable, and instrument-specific contract/tick/pip value and currency-conversion metadata when required by sizing. Existing `InstrumentSpec` currently has tick size, lot size, min quantity, optional min notional, and precision; it does not yet represent every FX/CFD contract/conversion rule. Missing required metadata blocks sizing/submission. |
| Account snapshot | Return typed balance/equity/margin/available funds/status with source timestamp and freshness. Reject mismatched account identity. |
| Submit | Accept a validated OMS order with stable client ID and canonical payload. Return a typed acknowledgement/outcome that distinguishes accepted, rejected, and unknown; include venue order ID when known, event/source timestamp, and any fills. Never collapse a transport ambiguity into rejection. |
| Cancel | Return a typed request result (requested/accepted/rejected/unknown) rather than implying terminal cancellation. Terminal status is confirmed by an event or a subsequent authoritative query. |
| Order/fill events | Provide an async stream of normalized immutable facts. Each event includes venue/account identity, event type, event ID or deduplication key where available, client/internal/venue order IDs as applicable, source timestamp, receive timestamp, sequence/cursor when available, schema version, and correlation ID. Preserve partial fills and corrections; duplicate delivery must be safe. |
| Queries | Account-scoped open orders, positions, and order lookup. Define lookup behavior for an unknown client ID and distinguish “not found” from “query failed”. |
| Reconciliation | Support snapshots for open orders, positions, and relevant account state, with snapshot time and completeness/continuation metadata if paginated. Queries must make it possible to compare venue state to OMS state after restart or stream gaps. |
| Operational limits | Surface rate limits, retry-after/backoff hints, pagination and sequence-gap information. Transport retry policy must not duplicate non-idempotent order submissions. |

## 4. Typed outcomes and error semantics

The final Rust API may use enums or structured result types, but it must preserve these meanings:

- **Accepted:** venue explicitly acknowledges the request; this does not necessarily mean filled.
- **Rejected:** venue or local validation definitively rejects the request, with a stable reason category and sanitized diagnostic.
- **Unknown:** request may have reached the venue but no authoritative result is available. Preserve the client ID and reconcile before any retry.
- **Cancel requested:** cancellation was accepted as a request; order may still be open, partially filled, or filled.
- **Terminal confirmed:** a venue event or authoritative query confirms filled, cancelled, expired, or rejected terminal state.
- **Not found:** authoritative query says the identifier is absent within the queried scope; this alone does not prove a prior ambiguous submit never arrived unless venue semantics guarantee that conclusion.
- **Unavailable/stale:** required state cannot be trusted now; block new exposure.

Errors must be typed sufficiently for callers to choose safe behavior. Free-form vendor text may be attached for diagnostics but must not be the sole machine-readable classifier. Secrets and sensitive account payloads must be redacted from logs.

## 5. Event ordering, replay, and reconnect

- Treat streams as at-least-once unless the provider contract proves stronger semantics.
- Deduplicate by venue-provided event ID or a documented stable composite key. If neither is reliable, persist enough source identity to detect likely duplicates and flag uncertain duplicates for reconciliation rather than dropping financial facts silently.
- Track sequence/cursor when the venue provides one. On gap, disconnect, parser failure, or suspected event loss, mark stream health degraded and request reconciliation snapshots before declaring state current.
- Do not assume event order across separate streams. Apply only valid lifecycle transitions; retain contradictory/out-of-order facts and trigger reconciliation.
- On reconnect, re-subscribe, capture the new stream cursor if supported, query orders/positions/account, then reconcile stream events and snapshots. Do not mark ready merely because the socket reconnects.
- Events should be immutable facts. Corrections or busts are new facts linked to the original fill/event, not silent mutation.

## 6. Instrument and order validation

Before venue submission, callers must validate the order against a fresh venue-specific instrument spec and supported capabilities. Validate positive quantity/price, tick and lot increments, minimum quantity/notional, quantity precision, order type, time in force, and any venue-specific constraints. Normalize only under an explicit documented rounding policy; default is reject, not silently round. The risk engine must size using instrument-aware loss per unit, stop distance, contract/tick/pip value, quote-currency conversion, volume step/min/max, exposure and account limits. Venue validation is a final constraint check, not a substitute for risk authorization.

## 7. Contract-test acceptance matrix

QOS-011 implementation is not ready until the contract and this test matrix are reviewed. Every adapter implementation (including simulator) should run the reusable contract suite with injected transport behavior.

| ID | Scenario | Required assertion |
|---|---|---|
| VA-01 | Capability discovery | Unsupported order type/TIF/cancel feature is reported explicitly and rejected before submit. |
| VA-02 | Account/venue mismatch | Request and returned event/snapshot with mismatched identity are rejected and do not mutate OMS state. |
| VA-03 | Duplicate submit, same client ID + same payload | Returns/reconciles the original order outcome; creates no duplicate external order. |
| VA-04 | Duplicate client ID, changed payload | Returns conflict; second payload is never submitted. |
| VA-05 | Definitive pre-acceptance rejection | OMS records rejected/failed according to the documented lifecycle; no retry loop. |
| VA-06 | Timeout before request is sent | Classified definite failure only when transport can prove it was not sent. |
| VA-07 | Timeout/disconnect after request may have been sent | State is Unknown; no blind retry; reconciliation by client ID/order snapshots is attempted. |
| VA-08 | Submit ack lost, later order found | Reconciliation links the venue order to the original client ID and resolves Unknown without duplicate submit. |
| VA-09 | Cancel request acknowledged but order remains open | OMS does not mark Cancelled; open state persists until terminal confirmation. |
| VA-10 | Partial fill then cancel | Filled quantity and fees remain recorded; remaining quantity transitions only after venue confirmation. |
| VA-11 | Duplicate/out-of-order fill event | Fill is not double-applied; invalid transition is surfaced and reconciliation requested. |
| VA-12 | Stream disconnect/sequence gap | Health becomes degraded/stale; new exposure is blocked until recovery and reconciliation criteria pass. |
| VA-13 | Stale/missing instrument spec | New exposure is rejected; no default spec is substituted for a live instrument. |
| VA-14 | Invalid tick/lot/minimum/precision | Order rejected before network submission; exact decimal checks are used. |
| VA-15 | Partial/paginated reconciliation snapshot | Snapshot is not treated as complete until all pages/continuation markers are resolved. |
| VA-16 | Venue state conflicts with OMS | Discrepancy is preserved, surfaced, and reconciled; no silent overwrite. |
| VA-17 | Rate limit / retry-after | Backoff is honored; submit is not retried unless idempotency/outcome semantics make retry safe. |
| VA-18 | Restart with Unknown orders | Recovery queries venue and resolves state without creating new client IDs or duplicate orders. |

### Test harness requirements

- Provide deterministic fake transport responses for reject, timeout-before-send, timeout-after-send, delayed acknowledgement, disconnect, duplicate events, sequence gaps, partial fills, rate limits, and conflicting snapshots.
- Assert both adapter output and OMS/persistence effects; a return-value-only test is insufficient for lifecycle/idempotency cases.
- Use exact decimal assertions. No test may depend on a live account or real money.
- Simulator contract tests establish local semantics only; provider-specific certification and paper testing remain required for a real adapter.

## 8. Implementation sequencing and exclusions

1. Review this contract and test matrix.
2. Add typed domain/connector types and reusable contract-test harness.
3. Make OMS cancel semantics represent request versus terminal confirmation; add reconciliation hooks.
4. Extend the simulator to exercise ambiguity, duplicate events, partial fills, stale state, and recovery.
5. Only then implement one selected live adapter after account access, deployment, permissions, and paper test path are confirmed.

Out of scope for QOS-010: implementing any venue adapter, adding a live mode, changing sizing logic, broad dependency upgrades, frontend changes, multi-account orchestration, or moving money.

## 9. Exit criteria

- [ ] Existing source surface and gaps are accurately documented.
- [ ] Capability, health/readiness, instrument, event, submit/cancel, and reconciliation semantics are explicit.
- [ ] Ambiguous outcomes, idempotency, duplicate/out-of-order events, stale state, and confirmed cancellation are covered.
- [ ] Contract-test matrix is concrete and testable.
- [ ] `TASKS.md`, `PROGRESS.md`, and `REVIEWS.md` are consistent.
- [ ] No runtime/trading behavior changed and no live execution enabled.
