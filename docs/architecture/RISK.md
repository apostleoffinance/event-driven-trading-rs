# Risk Engine

`crates/risk-engine` evaluates [`TradeIntent`](../domain)-derived [`RiskRequest`]s into
deterministic [`RiskDecision`]s.

## Owns

- Ordered pre-trade gate rules
- Quantity sizing / resize logic
- `RiskEvaluator` trait + `DefaultRiskEvaluator`

## Must NOT own

- Order submission / OMS
- Venue / broker HTTP
- Strategy implementations
- Persistence

## Evaluation order

```text
1. Global halt?
2. Account halt?
3. Strategy halt?
4. Instrument allowed?
5. Trading session allowed?
6. Drawdown breach?
7. Daily loss breach?
8. Position limit?
9–12. Size with exposure / leverage / trade-risk caps
     → APPROVE / RESIZE / REJECT
```

Each gate lives in its own module under `rules/` and is unit-tested in isolation.

## Determinism

Same `RiskRequest` ⇒ same `RiskDecision`. No clocks, RNG, or I/O in the decision path
(beyond values already present on the request).

## Flow

```text
TradeIntent
    ↓
RiskRequest  (+ AccountState + RiskPolicy)
    ↓
RiskEvaluator::evaluate
    ↓
RiskDecision { Approved | Resized | Rejected | Halted }
```
