# Reconciliation

`crates/reconciliation` compares internal engine state to venue state.

## Owns

- `InternalSnapshot` (caller-owned internal view)
- Pure compare (`compare_snapshots` / `CompareTolerances`)
- `Reconciler` (fetch venue → compare → emit events)
- Break taxonomy (`ReconciliationBreak`) and `ReconciliationReport`

## Must NOT own

- Strategy / risk / OMS mutation
- Silent overwrite of account, positions, or orders
- Venue HTTP (uses `VenueAdapter` reads only)
- Persistence schema

## Principle

**Alert only — never silently overwrite.**

Mismatches publish `TradingEvent::ReconciliationAlert`. Operators / later phases
decide remediation. Completed with alerts still means the compare finished;
`ReconciliationFailed` is reserved for fetch/IO failures.

## Lifecycle events

```text
ReconciliationStarted
        ↓
ReconciliationAlert*   (zero or more)
        ↓
ReconciliationCompleted
   or
ReconciliationFailed   (venue fetch error)
```

## What is compared

| Internal | Venue (`VenueAdapter`) |
|----------|------------------------|
| `AccountState` equity / balance / open_position_count | `account_state` |
| Position book (by instrument) | `positions` |
| Non-terminal OMS orders | `open_orders` |

AccountEngine does not own a position book — callers pass positions from the
runtime/store view they treat as internal truth.

## Runtime hook

`TradingRuntime::reconcile` builds an `InternalSnapshot` from account + OMS
open orders and the provided position vec, then delegates to `Reconciler`.
