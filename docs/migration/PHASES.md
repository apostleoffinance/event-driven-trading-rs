# Migration Phases

Branch: `refactor/trading-infrastructure`  
Baseline tag: `v0.1-paper-engine`

| Phase | Status | Objective |
|-------|--------|-----------|
| 0 | **Done** | Audit, tag, baseline `cargo check/test/clippy` |
| 1 | **Done** | Workspace + `crates/domain` |
| 2 | **Done** | Typed async events (`crates/events`) |
| 3 | **Done** | Strategy runtime + mean-reversion → `TradeIntent` |
| 4 | **Done** | Deterministic risk-engine |
| 5 | Pending | Account engine |
| 6 | Pending | OMS / execution lifecycle + idempotency |
| 7 | Pending | `VenueAdapter` + `SimulatedVenue` |
| 8 | Pending | PostgreSQL persistence |
| 9 | Pending | Continuous `trading-runtime` |
| 10 | Pending | Reconciliation |
| 11 | Pending | First prop venue connector |

## Phase 1 acceptance

- [x] Workspace builds with `crates/domain` + `crates/event-trading`
- [x] Domain models: Account, Venue, Instrument, Strategy, Deployment, TradeIntent, Risk*, Order, Fill, Position
- [x] Domain errors via `thiserror` (`DomainError` / `DomainResult`)
- [x] Financial values use `rust_decimal::Decimal` only
- [x] Architecture scenario test: Strategy → Deployment → Account → RiskRequest/Decision
- [x] `cargo fmt/check/test/clippy --workspace` green (verified at phase close)

## Phase 2 acceptance

- [x] `crates/events` with `TradingEvent` + `EventEnvelope`
- [x] Tokio `mpsc` publisher/subscriber (no Kafka/NATS)
- [x] `EventsError` / `EventsResult` — no unwrap in library paths
- [x] Flow test: `MarketDataReceived` → `TradeIntentCreated`
- [x] Legacy sync bus retained but documented as deprecated for new work
- [x] `cargo fmt/check/test/clippy --workspace` green

## Phase 3 acceptance

- [x] `crates/strategy-runtime` with stateful `Strategy` trait
- [x] `strategies/mean-reversion` updates rolling window and emits `TradeIntent`
- [x] No account risk / position sizing inside the strategy
- [x] Tests: warmup, buy/sell/hold, instrument filter, runtime event flow
- [x] `cargo fmt/check/test/clippy --workspace` green

## Phase 4 acceptance

- [x] `crates/risk-engine` with `RiskEvaluator`
- [x] Ordered, independently testable rules
- [x] Approve / resize / reject / halt paths
- [x] Determinism test (same request → same decision)
- [x] `TradeIntent` → `RiskRequest` → `RiskDecision` integration test
- [x] No venue/order side effects in risk-engine
- [x] `cargo fmt/check/test/clippy --workspace` green

## Out of scope until later phases

Rebalancing, vaults, multi-CEX execution, Kafka/K8s, frontend.
