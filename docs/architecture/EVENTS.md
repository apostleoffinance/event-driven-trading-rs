# Events Architecture

`crates/events` provides typed, immutable, in-process trading events over Tokio `mpsc`.

## Owns

- `TradingEvent` state-transition vocabulary
- `EventEnvelope` (event id + timestamp + payload)
- `MarketDataEvent` (venue-agnostic market data fact)
- `EventPublisher` / `EventSubscriber` channel pair
- `EventsError`

## Must NOT own

- Strategy logic
- Risk evaluation
- Venue HTTP / broker SDKs
- Persistence
- Distributed brokers (Kafka, NATS, Redis)

## Transport

```text
EventPublisher  --mpsc-->  EventSubscriber
```

- Bounded channel (default capacity 1024)
- Cloneable publishers, single consumer per channel
- Failures surface as `EventsError::ChannelClosed` / `Publish` — no panics

## Core pipeline (Phase 2 acceptance)

```text
MarketDataReceived
        ↓
StrategySignalGenerated   (optional signal metadata)
        ↓
TradeIntentCreated
```

Later phases consume the same vocabulary for risk, OMS, fills, positions, and reconciliation.

## Immutability & correlation

Events are facts: do not mutate an envelope after publish.

Correlate with domain IDs carried on payloads:

- `trade_intent_id`
- `order_id` / `client_order_id`
- `account_id` / `deployment_id` / `strategy_id`
- `venue_id` / `instrument_id`

## Legacy note

The sync `Mutex` pub/sub bus in `crates/event-trading` remains for the paper demo.
New code should use `crates/events` via `crates/trading-runtime`.
