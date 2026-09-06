# Strategy Runtime

## Language split

| Layer | Language | Location |
|-------|----------|----------|
| Strategy signals | **Python** | `strategies/python/` |
| Intent ingest + execution | **Rust** | `crates/intent-bridge` + trading stack |

See `STRATEGY_BRIDGE.md` for the JSON contract and pipe workflow.

## Rust crates (still present)

| Crate | Role |
|-------|------|
| `crates/strategy-runtime` | Legacy in-process Rust `Strategy` trait (Phase 3) |
| `strategies/mean-reversion` | Rust reference strategy — prefer Python for new work |

## Owns (strategy layer)

- Signal interface → `TradeIntent` only
- Rolling / research state inside the strategy process

## Must NOT own

- Account risk limits / position sizing from balance
- Order submission / venue calls
- OMS / execution
- Persistence

## Contract

```text
Market data / features
      ↓
Python strategy
      ↓
TradeIntent NDJSON
      ↓
Rust intent-bridge → Risk → OMS → Venue
```

Strategies may set optional `entry_price`, `stop_loss`, `take_profit`, `confidence`.  
Strategies must leave account risk sizing to the Risk Engine.
