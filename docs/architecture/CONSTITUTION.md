# Quant OS Trading Systems Engineering Constitution

> **Foundational principle:** In a financial system, correctness is more important than
> availability, and **explicit uncertainty is safer than a confident guess**.
>
> Rust gives memory safety and concurrency guarantees. It does **not** by itself make a
> trading system financially safe. Quant OS requires software-safety rules + financial-domain
> rules + market-structure rules + operational controls.

Every Cursor / human implementation touching this repository must obey these rules.

---

## Absolute rules (non-negotiable)

| # | Rule |
|---|------|
| 1 | **Strategy never submits orders** — strategies emit `TradeIntent` only |
| 2 | **Risk always precedes OMS / execution** — no bypass path |
| 3 | **Account ≠ Venue** — capital/risk ownership is separate from connectivity/execution |
| 4 | **Money is exact** — `Decimal` / `NUMERIC` / string wire; never `f64` financial state |
| 5 | **Every external operation is treated as unreliable** |
| 6 | **Every order is idempotent** (`client_order_id`) |
| 7 | **Unknown execution state is a valid state** — never guess Failed after ambiguous submit |
| 8 | **External state must be reconcilable** — alert-only; no silent overwrite |
| 9 | **Historical financial events are immutable / auditable** |
| 10 | **Financial lifecycles use explicit state machines** |
| 11 | **Risk fails closed** when required information is unavailable |
| 12 | **No silent state correction** |
| 13 | **No secrets** in strategies, frontend, orders, or domain objects |
| 14 | **Backtest, paper, and live share the same strategy semantics** |
| 15 | **Frontend observes / controls; it never becomes financial truth** |

---

## Pipeline authority

```text
Market Data (+ MarketDataHealth)
     ↓
Strategy                    → TradeIntent only
     ↓
Compatibility / InstrumentSpec
     ↓
Risk                        → Approved | Resized | Rejected | Halted
     ↓
OMS                         → idempotent Order lifecycle
     ↓
Venue Adapter
     ↓
External Market
     ↓
Fills → Positions → Persistence
     ↕
Reconciliation (alert-only)
```

Each layer has exclusive authority. Lower layers cannot override higher risk limits.

Risk hierarchy:

```text
Firm → Portfolio → Account → Strategy → Trade
```

---

## Rust trading rules

```text
Prefer safe Rust; #![forbid(unsafe_code)] on core crates
No unwrap()/expect()/panic! in financial critical paths
Result<T, E> for recoverable failures
Explicit error taxonomy
Controlled concurrency — no global Mutex architecture
Deterministic state transitions
Strong domain types + explicit precision (InstrumentSpec)
Property-based / invariant tests for risk and accounting
Structured logging + correlation / causation IDs
Deterministic recovery after restart
```

---

## Required architectural concepts

### 1. `Unknown` execution state

After submit, if the venue response is ambiguous (timeout, disconnect), the order enters
`UNKNOWN`. Reconciliation resolves it. Never assume failure.

### 2. `InstrumentSpec`

Every tradable instrument defines tick size, lot size, min quantity, min notional,
price/quantity precision. Invalid orders are rejected **before** venue submission.

### 3. `ExecutionStateMachine`

Legal `OrderStatus` transitions only — enforced in domain (`can_transition_to`).

### 4. Audit / event store

Lifecycle facts (`TradeIntentCreated`, `Risk*`, `Order*`, fills, recon alerts) carry
`event_id`, `correlation_id`, and optional `causation_id` so a trade is reconstructable.

### 5. `MarketDataHealth`

Presence ≠ validity. Track freshness, sequence gaps, and source quality. Stale or gapped
data blocks new strategy risk creation (fail closed).

---

## Operational defaults

| Condition | Behavior |
|-----------|----------|
| Risk / account state unavailable | New orders **blocked** (`Halted`) |
| Market data stale / sequence gap | Strategy intents **suppressed** |
| Venue timeout after submit | Order → **Unknown** → reconcile |
| Internal vs venue mismatch | **ReconciliationAlert** only |
| Kill switch on | No new risk; open orders policy explicit |

---

## References in tree

- Domain: `crates/domain`
- Risk: `crates/risk-engine` + `docs/architecture/RISK.md`
- Execution: `crates/execution-engine` + `docs/architecture/EXECUTION.md`
- Reconciliation: `crates/reconciliation` + `docs/architecture/RECONCILIATION.md`
- Strategy bridge: `docs/architecture/STRATEGY_BRIDGE.md`
