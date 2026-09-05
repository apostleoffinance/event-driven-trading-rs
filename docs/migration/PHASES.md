# Migration Phases

Branch: `refactor/trading-infrastructure`  
Baseline tag: `v0.1-paper-engine`

| Phase | Status | Objective |
|-------|--------|-----------|
| 0 | **Done** | Audit, tag, baseline `cargo check/test/clippy` |
| 1 | **Done** | Workspace + `crates/domain` |
| 2 | Pending | Typed async events |
| 3 | Pending | Strategy runtime + mean-reversion → `TradeIntent` |
| 4 | Pending | Deterministic risk-engine |
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

## Out of scope until later phases

Rebalancing, vaults, multi-CEX execution, Kafka/K8s, frontend.
