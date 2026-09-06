# Trading Runtime

`crates/trading-runtime` is the continuous orchestrator for the modular pipeline.

## Owns

- Session wiring (`RuntimeConfig`, `TradingRuntime`)
- Continuous market-data loop (`run` / `run_n` / `run_until`)
- Intent pipeline: risk → OMS → venue → fills → positions → account sync
- Optional persistence hooks via `TradingStore`
- Outbound `TradingEvent` publication for observers

## Must NOT own

- Strategy signal logic (uses `strategy-runtime` / strategy crates)
- Risk rule evaluation (uses `risk-engine`)
- Venue HTTP / fill simulation internals (uses `VenueAdapter`)
- SQL schema / migrations (uses `persistence`)

## Pipeline

```text
MarketDataReceived
        ↓
Strategy (TradeIntent)
        ↓
RiskCheck → RiskApproved | RiskResized | RiskRejected | TradingHalted
        ↓ (if executable)
OMS + VenueAdapter → Order* / fills
        ↓
PositionOpened + AccountUpdated
```

Market data arrives on an inbound `EventSubscriber`. Pipeline facts are published
on a separate outbound `EventPublisher` so a single task never deadlocks on a
full mpsc channel while also being the only consumer.

## Usage sketch

```rust
let mut runtime = TradingRuntime::new(config);
runtime.bootstrap_prop_on_simulated(&venue, capital).await?;
runtime
    .run(&mut market_in, &mut strategy, &ctx, &venue, &events_out)
    .await?;
```

## Acceptance (Phase 9)

Continuous loop test: market ticks → mean-reversion intents → risk → simulated
venue fills → open position, with outbound event vocabulary coverage.
