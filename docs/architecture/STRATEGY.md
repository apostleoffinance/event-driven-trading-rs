# Strategy Runtime

## Crates

| Crate | Role |
|-------|------|
| `crates/strategy-runtime` | `Strategy` trait, `StrategyContext`, intent ID helper, event worker |
| `strategies/mean-reversion` | Stateful mean-reversion implementation |

## Owns

- Strategy interface (`on_market_data` → `Vec<TradeIntent>`)
- Deployment context binding
- Publishing `StrategySignalGenerated` / `TradeIntentCreated` from market envelopes

## Must NOT own

- Account risk limits / position sizing from balance
- Order submission / venue calls
- OMS / execution
- Persistence

## Strategy contract

```text
MarketDataEvent
      ↓
Strategy::on_market_data(&mut self, ctx, event)
      ↓
Vec<TradeIntent>   # no Order, no account sizing
```

Strategies may keep internal state (rolling windows).  
Strategies may set optional `entry_price`, `stop_loss`, `take_profit`, `confidence`.  
Strategies must leave `target_quantity` / account risk to the Risk Engine (Phase 4).

## Mean reversion

1. Warm-up until `window_size` prices are stored (no intents)
2. Compare new price to mean of the existing window
3. If deviation > threshold → Buy (below) / Sell (above) `TradeIntent`
4. Slide the new price into the window

Legacy paper-demo strategy under `crates/event-trading` is deprecated for new work.
