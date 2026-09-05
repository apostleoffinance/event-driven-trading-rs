# Domain Crate

`crates/domain` holds shared quantitative trading primitives.

## Owns

- Identifiers (`AccountId`, `VenueId`, `OrderId`, `ClientOrderId`, …)
- Account / Venue / Instrument / Strategy / Deployment
- `TradeIntent`
- Risk policy request/decision types (not evaluation logic)
- Order state machine, Fill, Position
- `DomainError` / `DomainResult`
- Decimal validation helpers

## Must NOT own

- Database / SQLx
- HTTP / exchange clients
- Strategy implementations
- Risk evaluation rules
- Runtime orchestration / Tokio tasks
- UI

## Finance conventions

- All money, price, quantity, risk, and P&L fields use `rust_decimal::Decimal`
- Fallible constructors return `DomainResult<T>` — no silent defaults for invalid money
- Order transitions are explicit and tested
- Account (capital/risk ownership) is never conflated with Venue (execution location)

## Conceptual flow (Phase 1 representable)

```text
BTC Market Data
      ↓
Mean Reversion v1          (strategy metadata)
      ↓
TradeIntent
      ↓
deployment-001
      ↓
prop-account-001           (AccountType::Prop)
      ↓
RiskRequest + RiskPolicy
      ↓
RiskDecision               (types only; engine in Phase 4)
```
