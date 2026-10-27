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


## Instrument-aware sizing entry point (QOS-013)

`TradingRuntime::execute_intent_with_instrument_sizing` is an explicit opt-in alternative to `execute_intent`. It requires `RuntimeInstrumentSizingContext`, which binds a caller-supplied `InstrumentSizingContext` to an account ID and instrument ID. A mismatch or stale context rejects before OMS order creation; the method does not fall back to legacy sizing. The caller still owns the responsibility to supply trusted, current snapshot inputs. This API does not imply production or live readiness.
