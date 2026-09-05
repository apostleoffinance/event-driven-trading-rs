# Event Trading / Quant Trading Engine

Rust modular monolith for systematic / prop trading infrastructure.

> Migrating from a paper-trading demo into production-oriented quantitative infrastructure.  
> Baseline tag: `v0.1-paper-engine` · Branch: `refactor/trading-infrastructure`

## Workspace

```text
crates/
  domain/           # shared trading primitives (Phase 1)
  events/           # typed async events over Tokio mpsc (Phase 2)
  strategy-runtime/ # Strategy trait + intent worker (Phase 3)
  risk-engine/      # deterministic pre-trade risk (Phase 4)
  event-trading/    # legacy paper engine (preserved)
strategies/
  mean-reversion/   # stateful mean reversion → TradeIntent
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
4. Events are immutable facts over in-process async channels (`crates/events`)

See `docs/migration/AUDIT.md`, `docs/architecture/DOMAIN.md`, `docs/architecture/EVENTS.md`, `docs/architecture/STRATEGY.md`, and `docs/architecture/RISK.md`.

## License

MIT
