# Account Engine

`crates/account-engine` owns account registry, live state, snapshots, and
account-level controls.

## Owns

- Account registration / lookup
- `AccountState` updates (equity, margin, PnL, position count)
- Status controls (halt / resume / suspend / close)
- Kill-switch controls
- `AccountSnapshot` capture
- Simulated and prop account helpers

## Must NOT own

- Venue HTTP / broker SDKs
- Order submission
- Strategy logic
- Risk rule evaluation (consumes `AccountState` only)

## Account ≠ Venue

```text
Account: prop-account-001   (capital / risk ownership)
Venue:   prop               (where execution occurs)
```

## Account helpers

| Helper | AccountType | Venue id |
|--------|-------------|----------|
| `open_simulated` | Simulated | `simulated` |
| `open_prop` | Prop | `prop` |
| `open_prop_on_simulated` | Prop | `simulated` (legacy dual-path) |

Vault account types are rejected until a future phase.
