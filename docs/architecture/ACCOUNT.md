# Account Engine

`crates/account-engine` owns account registry, live state, snapshots, and
account-level controls.

## Owns

- Account registration / lookup
- `AccountState` updates (equity, margin, PnL, position count)
- Status controls (halt / resume / suspend / close)
- Kill-switch controls
- `AccountSnapshot` capture
- Simulated (and prop-on-simulated) account helpers

## Must NOT own

- Venue HTTP / broker SDKs
- Order submission
- Strategy logic
- Risk rule evaluation (consumes `AccountState` only)

## Account ≠ Venue

```text
Account: prop-account-001   (capital / risk ownership)
Venue:   simulated          (where execution occurs)
```

## Simulated accounts

`InMemoryAccountEngine::open_simulated` creates `AccountType::Simulated`
bound to venue id `simulated`.

`open_prop_on_simulated` creates `AccountType::Prop` still on the simulated
venue — live prop connectors arrive in Phase 11.

Vault account types are rejected until a future phase.
