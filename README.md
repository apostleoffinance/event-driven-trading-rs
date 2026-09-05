# Event Trading / Quant Trading Engine

Rust modular monolith for systematic / prop trading infrastructure.

> Migrating from a paper-trading demo into production-oriented quantitative infrastructure.  
> Baseline tag: `v0.1-paper-engine` · Branch: `refactor/trading-infrastructure`

## Workspace

```text
crates/
  domain/           # shared trading primitives (Phase 1)
  event-trading/    # legacy paper engine (preserved)
docs/
  architecture/
  migration/
```

## Quick start (legacy paper engine)

```bash
cargo run -p event-trading
cargo test --workspace
cargo run -p event-trading --bin test_all_exchanges
```

## Principles

1. Strategy emits **TradeIntent** — never submits orders
2. Risk Engine owns account limits / sizing / kill switches
3. **Account ≠ Venue**

See `docs/migration/AUDIT.md` and `docs/architecture/DOMAIN.md`.

## License

MIT
