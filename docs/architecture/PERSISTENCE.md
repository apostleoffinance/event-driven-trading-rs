# Persistence

`crates/persistence` stores trading state in PostgreSQL via SQLx.

## Owns

- Schema migrations (`migrations/`)
- `TradingStore` repositories for critical entities
- NUMERIC ↔ `rust_decimal::Decimal` mapping

## Must NOT own

- Strategy logic
- Risk evaluation
- Venue HTTP
- Runtime orchestration

## Critical entities (restart-safe)

- accounts / account_states
- venues / instruments
- strategies / deployments / risk profiles
- trade_intents
- risk_decisions
- orders (incl. `client_order_id` uniqueness)
- fills
- positions
- audit_events

## Local database

```bash
docker compose up -d
export DATABASE_URL=postgres://trading:trading@127.0.0.1:15432/trading
cargo test -p persistence
```

Money is always `NUMERIC(38,18)` — never `float`/`double`.
